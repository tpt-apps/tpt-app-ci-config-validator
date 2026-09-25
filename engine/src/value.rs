// Normalized document model. Every parser adapter (YAML/JSON/XML/HCL)
// converts into this tree, so rules are written once against one shape and
// the engine has no parser dependencies in its core.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Value>),
    Obj(BTreeMap<String, Value>),
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Num(_) => "number",
            Value::Str(_) => "string",
            Value::Arr(_) => "array",
            Value::Obj(_) => "object",
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(map) => map.get(key),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Integral floats count as ints (`replicas: 3` and `replicas: 3.0`).
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Num(n) if n.fract() == 0.0 => Some(*n as i64),
            Value::Str(s) => s.trim().parse::<i64>().ok(),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            Value::Str(s) => s.trim().parse::<f64>().ok(),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Rendering for messages and for scalar comparison against rule values.
    pub fn scalar_text(&self) -> String {
        match self {
            Value::Null => "null".into(),
            Value::Bool(b) => b.to_string(),
            Value::Num(n) => {
                if n.fract() == 0.0 {
                    format!("{}", *n as i64)
                } else {
                    n.to_string()
                }
            }
            Value::Str(s) => s.clone(),
            Value::Arr(_) | Value::Obj(_) => self.type_name().to_string(),
        }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.into())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::Num(n as f64)
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::Num(n)
    }
}

/// A parsed config file: normalized tree plus how it was identified.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    /// File name (or `<paste>` for inline input) used in findings.
    pub name: String,
    pub format: Format,
    pub root: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Yaml,
    Json,
    Xml,
    Hcl,
}

impl Format {
    pub fn label(self) -> &'static str {
        match self {
            Format::Yaml => "YAML",
            Format::Json => "JSON",
            Format::Xml => "XML",
            Format::Hcl => "HCL (Terraform)",
        }
    }
}

/// Best-effort format detection from a file name + leading content.
/// A known extension wins (a `.tf` file that starts with `#` comments is
/// still Terraform); otherwise content is sniffed with comments and blank
/// leading lines stripped.
pub fn detect_format(name: &str, text: &str) -> Format {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let body = strip_leading_comments(text);

    let looks_xml = body.starts_with("<?xml") || body.starts_with('<');
    let looks_json = body.starts_with('{') || body.starts_with('[');

    // Known extensions are authoritative — content sniffing is only for
    // pastes and unknown extensions.
    match ext.as_str() {
        "yaml" | "yml" if !looks_xml && !looks_json => return Format::Yaml,
        "json" if !looks_xml => return Format::Json,
        "xml" => return Format::Xml,
        "tf" | "tfvars" | "hcl" => return Format::Hcl,
        _ => {}
    }

    let looks_hcl = body.lines().any(|line| {
        let line = line.trim_start();
        HCL_BLOCK_STARTS.iter().any(|p| line.starts_with(p))
    });
    let looks_yaml =
        body.contains(": ") || body.contains(":\n") || body.trim_start().starts_with("- ");

    if looks_xml {
        Format::Xml
    } else if looks_json {
        Format::Json
    } else if looks_hcl {
        Format::Hcl
    } else if looks_yaml {
        Format::Yaml
    } else {
        Format::Yaml
    }
}

const HCL_BLOCK_STARTS: &[&str] = &[
    "terraform", "resource \"", "provider \"", "variable \"", "module ", "output ", "data \"", "locals",
];

/// Drop blank lines and whole-line comments (`#`, `//`) from the top so a
/// commented file still sniffs by its first real content line.
fn strip_leading_comments(text: &str) -> &str {
    let mut body = text;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            body = &body[line.len()..];
            body = body.strip_prefix('\n').unwrap_or(body);
            body = body.strip_prefix("\r\n").unwrap_or(body);
        } else {
            break;
        }
    }
    body.trim_start()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_beats_extension() {
        assert_eq!(detect_format("paste.txt", "apiVersion: apps/v1\n"), Format::Yaml);
        assert_eq!(detect_format("paste.txt", "{\"a\": 1}"), Format::Json);
        assert_eq!(detect_format("paste.txt", "<?xml version=\"1.0\"?>"), Format::Xml);
        assert_eq!(detect_format("x.yaml", "kind: Deployment"), Format::Yaml);
    }

    #[test]
    fn extension_wins_for_commented_files() {
        // A `.tf` file that opens with `#` comments is still Terraform, and
        // a commented YAML paste is still YAML.
        assert_eq!(detect_format("main.tf", "# comment\n# more\nterraform {\n}\n"), Format::Hcl);
        assert_eq!(detect_format("paste.txt", "# note:\nkind: Deployment\n"), Format::Yaml);
        assert_eq!(detect_format("flow.tf", "// module \"vpc\" {\n}\n"), Format::Hcl);
    }
}
