// Findings: what the engine reports for each violated rule.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }

    pub fn parse(s: &str) -> Option<Severity> {
        match s.to_ascii_lowercase().as_str() {
            "error" | "err" => Some(Severity::Error),
            "warning" | "warn" => Some(Severity::Warning),
            "info" => Some(Severity::Info),
            _ => None,
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// One rule violation. `path` is the concrete location in the document
/// (`spec.template.spec.containers[1].image`), empty for document-level
/// findings like "missing spec.selector".
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub rule_id: String,
    pub severity: Severity,
    pub path: String,
    pub message: String,
}

impl Finding {
    pub fn new(rule_id: impl Into<String>, severity: Severity, path: impl Into<String>, message: impl Into<String>) -> Self {
        Finding {
            rule_id: rule_id.into(),
            severity,
            path: path.into(),
            message: message.into(),
        }
    }
}

/// Outcome for one document: findings plus whether parsing itself failed.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub document: String,
    pub format: crate::value::Format,
    /// Parse failure (syntax error). When set, rule findings are not produced.
    pub parse_error: Option<String>,
    pub findings: Vec<Finding>,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.parse_error.is_none()
            && self.findings.iter().all(|f| f.severity != Severity::Error)
    }

    pub fn error_count(&self) -> usize {
        self.findings.iter().filter(|f| f.severity == Severity::Error).count()
    }

    pub fn warning_count(&self) -> usize {
        self.findings.iter().filter(|f| f.severity == Severity::Warning).count()
    }
}
