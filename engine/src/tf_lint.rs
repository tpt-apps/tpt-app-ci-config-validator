// Terraform (.tf) checks. HCL is line/block structured rather than the
// nested-map shape the selector engine walks, so these checks are a
// line-based lint: block-label hygiene, brace balance, duplicate resource
// addresses, and hardcoded-credential patterns. Deterministic, wasm-safe,
// no HCL grammar dependency.

use crate::finding::{Finding, Severity};

/// Line-lint raw `.tf` source text.
pub fn lint_text(name: &str, text: &str) -> Vec<Finding> {
    let mut findings = Vec::new();

    let mut brace_depth: i64 = 0;
    let mut seen_resources: std::collections::BTreeMap<(String, String), usize> = Default::default();
    let resource_re = regex::Regex::new(r#"^\s*resource\s+"([^"]+)"\s+"([^"]+)""#).expect("static regex");
    let secret_re = regex::Regex::new(
        r#"(?i)^\s*[a-z0-9_]*(password|secret|token|api_key|access_key|private_key)[a-z0-9_]*\s*=\s*"([^"$]{4,})""#,
    )
    .expect("static regex");

    for (idx, line) in text.lines().enumerate() {
        let line_no = idx + 1;
        let loc = format!("line {line_no}");

        // Brace balance, ignoring braces inside quoted strings on the line.
        let code = strip_quoted(line);
        brace_depth += code.matches('{').count() as i64;
        brace_depth -= code.matches('}').count() as i64;

        // Unterminated string on a physical line (HCL strings may span
        // lines via heredocs, so this is a warning, not an error).
        if code.matches('"').count() % 2 == 1 {
            findings.push(Finding::new(
                "tf-string-terminated",
                Severity::Warning,
                loc.clone(),
                "possible unterminated string (odd number of quotes on this line)",
            ));
        }

        if let Some(caps) = resource_re.captures(line) {
            let r#type = caps[1].to_string();
            let r#name = caps[2].to_string();
            let addr = (r#type.clone(), r#name.clone());
            if let Some(prev) = seen_resources.get(&addr) {
                findings.push(Finding::new(
                    "tf-duplicate-resource",
                    Severity::Error,
                    loc.clone(),
                    format!(
                        "duplicate resource address `{type}.{name}` (first defined on line {prev})"
                    ),
                ));
            } else {
                seen_resources.insert(addr, line_no);
            }
        }

        if let Some(caps) = secret_re.captures(line) {
            let key = caps[1].to_string();
            findings.push(Finding::new(
                "tf-hardcoded-secret",
                Severity::Warning,
                loc,
                format!(
                    "`{key}` looks like a hardcoded credential — use a variable, TF_VAR, or a secrets manager"
                ),
            ));
        }
    }

    if brace_depth != 0 {
        findings.push(Finding::new(
            "tf-brace-balance",
            Severity::Error,
            name.to_string(),
            format!("unbalanced braces: {brace_depth} unclosed `{{` block(s) — a block is missing its closing brace"),
        ));
    }

    findings
}

/// Blank out the *contents* of double-quoted spans but keep the quote
/// delimiters themselves, so brace counting ignores string bodies while the
/// delimiter count still reveals unterminated strings.
fn strip_quoted(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_string = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_string = !in_string;
                out.push('"');
            }
            '\\' if in_string => {
                chars.next(); // skip escaped char inside string
            }
            _ if !in_string => out.push(c),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_unbalanced_braces() {
        let text = "resource \"aws_s3_bucket\" \"b\" {\n  tags = {\n    Team = \"x\"\n";
        let findings = lint_text("main.tf", text);
        assert!(findings.iter().any(|f| f.rule_id == "tf-brace-balance"));
    }

    #[test]
    fn balanced_braces_pass() {
        let text = "resource \"aws_s3_bucket\" \"b\" {\n  tags = {\n    Team = \"x\"\n  }\n}\n";
        let findings = lint_text("main.tf", text);
        assert!(findings.iter().all(|f| f.rule_id != "tf-brace-balance"), "{findings:?}");
    }

    #[test]
    fn duplicate_resource_address() {
        let text = "resource \"a\" \"b\" {}\nresource \"a\" \"b\" {}\n";
        let findings = lint_text("main.tf", text);
        assert!(findings.iter().any(|f| f.rule_id == "tf-duplicate-resource" && f.path == "line 2"));
    }

    #[test]
    fn hardcoded_secret_warns_but_var_refs_do_not() {
        let text = "password = \"hunter2\"\nok = \"${var.password}\"\n";
        let findings = lint_text("main.tf", text);
        assert!(findings.iter().any(|f| f.rule_id == "tf-hardcoded-secret" && f.path == "line 1"));
        assert!(!findings.iter().any(|f| f.path == "line 2" && f.rule_id == "tf-hardcoded-secret"));
    }

    #[test]
    fn short_values_are_not_secrets() {
        // 4+ chars required, so empty/short placeholder values don't warn.
        let text = "token = \"abc\"\n";
        assert!(lint_text("m.tf", text).is_empty());
    }

    #[test]
    fn braces_inside_strings_do_not_count() {
        let text = "locals {\n  json = \"{ }\"\n}\n";
        assert!(lint_text("m.tf", text).is_empty());
    }
}
