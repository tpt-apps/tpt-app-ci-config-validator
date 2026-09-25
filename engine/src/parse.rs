// Parser adapters: config text → normalized `Value` tree, per format.
// YAML goes through `tpt-yaml-core` (multi-document aware), JSON through
// `serde_json`, XML through the vendored minimal parser in `xml`. HCL is
// not adapted to a tree — Terraform validation is the line lint.

use crate::value::{detect_format, Document, Format, Value};
use std::collections::BTreeMap;
use tpt_yaml_core as yaml;

/// Outcome of parsing one source text: zero or more documents (YAML streams
/// split into one `Document` each) or a single parse failure.
#[derive(Debug, Clone, PartialEq)]
pub enum Parsed {
    Docs(Vec<Document>),
    Failed(String),
}

/// Detect the format from the file name + content, then parse.
pub fn parse_source(name: &str, text: &str) -> Parsed {
    let format = detect_format(name, text);
    parse_as(name, text, format)
}

pub fn parse_as(name: &str, text: &str, format: Format) -> Parsed {
    match format {
        Format::Yaml => parse_yaml(name, text),
        Format::Json => parse_json(name, text),
        Format::Xml => match crate::xml::parse(text) {
            Ok(root) => Parsed::Docs(vec![Document { name: name.into(), format, root }]),
            Err(e) => Parsed::Failed(e),
        },
        // Terraform validates by line lint, not tree rules; callers route
        // there before parsing. Treat any HCL text as "no tree".
        Format::Hcl => Parsed::Failed(
            "HCL is validated by lint, not by tree rules (internal routing bug)".into(),
        ),
    }
}

fn parse_yaml(name: &str, text: &str) -> Parsed {
    match yaml::parse(text) {
        Err(e) => Parsed::Failed(e.to_string()),
        Ok(doc) => {
            if doc.documents.is_empty() {
                return Parsed::Failed("empty document".into());
            }
            let count = doc.documents.len();
            let docs = doc
                .documents
                .iter()
                .enumerate()
                .map(|(i, root)| Document {
                    name: doc_name(name, i, count),
                    format: Format::Yaml,
                    root: adapt_yaml_node(&doc, *root, 0),
                })
                .collect();
            Parsed::Docs(docs)
        }
    }
}

fn doc_name(name: &str, index: usize, count: usize) -> String {
    if count == 1 {
        name.to_string()
    } else {
        format!("{name}#{}", index + 1)
    }
}

/// Convert a tpt-yaml arena node into the normalized tree. Aliases are
/// followed with a depth budget so a self-referencing anchor can't recurse
/// forever.
fn adapt_yaml_node(doc: &yaml::Document, id: yaml::NodeId, depth: u32) -> Value {
    if depth > 64 {
        return Value::Null;
    }
    let Some(node) = doc.node(id) else {
        return Value::Null;
    };
    match &node.kind {
        yaml::NodeKind::Scalar(scalar) => scalar_value(scalar),
        yaml::NodeKind::Sequence(items) => Value::Arr(
            items
                .iter()
                .map(|item| adapt_yaml_node(doc, *item, depth + 1))
                .collect(),
        ),
        yaml::NodeKind::Mapping(pairs) => {
            let mut map = BTreeMap::new();
            for (key_id, value_id) in pairs {
                // Keys are scalars (or complex keys, which collapse to their
                // rendered form — none of the target schemas use them).
                let key = match doc.node(*key_id) {
                    Some(key_node) => match &key_node.kind {
                        yaml::NodeKind::Scalar(scalar) => scalar_value(scalar).scalar_text(),
                        _ => yaml::node::render_inline(key_node, doc),
                    },
                    None => continue,
                };
                let value = adapt_yaml_node(doc, *value_id, depth + 1);
                map.insert(key, value);
            }
            Value::Obj(map)
        }
        yaml::NodeKind::Alias(target) => adapt_yaml_node(doc, *target, depth + 1),
    }
}

fn scalar_value(scalar: &yaml::Scalar) -> Value {
    match &scalar.value {
        yaml::ScalarValue::Null => Value::Null,
        yaml::ScalarValue::Bool(b) => Value::Bool(*b),
        yaml::ScalarValue::Int(i) => Value::Num(*i as f64),
        yaml::ScalarValue::Float(f) => Value::Num(*f),
        yaml::ScalarValue::String(s) => Value::Str(s.clone()),
        yaml::ScalarValue::Timestamp(s) => Value::Str(s.clone()),
    }
}

fn parse_json(name: &str, text: &str) -> Parsed {
    match serde_json::from_str::<serde_json::Value>(text) {
        Err(e) => Parsed::Failed(e.to_string()),
        Ok(value) => Parsed::Docs(vec![Document {
            name: name.into(),
            format: Format::Json,
            root: adapt_json(value),
        }]),
    }
}

fn adapt_json(value: serde_json::Value) -> Value {
    match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => n
            .as_f64()
            .map(Value::Num)
            .unwrap_or(Value::Null),
        serde_json::Value::String(s) => Value::Str(s),
        serde_json::Value::Array(items) => Value::Arr(items.into_iter().map(adapt_json).collect()),
        serde_json::Value::Object(map) => Value::Obj(
            map.into_iter()
                .map(|(k, v)| (k, adapt_json(v)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multi_document_yaml() {
        let text = "kind: A\n---\nkind: B\n";
        match parse_source("two.yaml", text) {
            Parsed::Docs(docs) => {
                assert_eq!(docs.len(), 2);
                assert_eq!(docs[0].name, "two.yaml#1");
                assert_eq!(docs[1].name, "two.yaml#2");
                assert_eq!(docs[1].root.get("kind").and_then(Value::as_str), Some("B"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn yaml_scalar_typing() {
        match parse_source("t.yaml", "n: 3\nf: 1.5\nb: true\ns: hello\nx:\n") {
            Parsed::Docs(docs) => {
                let root = &docs[0].root;
                assert_eq!(root.get("n"), Some(&Value::Num(3.0)));
                assert_eq!(root.get("f"), Some(&Value::Num(1.5)));
                assert_eq!(root.get("b"), Some(&Value::Bool(true)));
                assert_eq!(root.get("s"), Some(&Value::Str("hello".into())));
                assert_eq!(root.get("x"), Some(&Value::Null));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn yaml_parse_error_is_reported() {
        // A tab-indented line is a control character error in YAML.
        let text = "a:\n\tb: 1\n";
        match parse_source("bad.yaml", text) {
            Parsed::Failed(msg) => assert!(!msg.is_empty()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn yaml_anchors_and_aliases_resolve() {
        let text = "base: &b\n  x: 1\nuse: *b\n";
        match parse_source("a.yaml", text) {
            Parsed::Docs(docs) => {
                let use_v = docs[0].root.get("use").unwrap();
                assert_eq!(use_v.get("x"), Some(&Value::Num(1.0)));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_json() {
        match parse_source("c.json", r#"{"a": [1, "two", {"b": null}]}"#) {
            Parsed::Docs(docs) => {
                assert_eq!(docs.len(), 1);
                assert_eq!(docs[0].format, Format::Json);
                let arr = docs[0].root.get("a").unwrap();
                assert!(matches!(arr, Value::Arr(items) if items.len() == 3));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_xml() {
        match parse_source("p.xml", "<project><artifactId>x</artifactId></project>") {
            Parsed::Docs(docs) => {
                assert_eq!(docs[0].format, Format::Xml);
                assert_eq!(
                    docs[0].root.get("artifactId").and_then(Value::as_str),
                    Some("x")
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn flow_yaml_collections() {
        match parse_source("f.yaml", "list: [1, 2]\nmap: {a: 1}\n") {
            Parsed::Docs(docs) => {
                let root = &docs[0].root;
                assert!(matches!(root.get("list"), Some(Value::Arr(_))));
                assert!(matches!(root.get("map"), Some(Value::Obj(_))));
            }
            other => panic!("{other:?}"),
        }
    }
}
