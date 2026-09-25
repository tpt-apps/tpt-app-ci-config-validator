// Path selectors: a small, readable subset of JSONPath —
// `spec.template.spec.containers[*].image`, `jobs[*].steps[0]`.
// Keys may not contain dots (config schemas we target never do); array
// access is `[*]` or `[N]`. The selector engine is total: it never panics
// on unexpected shapes and simply stops descending.

use crate::value::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum Seg {
    Key(String),
    Index(usize),
    Wildcard,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Selector {
    segs: Vec<Seg>,
}

/// A match: the concrete path it was found at plus the value.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub path: String,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectError(pub String);

impl std::fmt::Display for SelectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SelectError {}

impl Selector {
    pub fn parse(path: &str) -> Result<Selector, SelectError> {        let path = path.trim();
        if path.is_empty() {
            return Err(SelectError("empty selector".into()));
        }
        let mut segs = Vec::new();
        for raw in path.split('.') {
            let raw = raw.trim();
            if raw.is_empty() {
                return Err(SelectError(format!("empty segment in `{path}`")));
            }
            // Split a trailing index/wildcard off a key: `containers[*]`,
            // `steps[0]`.
            let (head, bracket) = match raw.find('[') {
                Some(i) => (&raw[..i], Some(&raw[i..])),
                None => (raw, None),
            };
            if !head.is_empty() {
                segs.push(Seg::Key(head.to_string()));
            } else if bracket.is_none() {
                return Err(SelectError(format!("empty segment in `{path}`")));
            }
            if let Some(bracket) = bracket {
                if !bracket.ends_with(']') || bracket.len() < 3 {
                    return Err(SelectError(format!("bad bracket `{bracket}` in `{path}`")));
                }
                let inner = &bracket[1..bracket.len() - 1];
                if inner == "*" {
                    segs.push(Seg::Wildcard);
                } else if let Ok(i) = inner.parse::<usize>() {
                    segs.push(Seg::Index(i));
                } else {
                    return Err(SelectError(format!("bad index `{inner}` in `{path}`")));
                }
            }
        }
        Ok(Selector { segs })
    }

    /// Evaluate against a document root. Order: document order, wildcards
    /// expanding innermost-first.
    pub fn eval(&self, root: &Value) -> Vec<Hit> {
        let mut hits = Vec::new();
        self.walk(root, "", &self.segs, &mut hits);
        hits
    }


    fn walk(&self, node: &Value, path: &str, segs: &[Seg], out: &mut Vec<Hit>) {
        match segs.first() {
            None => out.push(Hit {
                path: path.to_string(),
                value: node.clone(),
            }),
            Some(Seg::Key(key)) => {
                match node {
                    Value::Obj(_) => {
                        if let Some(child) = node.get(key) {
                            let child_path = if path.is_empty() {
                                key.clone()
                            } else {
                                format!("{path}.{key}")
                            };
                            self.walk(child, &child_path, &segs[1..], out);
                        }
                    }
                    // A key applied to an array maps over the items. This is
                    // what makes XML rules uniform: `<dependency>` may parse
                    // to an object (one occurrence) or an array (several),
                    // and `dependency.artifactId` should read the same in
                    // both shapes.
                    Value::Arr(items) => {
                        for (i, child) in items.iter().enumerate() {
                            if let Some(grandchild) = child.get(key) {
                                let child_path = if path.is_empty() {
                                    key.clone()
                                } else {
                                    format!("{path}.{key}")
                                };
                                self.walk(
                                    grandchild,
                                    &format!("{child_path}[{i}]"),
                                    &segs[1..],
                                    out,
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
            Some(Seg::Index(i)) => {
                if let Value::Arr(items) = node {
                    if let Some(child) = items.get(*i) {
                        self.walk(child, &format!("{path}[{i}]"), &segs[1..], out);
                    }
                }
            }
            Some(Seg::Wildcard) => {
                // A wildcard under an object walks its values; under an
                // array, its items. A missing/degenerate node matches nothing.
                match node {
                    Value::Arr(items) => {
                        for (i, child) in items.iter().enumerate() {
                            self.walk(child, &format!("{path}[{i}]"), &segs[1..], out);
                        }
                    }
                    Value::Obj(map) => {
                        for (key, child) in map {
                            let child_path = if path.is_empty() {
                                key.clone()
                            } else {
                                format!("{path}.{key}")
                            };
                            self.walk(child, &child_path, &segs[1..], out);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

impl std::fmt::Display for Selector {
    /// Reconstructs the authored form: `spec.template.spec.containers[*].image`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for seg in &self.segs {
            match seg {
                Seg::Key(key) => {
                    if !first {
                        f.write_str(".")?;
                    }
                    write!(f, "{key}")?;
                    first = false;
                }
                Seg::Index(i) => write!(f, "[{i}]")?,
                Seg::Wildcard => write!(f, "[*]")?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn sample() -> Value {
        let container = |name: &str, image: &str| {
            let mut m = BTreeMap::new();
            m.insert("name".to_string(), Value::from(name));
            m.insert("image".to_string(), Value::from(image));
            Value::Obj(m)
        };
        let mut containers = BTreeMap::new();
        containers.insert("containers".to_string(), Value::Arr(vec![
            container("api", "api:1.0"),
            container("sidecar", "proxy:2.0"),
        ]));
        let mut spec = BTreeMap::new();
        spec.insert("template".to_string(), {
            let mut t = BTreeMap::new();
            t.insert("spec".to_string(), Value::Obj(containers));
            Value::Obj(t)
        });
        let mut root = BTreeMap::new();
        root.insert("spec".to_string(), Value::Obj(spec));
        Value::Obj(root)
    }

    #[test]
    fn wildcard_collects_all_images() {
        let sel = Selector::parse("spec.template.spec.containers[*].image").unwrap();
        let hits = sel.eval(&sample());
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].path, "spec.template.spec.containers[0].image");
        assert_eq!(hits[1].value.as_str(), Some("proxy:2.0"));
    }

    #[test]
    fn index_selects_one() {
        let sel = Selector::parse("spec.template.spec.containers[1].name").unwrap();
        let hits = sel.eval(&sample());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].value.as_str(), Some("sidecar"));
    }

    #[test]
    fn missing_path_is_no_hits_not_error() {
        let sel = Selector::parse("spec.replicas").unwrap();
        assert!(sel.eval(&sample()).is_empty());
    }

    #[test]
    fn bad_selectors_rejected() {
        assert!(Selector::parse("").is_err());
        assert!(Selector::parse("a..b").is_err());
        assert!(Selector::parse("a[x]").is_err());
        assert!(Selector::parse("a[*").is_err());
    }

    #[test]
    fn wildcard_over_map_walks_values() {
        let sel = Selector::parse("jobs[*].runs-on").unwrap();
        let mut job_a = BTreeMap::new();
        job_a.insert("runs-on".to_string(), Value::from("ubuntu-latest"));
        let mut jobs = BTreeMap::new();
        jobs.insert("test".to_string(), Value::Obj(job_a));
        let mut root = BTreeMap::new();
        root.insert("jobs".to_string(), Value::Obj(jobs));
        let hits = sel.eval(&Value::Obj(root));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "jobs.test.runs-on");
    }
}
