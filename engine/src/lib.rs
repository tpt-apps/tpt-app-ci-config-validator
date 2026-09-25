// TPT CI Config Validator — validation engine.
//
// Target-independent (host + wasm32): config file(s) + schema/rule set in,
// validation findings out as plain structs. No UI deps, host-unit-testable.
//
// Shape: parser adapters (`parse`) convert YAML/JSON/XML text into the
// normalized `Value` tree; rules (`rules`, `builtin`) are path selectors +
// conditions over that tree; `validate` runs them and returns `Report`s.
// Terraform (HCL) is line-linted instead of tree-ruled.

pub mod builtin;
pub mod cond;
pub mod finding;
pub mod parse;
pub mod rules;
pub mod select;
pub mod tf_lint;
pub mod value;
pub mod validate;
pub mod xml;

pub use finding::{Finding, Report, Severity};
pub use rules::{Rule, RuleError, RuleSet};
pub use value::{detect_format, Document, Format, Value};
pub use validate::{validate, validate_batch};

use parse::Parsed;

/// Validate one source text end to end: format detection, parsing (YAML
/// streams become one report per document), the matching built-in rule
/// pack, the caller's custom rules, and the hand-written cross-checks.
///
/// Custom rules run for tree-shaped formats only (YAML/JSON/XML) — there
/// is no value tree to select paths in for Terraform.
pub fn validate_source(name: &str, text: &str, custom: Option<&RuleSet>) -> Vec<Report> {
    let format = detect_format(name, text);

    if format == Format::Hcl {
        let mut findings = tf_lint::lint_text(name, text);
        findings.sort_by(|a, b| (&a.path, &a.rule_id).cmp(&(&b.path, &b.rule_id)));
        return vec![Report {
            document: name.to_string(),
            format,
            parse_error: None,
            findings,
        }];
    }

    match parse::parse_as(name, text, format) {
        Parsed::Failed(message) => vec![Report {
            document: name.to_string(),
            format,
            parse_error: Some(message),
            findings: Vec::new(),
        }],
        Parsed::Docs(docs) => docs.iter().map(|doc| report_for(doc, custom)).collect(),
    }
}

/// Rules applied to one parsed document: its built-in pack (if any), the
/// user's custom rules, and the cross-checks.
fn report_for(doc: &Document, custom: Option<&RuleSet>) -> Report {
    let mut findings = Vec::new();
    if let Some(pack) = builtin::builtin_pack(doc) {
        findings.extend(validate::validate(doc, &pack).findings);
    }
    if let Some(custom) = custom {
        findings.extend(validate::validate(doc, custom).findings);
    }
    findings.extend(builtin::extra_checks(doc));
    findings.sort_by(|a, b| (&a.path, &a.rule_id).cmp(&(&b.path, &b.rule_id)));
    Report {
        document: doc.name.clone(),
        format: doc.format,
        parse_error: None,
        findings,
    }
}

/// The built-in rule packs, for UI display ("what will be checked").
pub fn builtin_pack_names() -> &'static [&'static str] {
    &["Kubernetes", "GitHub Actions", "Maven pom.xml", "Terraform (lint)"]
}
