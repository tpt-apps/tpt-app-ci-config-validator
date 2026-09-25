// Rule conditions: the assertion a rule makes about each selected value.
// The operator set is deliberately small and reads like plain words in the
// rule file: `condition: not_matches`, `value: ":latest$"`.

use crate::select::Hit;
use crate::value::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum Cond {
    /// The selected location must exist. Fires once (at the selector's
    /// parent path) when the selector matched nothing.
    Exists,
    /// The selected location must not exist. Fires per existing hit.
    Missing,
    Equals(Value),
    NotEquals(Value),
    /// Case-sensitive regex on the value's text form.
    Matches(String),
    NotMatches(String),
    /// Substring (string values) or membership (array values).
    Contains(Value),
    Gt(f64),
    Gte(f64),
    Lt(f64),
    Lte(f64),
    /// Value's text form is one of the listed options.
    OneOf(Vec<String>),
    /// Value has this type: string | number | bool | array | object | null.
    IsA(String),
    /// Object (or any item of an array of objects) has this key. This is
    /// how "every container must have an image" is expressed: select
    /// `containers[*]`, then `has_key: image` — selecting
    /// `containers[*].image` alone cannot see containers missing the key.
    HasKey(String),
}

impl Cond {
    /// Does this condition hold at the hit? `None` means the condition is
    /// not value-shaped (Exists/Missing) and is evaluated by the driver.
    pub fn holds_at(&self, hit: &Hit) -> Option<bool> {
        let v = &hit.value;
        let verdict = match self {
            Cond::Exists | Cond::Missing => return None,
            Cond::Equals(expected) => scalar_eq(v, expected),
            Cond::NotEquals(expected) => !scalar_eq(v, expected),
            Cond::Matches(pattern) => regex(pattern).is_some_and(|re| re.is_match(&v.scalar_text())),
            Cond::NotMatches(pattern) => !regex(pattern).is_some_and(|re| re.is_match(&v.scalar_text())),
            Cond::Contains(needle) => match (v, needle) {
                (Value::Str(s), Value::Str(n)) => s.contains(n.as_str()),
                (Value::Arr(items), needle) => items.iter().any(|item| scalar_eq(item, needle)),
                (Value::Obj(_), _) => false,
                (v, needle) => v.scalar_text().contains(&needle.scalar_text()),
            },
            Cond::Gt(n) => v.as_f64().is_some_and(|x| x > *n),
            Cond::Gte(n) => v.as_f64().is_some_and(|x| x >= *n),
            Cond::Lt(n) => v.as_f64().is_some_and(|x| x < *n),
            Cond::Lte(n) => v.as_f64().is_some_and(|x| x <= *n),
            Cond::OneOf(options) => {
                let text = v.scalar_text();
                options.iter().any(|o| o == &text)
            }
            Cond::IsA(ty) => v.type_name() == ty.as_str(),
            Cond::HasKey(key) => match v {
                Value::Obj(map) => map.contains_key(key),
                Value::Arr(items) => items
                    .iter()
                    .any(|item| matches!(item, Value::Obj(map) if map.contains_key(key))),
                _ => false,
            },
        };
        Some(verdict)
    }

    /// Human-readable phrasing for finding messages.
    pub fn describe(&self) -> String {
        match self {
            Cond::Exists => "must exist".to_string(),
            Cond::Missing => "must not be set".to_string(),
            Cond::Equals(v) => format!("must equal `{}`", v.scalar_text()),
            Cond::NotEquals(v) => format!("must not equal `{}`", v.scalar_text()),
            Cond::Matches(p) => format!("must match `{p}`"),
            Cond::NotMatches(p) => format!("must not match `{p}`"),
            Cond::Contains(v) => format!("must contain `{}`", v.scalar_text()),
            Cond::Gt(n) => format!("must be > {n}"),
            Cond::Gte(n) => format!("must be >= {n}"),
            Cond::Lt(n) => format!("must be < {n}"),
            Cond::Lte(n) => format!("must be <= {n}"),
            Cond::OneOf(opts) => format!("must be one of {}", quote_list(opts)),
            Cond::IsA(ty) => format!("must be a {ty}"),
            Cond::HasKey(key) => format!("must have a `{key}` key"),
        }
    }

    /// The condition's operator keyword as written in a rule file.
    pub fn keyword(&self) -> &'static str {
        match self {
            Cond::Exists => "exists",
            Cond::Missing => "missing",
            Cond::Equals(_) => "equals",
            Cond::NotEquals(_) => "not_equals",
            Cond::Matches(_) => "matches",
            Cond::NotMatches(_) => "not_matches",
            Cond::Contains(_) => "contains",
            Cond::Gt(_) => "gt",
            Cond::Gte(_) => "gte",
            Cond::Lt(_) => "lt",
            Cond::Lte(_) => "lte",
            Cond::OneOf(_) => "one_of",
            Cond::IsA(_) => "is_a",
            Cond::HasKey(_) => "has_key",
        }
    }
}

fn scalar_eq(a: &Value, b: &Value) -> bool {
    // 3 == 3.0 == "3" == "03" — rule authors write values by hand, so any
    // numeric-looking pair compares numerically; type strictness is the
    // `is_a` condition's job.
    if let (Some(x), Some(y)) = (a.as_f64(), b.as_f64()) {
        return (x - y).abs() < f64::EPSILON;
    }
    if let (Some(x), Some(y)) = (a.as_bool(), b.as_bool()) {
        return x == y;
    }
    a.scalar_text() == b.scalar_text()
}

fn regex(pattern: &str) -> Option<regex::Regex> {
    // Bad patterns in rule files are a rule-authoring bug: the rule is
    // skipped (with a finding from the loader), never a panic.
    regex::Regex::new(pattern).ok()
}

fn quote_list(items: &[String]) -> String {
    items.iter().map(|s| format!("`{s}`")).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(v: Value) -> Hit {
        Hit { path: "x".into(), value: v }
    }

    #[test]
    fn numeric_equality_is_forgiving() {
        assert_eq!(scalar_eq(&Value::Num(3.0), &Value::Num(3.0)), true);
        assert_eq!(scalar_eq(&Value::from("3"), &Value::Num(3.0)), true);
        assert_eq!(scalar_eq(&Value::from("03"), &Value::Num(3.0)), true);
        assert_eq!(scalar_eq(&Value::from("three"), &Value::Num(3.0)), false);
    }

    #[test]
    fn matches_operators() {
        let c = Cond::NotMatches(":latest$".into());
        assert_eq!(c.holds_at(&hit(Value::from("img:1.0"))), Some(true));
        assert_eq!(c.holds_at(&hit(Value::from("img:latest"))), Some(false));
    }

    #[test]
    fn bad_regex_fails_closed() {
        // A broken pattern must not report the config as violating.
        let c = Cond::Matches("(".into());
        assert_eq!(c.holds_at(&hit(Value::from("anything"))), Some(false));
    }

    #[test]
    fn numeric_compare_rejects_non_numbers() {
        assert_eq!(Cond::Gte(1.0).holds_at(&hit(Value::from("three"))), Some(false));
        assert_eq!(Cond::Gte(1.0).holds_at(&hit(Value::Num(3.0))), Some(true));
    }

    #[test]
    fn one_of_matches_text_form() {
        let c = Cond::OneOf(vec!["apps/v1".into(), "batch/v1".into()]);
        assert_eq!(c.holds_at(&hit(Value::from("apps/v1"))), Some(true));
        assert_eq!(c.holds_at(&hit(Value::from("extensions/v1beta1"))), Some(false));
    }

    #[test]
    fn contains_on_string_and_array() {
        assert_eq!(Cond::Contains(Value::from("run")).holds_at(&hit(Value::from("runs-on"))), Some(true));
        let arr = Value::Arr(vec![Value::from("a"), Value::from("b")]);
        assert_eq!(Cond::Contains(Value::from("b")).holds_at(&hit(arr)), Some(true));
    }
}
