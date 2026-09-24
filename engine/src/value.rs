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
/// Content wins over extension: a `.txt` paste that starts with
/// `apiVersion:` is still YAML.
pub fn detect_format(name: &str, text: &str) -> Format {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let trimmed = text.trim_start();

    let looks_xml = trimmed.starts_with("<?xml") || trimmed.starts_with('<');
    let looks_json = trimmed.starts_with('{') || trimmed.starts_with('[');
    let looks_hcl = ["terraform ", "resource \"", "provider \"", "variable \"", "module ", "output ", "data "]
        .iter()
        .any(|p| trimmed.starts_with(p)) || trimmed.starts_with("terraform{");
    let looks_yaml = ["apiVersion:", "kind:", "version:", "name:", "on:", "jobs:", "-", " "]
        .iter()
        .any(|p| trimmed.starts_with(p))
        || trimmed.contains(": ")
            || trimmed.contains(":\n");

    if looks_xml {
        Format::Xml
    } else if looks_json {
        Format::Json
    } else if looks_hcl {
        Format::Hcl
    } else if matches!(ext.as_str(), "yaml" | "yml") || looks_yaml {
        Format::Yaml
    } else if ext == "json" {
        Format::Json
    } else if ext == "xml" {
        Format::Xml
    } else if ext == "tf" || ext == "tfvars" || ext == "hcl" {
        Format::Hcl
    } else {
        Format::Yaml
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_beats_extension() {
        assert_eq!(detect_format("paste.txt", "apiVersion: apps/v1\n"), Format::Yaml);
        assert_eq!(detect_format("paste.txt", "{\"a\": 1}"), Format::Json);
        assert_eq!(detect_format("paste.txt", "<?xml version=\"1.0\"?>"), Format::Xml);
        assert_eq!(detect_format("x.tf", "terraform {\n}\n"), Format::Hcl);
        assert_eq!(detect_format("x.yaml", "kind: Deployment"), Format::Yaml);
    }
}
