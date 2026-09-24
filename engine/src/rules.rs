// The custom-rule format. Users author a rules file (YAML or JSON) that
// loads into a `RuleSet`; the same structs back the built-in schema packs.

use crate::cond::Cond;
use crate::select::Selector;
use crate::value::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub id: String,
    pub description: String,
    pub severity: crate::finding::Severity,
    /// Where to look: `spec.template.spec.containers[*].image`.
    pub select: Selector,
    /// Optional document-level precondition: the rule only runs when this
    /// condition holds somewhere in the doc (e.g. `kind` equals `Deployment`).
    pub when: Option<When>,
    /// The assertion made about every selected value.
    pub cond: Cond,
}

/// Document-level precondition: a selector plus a condition.
#[derive(Debug, Clone, PartialEq)]
pub struct When {
    pub select: Selector,
    pub cond: Cond,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RuleSet {
    pub name: String,
    pub rules: Vec<Rule>,
}

impl RuleSet {
    pub fn from_rules(name: impl Into<String>, rules: Vec<Rule>) -> RuleSet {
        RuleSet { name: name.into(), rules }
    }
}

/// A problem with a rule file itself (bad operator, bad regex, bad path).
#[derive(Debug, Clone, PartialEq)]
pub struct RuleError {
    pub rule: Option<String>,
    pub message: String,
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.rule {
            Some(id) => write!(f, "rule `{id}`: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for RuleError {}

/// Load a `RuleSet` from a normalized value tree (the output of any parser
/// adapter). Tolerant: one bad rule is reported and skipped, the rest load.
pub fn rule_set_from_value(root: &Value) -> Result<(RuleSet, Vec<RuleError>), RuleError> {
    let name = root
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("custom rules")
        .to_string();

    let raw_rules = match root.get("rules") {
        Some(Value::Arr(items)) => items.clone(),
        Some(_) => {
            return Err(RuleError {
                rule: None,
                message: "`rules` must be a list".into(),
            })
        }
        None => {
            return Err(RuleError {
                rule: None,
                message: "missing `rules:` list".into(),
            })
        }
    };

    let mut errors = Vec::new();
    let mut rules = Vec::new();
    for (i, raw) in raw_rules.iter().enumerate() {
        match rule_from_value(raw) {
            Ok(rule) => rules.push(rule),
            Err(e) => errors.push(RuleError {
                rule: raw.get("id").and_then(Value::as_str).map(str::to_string),
                message: format!("rule #{i}: {}", e.message),
            }),
        }
    }
    Ok((RuleSet { name, rules }, errors))
}

fn rule_from_value(raw: &Value) -> Result<Rule, RuleError> {
    let err = |message: &str| RuleError { rule: None, message: message.to_string() };

    let id = raw
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| err("missing `id`"))?
        .to_string();
    let wrap = |message: String| RuleError { rule: Some(id.clone()), message };

    let description = raw
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let severity = raw
        .get("severity")
        .and_then(Value::as_str)
        .and_then(crate::finding::Severity::parse)
        .unwrap_or(crate::finding::Severity::Error);

    let select_path = raw
        .get("select")
        .and_then(Value::as_str)
        .ok_or_else(|| wrap("missing `select` path".into()))?;
    let select = Selector::parse(select_path).map_err(|e| wrap(e.0))?;

    let when = match raw.get("when") {
        None => None,
        Some(w) => {
            let select_path = w
                .get("select")
                .and_then(Value::as_str)
                .ok_or_else(|| wrap("`when` needs a `select` path".into()))?;
            let select = Selector::parse(select_path).map_err(|e| wrap(e.0))?;
            let cond = cond_from_value(w).map_err(|e| wrap(e))?;
            Some(When { select, cond })
        }
    };

    let cond = cond_from_value(raw).map_err(|e| wrap(e))?;

    Ok(Rule { id, description, severity, select, when, cond })
}

/// A rule (or `when`) carries `condition: <operator>` plus an optional
/// `value:`. Unit conditions (`exists`, `missing`) take no value.
fn cond_from_value(raw: &Value) -> Result<Cond, String> {
    let keyword = raw
        .get("condition")
        .and_then(Value::as_str)
        .ok_or("missing `condition`")?;
    let value = raw.get("value");

    let need_value = || value.ok_or_else(|| format!("condition `{keyword}` needs a `value`")).cloned();
    let no_value = || {
        if value.is_some() {
            Err(format!("condition `{keyword}` takes no `value`"))
        } else {
            Ok(())
        }
    };

    match keyword {
        "exists" | "present" => {
            no_value()?;
            Ok(Cond::Exists)
        }
        "missing" | "absent" => {
            no_value()?;
            Ok(Cond::Missing)
        }
        "equals" | "eq" => Ok(Cond::Equals(need_value()?)),
        "not_equals" | "neq" => Ok(Cond::NotEquals(need_value()?)),
        "matches" => Ok(Cond::Matches(value_text(need_value()?)?)),
        "not_matches" => Ok(Cond::NotMatches(value_text(need_value()?)?)),
        "contains" => Ok(Cond::Contains(need_value()?)),
        "gt" => Ok(Cond::Gt(number(value)?)),
        "gte" => Ok(Cond::Gte(number(value)?)),
        "lt" => Ok(Cond::Lt(number(value)?)),
        "lte" => Ok(Cond::Lte(number(value)?)),
        "one_of" => {
            let v = need_value()?;
            match v {
                Value::Arr(items) => {
                    let opts = items.iter().map(Value::scalar_text).collect();
                    Ok(Cond::OneOf(opts))
                }
                other => Err(format!("condition `one_of` needs a list, got {}", other.type_name())),
            }
        }
        "is_a" | "type" => Ok(Cond::IsA(value_text(need_value()?)?)),
        other => Err(format!(
            "unknown condition `{other}` (use: exists, missing, equals, not_equals, matches, not_matches, contains, gt, gte, lt, lte, one_of, is_a)"
        )),
    }
}

fn value_text(v: Value) -> Result<String, String> {
    match v {
        Value::Str(s) => Ok(s),
        other => Ok(other.scalar_text()),
    }
}

fn number(value: Option<&Value>) -> Result<f64, String> {
    value
        .and_then(Value::as_f64)
        .ok_or_else(|| "condition needs a numeric `value`".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(pairs: &[(&str, Value)]) -> Value {
        Value::Obj(pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
    }

    #[test]
    fn loads_a_rules_file() {
        let file = obj(&[
            ("name", Value::from("team rules")),
            (
                "rules",
                Value::Arr(vec![obj(&[
                    ("id", Value::from("no-latest")),
                    ("description", Value::from("pin images")),
                    ("severity", Value::from("error")),
                    ("select", Value::from("spec.template.spec.containers[*].image")),
                    ("condition", Value::from("not_matches")),
                    ("value", Value::from(":latest$")),
                ])]),
            ),
        ]);
        let (set, errors) = rule_set_from_value(&file).unwrap();
        assert!(errors.is_empty());
        assert_eq!(set.rules.len(), 1);
        assert_eq!(set.rules[0].id, "no-latest");
        assert_eq!(set.rules[0].cond, Cond::NotMatches(":latest$".into()));
    }

    #[test]
    fn one_bad_rule_does_not_poison_the_file() {
        let file = obj(&[
            (
                "rules",
                Value::Arr(vec![
                    obj(&[
                        ("id", Value::from("broken")),
                        ("select", Value::from("a.b")),
                        ("condition", Value::from("frobnicate")),
                    ]),
                    obj(&[
                        ("id", Value::from("fine")),
                        ("select", Value::from("a.b")),
                        ("condition", Value::from("exists")),
                    ]),
                ]),
            ),
        ]);
        let (set, errors) = rule_set_from_value(&file).unwrap();
        assert_eq!(set.rules.len(), 1);
        assert_eq!(set.rules[0].id, "fine");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule.as_deref(), Some("broken"));
    }

    #[test]
    fn unit_condition_rejects_value() {
        let rule = obj(&[
            ("id", Value::from("x")),
            ("select", Value::from("a")),
            ("condition", Value::from("exists")),
            ("value", Value::from("nope")),
        ]);
        let e = rule_from_value(&rule).unwrap_err();
        assert!(e.message.contains("takes no `value`"));
    }

    #[test]
    fn when_loads_with_condition() {
        let rule = obj(&[
            ("id", Value::from("k8s-only")),
            ("select", Value::from("spec.replicas")),
            ("when", obj(&[
                ("select", Value::from("kind")),
                ("condition", Value::from("equals")),
                ("value", Value::from("Deployment")),
            ])),
            ("condition", Value::from("gte")),
            ("value", Value::Num(1.0)),
        ]);
        let rule = rule_from_value(&rule).unwrap();
        let when = rule.when.unwrap();
        assert_eq!(when.cond, Cond::Equals(Value::from("Deployment")));
    }
}
