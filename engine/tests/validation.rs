// Fixture-driven validation: real-world-shaped Kubernetes, GitHub Actions,
// Terraform, and Maven configs, valid and deliberately broken. These run
// the whole pipeline (detect → parse → packs → checks) exactly as the UI
// and CLI do.

use tpt_config_engine::{validate_source, Report, Severity};

fn load(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {name} missing at {}: {e}", path.display()))
}

fn errors(r: &Report) -> Vec<&tpt_config_engine::Finding> {
    r.findings.iter().filter(|f| f.severity == Severity::Error).collect()
}

fn warnings(r: &Report) -> Vec<&tpt_config_engine::Finding> {
    r.findings.iter().filter(|f| f.severity == Severity::Warning).collect()
}

#[test]
fn valid_k8s_deployment_passes_clean() {
    let reports = validate_source("k8s-valid-deployment.yaml", &load("k8s-valid-deployment.yaml"), None);
    assert_eq!(reports.len(), 1);
    let r = &reports[0];
    assert!(r.parse_error.is_none(), "{:?}", r.parse_error);
    assert!(r.passed(), "{:?}", r.findings);
    // The pin-your-images warning must not fire on a fully pinned manifest.
    assert!(warnings(r).is_empty(), "{:?}", warnings(r));
}

#[test]
fn broken_k8s_deployment_reports_each_defect() {
    let reports = validate_source("k8s-broken-deployment.yaml", &load("k8s-broken-deployment.yaml"), None);
    let r = &reports[0];
    assert!(r.parse_error.is_none(), "{:?}", r.parse_error);
    let errs = errors(r);
    let ids: Vec<&str> = errs.iter().map(|f| f.rule_id.as_str()).collect();

    assert!(ids.contains(&"k8s-name-dns"), "{ids:?}");
    assert!(ids.contains(&"k8s-replicas-number"), "{ids:?}");
    assert!(ids.contains(&"k8s-selector-required"), "{ids:?}");
    assert!(ids.contains(&"k8s-container-image"), "{ids:?}");
    assert!(ids.contains(&"k8s-image-floating"), "{ids:?}");
    // The :latest image is explicitly tagged, so the "pin a tag" warning
    // still passes; the floating error is the finding.
    assert!(ids.contains(&"k8s-selector-required"));
    assert_eq!(r.error_count(), errs.len());
}

#[test]
fn valid_gha_workflow_passes_clean() {
    let reports = validate_source("gha-valid-workflow.yml", &load("gha-valid-workflow.yml"), None);
    let r = &reports[0];
    assert!(r.parse_error.is_none(), "{:?}", r.parse_error);
    assert!(r.passed(), "{:?}", r.findings);
}

#[test]
fn broken_gha_workflow_reports_per_job() {
    let reports = validate_source("gha-broken-workflow.yml", &load("gha-broken-workflow.yml"), None);
    let r = &reports[0];
    assert!(r.parse_error.is_none(), "{:?}", r.parse_error);
    let errs = errors(r);
    let ids: Vec<&str> = errs.iter().map(|f| f.rule_id.as_str()).collect();

    // job `test` lacks runs-on; job `lint` has a step with no run/uses.
    assert!(ids.contains(&"gha-runs-on"), "{ids:?}");
    assert!(ids.contains(&"gha-step-action"), "{ids:?}");
    let runs_on = errs.iter().find(|f| f.rule_id == "gha-runs-on").unwrap();
    assert_eq!(runs_on.path, "jobs.test.runs-on");
}

#[test]
fn valid_terraform_passes_clean() {
    let reports = validate_source("tf-valid-main.tf", &load("tf-valid-main.tf"), None);
    let r = &reports[0];
    assert!(r.parse_error.is_none(), "{:?}", r.parse_error);
    assert!(r.passed(), "{:?}", r.findings);
    assert!(warnings(r).is_empty(), "{:?}", warnings(r));
}

#[test]
fn broken_terraform_reports_balance_duplicates_and_secrets() {
    let reports = validate_source("tf-broken-main.tf", &load("tf-broken-main.tf"), None);
    let r = &reports[0];
    let errs = errors(r);
    let ids: Vec<&str> = errs.iter().map(|f| f.rule_id.as_str()).collect();

    assert!(ids.contains(&"tf-brace-balance"), "{ids:?}");
    assert!(ids.contains(&"tf-duplicate-resource"), "{ids:?}");
    let warns = warnings(r);
    assert!(warns.iter().any(|f| f.rule_id == "tf-hardcoded-secret"), "{warns:?}");
}

#[test]
fn valid_pom_passes_clean() {
    let reports = validate_source("xml-valid-pom.xml", &load("xml-valid-pom.xml"), None);
    let r = &reports[0];
    assert!(r.parse_error.is_none(), "{:?}", r.parse_error);
    assert!(r.passed(), "{:?}", r.findings);
}

#[test]
fn broken_pom_is_a_parse_error() {
    let reports = validate_source("xml-broken-pom.xml", &load("xml-broken-pom.xml"), None);
    let r = &reports[0];
    assert!(r.parse_error.is_some(), "expected a parse error, got {:?}", r.findings);
    assert!(!r.passed());
}

#[test]
fn unknown_yaml_still_checks_no_pack_without_failing() {
    // A generic YAML doc that is neither k8s nor a workflow: no built-in
    // pack applies, parsing succeeds, nothing fires.
    let reports = validate_source("misc.yaml", "service:\n  port: 8080\n", None);
    let r = &reports[0];
    assert!(r.parse_error.is_none());
    assert!(r.findings.is_empty(), "{:?}", r.findings);
}

#[test]
fn malformed_yaml_reports_the_parse_error() {
    let reports = validate_source("bad.yaml", "a:\n\tb: 1\n", None);
    assert!(reports[0].parse_error.is_some());
}

#[test]
fn custom_rules_run_alongside_builtins() {
    let rules_text = r#"
name: team rules
rules:
  - id: team-label
    description: Every manifest declares an environment label
    severity: warning
    when:
      select: kind
      condition: exists
    select: metadata.labels.environment
    condition: exists
"#;
    let rules_doc = match tpt_config_engine::parse::parse_source("rules.yaml", rules_text) {
        tpt_config_engine::parse::Parsed::Docs(mut docs) => docs.remove(0),
        tpt_config_engine::parse::Parsed::Failed(e) => panic!("rule file failed to parse: {e}"),
    };
    let (set, errs) = tpt_config_engine::rules::rule_set_from_value(&rules_doc.root).unwrap();
    assert!(errs.is_empty(), "{errs:?}");

    let reports = validate_source(
        "k8s-valid-deployment.yaml",
        &load("k8s-valid-deployment.yaml"),
        Some(&set),
    );
    // The valid fixture has no `team` label → custom warning fires; the
    // built-in pack still passes it otherwise.
    let r = &reports[0];
    assert!(r.findings.iter().any(|f| f.rule_id == "team-label" && f.severity == Severity::Warning));
    assert_eq!(r.error_count(), 0);
}

#[test]
fn multi_doc_yaml_validates_each_document() {
    let text = "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: ok-config\n---\napiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: Bad_Name\n";
    let reports = validate_source("multi.yaml", text, None);
    assert_eq!(reports.len(), 2);
    assert!(reports[0].passed(), "{:?}", reports[0].findings);
    assert!(!reports[1].passed());
    assert_eq!(reports[1].document, "multi.yaml#2");
    assert!(reports[1].findings.iter().any(|f| f.rule_id == "k8s-name-dns"));
}
