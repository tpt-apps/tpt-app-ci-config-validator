// Validation driver: run a RuleSet against a Document, collect findings.

use crate::cond::Cond;
use crate::finding::{Finding, Report, Severity};
use crate::rules::RuleSet;
use crate::select::Selector;
use crate::value::{Document, Format, Value};

/// Validate one document against a rule set. A parse error short-circuits:
/// the report carries the parse error and no rule findings.
pub fn validate(doc: &Document, ruleset: &RuleSet) -> Report {
    let mut report = Report {
        document: doc.name.clone(),
        format: doc.format,
        parse_error: None,
        findings: Vec::new(),
    };

    for rule in &ruleset.rules {
        // A broken pattern in the rule itself is a rule-authoring problem,
        // not a config violation: report it once and skip the rule.
        if let Some(bad) = invalid_pattern(&rule.cond) {
            report.findings.push(Finding::new(
                rule.id.clone(),
                Severity::Warning,
                "",
                format!("rule skipped: invalid regex `{bad}`"),
            ));
            continue;
        }

        if let Some(when) = &rule.when {
            if !when_applies(when.select.eval(&doc.root), &when.cond) {
                continue;
            }
        }

        let hits = rule.select.eval(&doc.root);
        match &rule.cond {
            Cond::Exists => {
                if hits.is_empty() {
                    let path = literal_path(&rule.select);
                    report.findings.push(Finding::new(
                        rule.id.clone(),
                        rule.severity,
                        path.clone(),
                        message_for(rule, &path, None),
                    ));
                }
            }
            Cond::Missing => {
                for hit in &hits {
                    report.findings.push(Finding::new(
                        rule.id.clone(),
                        rule.severity,
                        hit.path.clone(),
                        message_for(rule, &hit.path, Some(&hit.value)),
                    ));
                }
            }
            cond => {
                for hit in &hits {
                    if cond.holds_at(hit) == Some(false) {
                        report.findings.push(Finding::new(
                            rule.id.clone(),
                            rule.severity,
                            hit.path.clone(),
                            message_for(rule, &hit.path, Some(&hit.value)),
                        ));
                    }
                }
            }
        }
    }

    // Stable, readable output: document order, then path, then rule id.
    report
        .findings
        .sort_by(|a, b| (&a.path, &a.rule_id).cmp(&(&b.path, &b.rule_id)));
    report
}

/// Validate a batch (pro edition). One report per document, input order kept.
pub fn validate_batch(docs: &[Document], ruleset: &RuleSet) -> Vec<Report> {
    docs.iter().map(|doc| validate(doc, ruleset)).collect()
}

fn when_applies(hits: Vec<crate::select::Hit>, cond: &Cond) -> bool {
    match cond {
        Cond::Exists => !hits.is_empty(),
        Cond::Missing => hits.is_empty(),
        cond => hits.iter().any(|hit| cond.holds_at(hit) == Some(true)),
    }
}

/// The selector as authored (used when nothing was selected, so there is no
/// concrete hit path to point at).
fn literal_path(sel: &Selector) -> String {
    sel.to_string()
}

fn message_for(rule: &crate::rules::Rule, path: &str, found: Option<&crate::value::Value>) -> String {
    let assertion = rule.cond.describe();
    match found {
        None => match rule.description.as_str() {
            "" => format!("`{path}` {assertion}"),
            desc => format!("{desc} (missing `{path}`)"),
        },
        Some(v) => {
            let found_text = v.scalar_text();
            let found_text = if found_text.len() > 80 {
                format!("{}…", &found_text[..80])
            } else {
                found_text
            };
            match rule.description.as_str() {
                "" => format!("`{path}` {assertion} (found `{found_text}`)"),
                desc => format!("{desc} (found `{found_text}`)"),
            }
        }
    }
}

fn invalid_pattern(cond: &Cond) -> Option<String> {
    match cond {
        Cond::Matches(p) | Cond::NotMatches(p) => {
            if regex::Regex::new(p).is_err() {
                Some(p.clone())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Convenience for callers that already know the shape: build a Document.
pub fn document(name: impl Into<String>, format: Format, root: Value) -> Document {
    Document { name: name.into(), format, root }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Rule, When};
    use crate::value::Value;
    use std::collections::BTreeMap;

    fn rule(id: &str, select: &str, cond: Cond) -> Rule {
        Rule {
            id: id.into(),
            description: String::new(),
            severity: Severity::Error,
            select: Selector::parse(select).unwrap(),
            when: None,
            cond,
        }
    }

    fn obj(pairs: &[(&str, Value)]) -> Value {
        Value::Obj(pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
    }

    #[test]
    fn exists_fires_once_when_missing() {
        let set = RuleSet::from_rules("t", vec![rule("needs-name", "metadata.name", Cond::Exists)]);
        let doc = document("d.yaml", Format::Yaml, obj(&[("apiVersion", Value::from("apps/v1"))]));
        let report = validate(&doc, &set);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].path, "metadata.name");
        assert!(report.findings[0].message.contains("must exist"));
    }

    #[test]
    fn per_hit_violations() {
        let set = RuleSet::from_rules(
            "t",
            vec![rule("no-latest", "containers[*].image", Cond::NotMatches(":latest$".into()))],
        );
        let container = |image: &str| obj(&[("image", Value::from(image))]);
        let doc = document(
            "d.yaml",
            Format::Yaml,
            obj(&[(
                "containers",
                Value::Arr(vec![container("a:1"), container("b:latest"), container("c:latest")]),
            )]),
        );
        let report = validate(&doc, &set);
        assert_eq!(report.findings.len(), 2);
        assert_eq!(report.findings[0].path, "containers[1].image");
        assert_eq!(report.findings[1].path, "containers[2].image");
    }

    #[test]
    fn when_gates_the_rule() {
        let set = RuleSet::from_rules(
            "t",
            vec![Rule {
                id: "deploy-replicas".into(),
                description: String::new(),
                severity: Severity::Error,
                select: Selector::parse("spec.replicas").unwrap(),
                when: Some(When {
                    select: Selector::parse("kind").unwrap(),
                    cond: Cond::Equals(Value::from("Deployment")),
                }),
                cond: Cond::Exists,
            }],
        );
        // Service doc: the Deployment rule must not fire.
        let svc = document("s.yaml", Format::Yaml, obj(&[("kind", Value::from("Service"))]));
        assert!(validate(&svc, &set).findings.is_empty());
        // Deployment doc: it must.
        let dep = document("d.yaml", Format::Yaml, obj(&[("kind", Value::from("Deployment"))]));
        assert_eq!(validate(&dep, &set).findings.len(), 1);
    }

    #[test]
    fn invalid_regex_is_skipped_with_warning() {
        let set = RuleSet::from_rules("t", vec![rule("bad-re", "a", Cond::Matches("(".into()))]);
        let doc = document("d.yaml", Format::Yaml, obj(&[("a", Value::from("x"))]));
        let report = validate(&doc, &set);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].severity, Severity::Warning);
        assert!(report.findings[0].message.contains("rule skipped"));
    }

    #[test]
    fn missing_never_fires_on_absent_paths() {
        let set = RuleSet::from_rules("t", vec![rule("no-host", "spec.host", Cond::Missing)]);
        let doc = document("d.yaml", Format::Yaml, obj(&[("spec", Value::Obj(BTreeMap::new()))]));
        assert!(validate(&doc, &set).findings.is_empty());
    }

    #[test]
    fn warning_severity_still_passes() {
        let mut r = rule("w", "a", Cond::Exists);
        r.severity = Severity::Warning;
        let set = RuleSet::from_rules("t", vec![r]);
        let doc = document("d.yaml", Format::Yaml, Value::Obj(BTreeMap::new()));
        let report = validate(&doc, &set);
        assert!(report.passed());
        assert_eq!(report.error_count(), 0);
        assert_eq!(report.warning_count(), 1);
    }
}
