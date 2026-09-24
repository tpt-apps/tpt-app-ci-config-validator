// TPT CI Config Validator — validation engine.
//
// Target-independent (host + wasm32): config file(s) + schema/rule set in,
// validation findings out as plain structs. No UI deps, host-unit-testable.
// Phase 1 fills this in: YAML/JSON/XML parsing adapters over the tpt-*
// base crates, built-in schema rules for Kubernetes/Terraform/CI configs,
// and the custom-rule format.

#![deny(rustdoc::broken_intra_doc_links)]
