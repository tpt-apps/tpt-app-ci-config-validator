// TPT CI Config Validator — validation engine.
//
// Target-independent (host + wasm32): config file(s) + schema/rule set in,
// validation findings out as plain structs. No UI deps, host-unit-testable.
//
// Shape: parser adapters (`parse`) convert YAML/JSON/XML/HCL text into the
// normalized `Value` tree; rules (`rules`, `builtin`) are path selectors +
// conditions over that tree; `validate` runs them and returns `Report`s.

pub mod builtin;
pub mod cond;
pub mod finding;
pub mod rules;
pub mod select;
pub mod tf_lint;
pub mod value;
pub mod validate;

pub use finding::{Finding, Report, Severity};
pub use rules::{Rule, RuleError, RuleSet};
pub use value::{detect_format, Document, Format, Value};
pub use validate::{validate, validate_batch};
