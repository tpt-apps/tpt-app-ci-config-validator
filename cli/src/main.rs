//! TPT CI Config Validator — CI CLI (Pro edition).
//!
//! Validates config files (or whole directory trees) against the built-in
//! schema packs and an optional custom rules file. Built to drop into a CI
//! pipeline:
//!
//! ```text
//! tpt-cicfg-cli deploy/ --rules team-rules.yaml --format text
//! ```
//!
//! Exit codes (the CI contract):
//!   0 — every file passed (warnings do not fail the run)
//!   1 — violations found (any error-severity finding or parse failure)
//!   2 — usage or I/O error (bad flags, unreadable path, bad rules file)
//!
//! `--format json` emits a single machine-readable object on stdout; text
//! format is for humans. Diagnostics always go to stderr.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use tpt_config_engine::{validate_source, Format, Report, RuleSet};
use tpt_config_engine::{parse::parse_source, rules::rule_set_from_value};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
TPT CI Config Validator — validate configs against schemas and custom rules

USAGE:
    tpt-cicfg-cli [OPTIONS] <PATH>...

ARGS:
    <PATH>...            Config files or directories to validate. Directories
                         are walked recursively (.yaml .yml .json .xml .tf .hcl).

OPTIONS:
    -r, --rules <FILE>   Custom rules file (YAML or JSON). See the docs or
                         examples/custom-rules.yaml for the format.
    -f, --format <FMT>   Output format: text (default) or json.
    -h, --help           Print help.
    -V, --version        Print version.

EXIT CODES:
    0    all files passed (warnings allowed)
    1    violations found (errors or parse failures)
    2    usage or I/O error
";

#[derive(Debug, Clone, Copy, PartialEq)]
enum OutFormat {
    Text,
    Json,
}

struct Args {
    paths: Vec<PathBuf>,
    rules: Option<PathBuf>,
    format: OutFormat,
}

enum ParseOutcome {
    Ok(Args),
    Help,
    Version,
    Bad(String),
}

fn parse_args<I: Iterator<Item = String>>(mut it: I) -> ParseOutcome {
    let mut paths = Vec::new();
    let mut rules = None;
    let mut format = OutFormat::Text;

    while let Some(arg) = it.next() {
        let mut take_value = |flag: &str, slot: &mut Option<String>| -> Option<ParseOutcome> {
            let value = match it.next() {
                Some(v) => v,
                None => {
                    return Some(ParseOutcome::Bad(format!("{flag} needs a value")));
                }
            };
            *slot = Some(value);
            None
        };

        let outcome = if arg == "-h" || arg == "--help" {
            ParseOutcome::Help
        } else if arg == "-V" || arg == "--version" {
            ParseOutcome::Version
        } else if arg == "-r" || arg == "--rules" {
            let mut slot = None;
            match take_value("--rules", &mut slot) {
                Some(o) => o,
                None => {
                    rules = slot.map(PathBuf::from);
                    continue;
                }
            }
        } else if arg == "-f" || arg == "--format" {
            let mut slot = None;
            match take_value("--format", &mut slot) {
                Some(o) => o,
                None => {
                    let value = slot.expect("just set");
                    format = match value.as_str() {
                        "text" => OutFormat::Text,
                        "json" => OutFormat::Json,
                        other => {
                            return ParseOutcome::Bad(format!(
                                "unknown format `{other}` (use text or json)"
                            ))
                        }
                    };
                    continue;
                }
            }
        } else if let Some(value) = arg.strip_prefix("--rules=") {
            rules = Some(PathBuf::from(value));
            continue;
        } else if let Some(value) = arg.strip_prefix("--format=") {
            format = match value {
                "text" => OutFormat::Text,
                "json" => OutFormat::Json,
                other => {
                    return ParseOutcome::Bad(format!("unknown format `{other}` (use text or json)"))
                }
            };
            continue;
        } else if arg.starts_with('-') && arg.len() > 1 {
            ParseOutcome::Bad(format!("unknown flag `{arg}` (see --help)"))
        } else {
            paths.push(PathBuf::from(arg));
            continue;
        };
        return outcome;
    }

    if paths.is_empty() {
        return ParseOutcome::Bad("no config paths given (see --help)".into());
    }
    ParseOutcome::Ok(Args { paths, rules, format })
}

/// Files to validate: positional files pass through; directories are walked
/// recursively for known config extensions, skipping VCS/build dirs.
fn collect_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    const EXTS: &[&str] = &["yaml", "yml", "json", "xml", "tf", "hcl"];
    const SKIP_DIRS: &[&str] = &[".git", "target", "node_modules", "dist", ".github"];

    let mut files = Vec::new();
    let mut stack: Vec<PathBuf> = paths.to_vec();
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            let entries = std::fs::read_dir(&path)
                .map_err(|e| format!("cannot read directory {}: {e}", path.display()))?;
            for entry in entries {
                let entry = entry.map_err(|e| format!("directory walk error: {e}"))?;
                let child = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if child.is_dir() {
                    if !SKIP_DIRS.contains(&name.as_str()) && !name.starts_with('.') {
                        stack.push(child);
                    }
                } else if matches!(
                    child.extension().and_then(|e| e.to_str()),
                    Some(ext) if EXTS.contains(&ext.to_ascii_lowercase().as_str())
                ) {
                    files.push(child);
                }
            }
        } else {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn load_rules(path: &Path) -> Result<RuleSet, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read rules file {}: {e}", path.display()))?;
    let doc = match parse_source(&path.display().to_string(), &text) {
        tpt_config_engine::parse::Parsed::Docs(mut docs) => docs.remove(0),
        tpt_config_engine::parse::Parsed::Failed(e) => {
            return Err(format!("rules file {} failed to parse: {e}", path.display()))
        }
    };
    let (set, errors) =
        rule_set_from_value(&doc.root).map_err(|e| format!("bad rules file: {e}"))?;
    if !errors.is_empty() {
        for e in &errors {
            eprintln!("warning: skipped rule: {e}");
        }
    }
    Ok(set)
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        ParseOutcome::Ok(args) => args,
        ParseOutcome::Help => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        ParseOutcome::Version => {
            println!("tpt-cicfg-cli {VERSION}");
            return ExitCode::SUCCESS;
        }
        ParseOutcome::Bad(message) => {
            eprintln!("tpt-cicfg-cli: {message}");
            return ExitCode::from(2);
        }
    };

    let rules = match &args.rules {
        Some(path) => match load_rules(path) {
            Ok(set) => Some(set),
            Err(e) => {
                eprintln!("tpt-cicfg-cli: {e}");
                return ExitCode::from(2);
            }
        },
        None => None,
    };

    let files = match collect_files(&args.paths) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("tpt-cicfg-cli: {e}");
            return ExitCode::from(2);
        }
    };
    if files.is_empty() {
        eprintln!("tpt-cicfg-cli: no config files found in the given paths");
        return ExitCode::from(2);
    }

    let mut reports: Vec<(String, Vec<Report>)> = Vec::new();
    let mut io_errors: Vec<String> = Vec::new();
    for file in &files {
        let text = match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(e) => {
                io_errors.push(format!("{}: {e}", file.display()));
                continue;
            }
        };
        let name = file.display().to_string();
        let parsed = validate_source(&name, &text, rules.as_ref());
        reports.push((name, parsed));
    }

    let failed = io_errors
        .iter()
        .any(|_| true)
        || reports.iter().any(|(_, rs)| rs.iter().any(|r| !r.passed()));

    match args.format {
        OutFormat::Json => {
            print!("{}", json_output(&reports, &io_errors));
            println!();
        }
        OutFormat::Text => text_output(&reports, &io_errors),
    }

    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn text_output(reports: &[(String, Vec<Report>)], io_errors: &[String]) {
    for err in io_errors {
        eprintln!("error: {err}");
    }
    for (name, reports) in reports {
        for report in reports {
            match &report.parse_error {
                Some(err) => {
                    println!("{name}: PARSE ERROR");
                    println!("  {err}");
                }
                None if report.findings.is_empty() => println!("{name}: PASSED"),
                None => {
                    let errors = report.error_count();
                    let warnings = report.warning_count();
                    if errors > 0 {
                        println!("{name}: FAILED ({errors} error(s), {warnings} warning(s))");
                    } else {
                        println!("{name}: PASSED ({warnings} warning(s))");
                    }
                    for finding in &report.findings {
                        println!(
                            "  {} {}{}: {}",
                            finding.severity.label(),
                            finding.path,
                            if finding.path.is_empty() { "" } else { ":" },
                            finding.message
                        );
                    }
                }
            }
        }
    }
}

fn json_output(reports: &[(String, Vec<Report>)], io_errors: &[String]) -> String {
    let mut total_errors = 0usize;
    let mut total_warnings = 0usize;
    let mut files_json = Vec::new();

    for (_name, rs) in reports {
        for report in rs {
            total_errors += report.error_count() + usize::from(report.parse_error.is_some());
            total_warnings += report.warning_count();
            files_json.push(serde_json::json!({
                "file": report.document,
                "format": report.format.label(),
                "passed": report.passed(),
                "parseError": report.parse_error,
                "findings": report.findings.iter().map(|f| serde_json::json!({
                    "ruleId": f.rule_id,
                    "severity": f.severity.label(),
                    "path": f.path,
                    "message": f.message,
                })).collect::<Vec<_>>(),
            }));
        }
    }
    for err in io_errors {
        files_json.push(serde_json::json!({
            "file": err,
            "format": Format::Yaml.label(),
            "passed": false,
            "parseError": err,
            "findings": [],
        }));
    }

    serde_json::json!({
        "tool": "tpt-cicfg-cli",
        "version": VERSION,
        "summary": {
            "files": files_json.len(),
            "errors": total_errors,
            "warnings": total_warnings,
            "passed": total_errors == 0,
        },
        "reports": files_json,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> ParseOutcome {
        parse_args(items.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_flags_and_paths() {
        match args(&["deploy.yaml", "--rules", "r.yaml", "--format", "json"]) {
            ParseOutcome::Ok(a) => {
                assert_eq!(a.paths, vec![PathBuf::from("deploy.yaml")]);
                assert_eq!(a.rules, Some(PathBuf::from("r.yaml")));
                assert_eq!(a.format, OutFormat::Json);
            }
            _ => panic!("expected ok"),
        }
    }

    #[test]
    fn equals_forms() {
        match args(&("--rules=team.yaml --format=json x.yaml".split(' ').collect::<Vec<_>>().as_slice())) {
            ParseOutcome::Ok(a) => {
                assert_eq!(a.rules, Some(PathBuf::from("team.yaml")));
                assert_eq!(a.format, OutFormat::Json);
            }
            _ => panic!("expected ok"),
        }
    }

    #[test]
    fn missing_paths_is_bad() {
        assert!(matches!(args(&[]), ParseOutcome::Bad(_)));
        assert!(matches!(args(&["--format", "json"]), ParseOutcome::Bad(_)));
    }

    #[test]
    fn dangling_flag_value_is_bad() {
        assert!(matches!(args(&["--rules"]), ParseOutcome::Bad(_)));
    }

    #[test]
    fn unknown_flag_and_format() {
        assert!(matches!(args(&["--frobnicate", "x"]), ParseOutcome::Bad(_)));
        assert!(matches!(args(&["-f", "xml", "x"]), ParseOutcome::Bad(_)));
    }
}
