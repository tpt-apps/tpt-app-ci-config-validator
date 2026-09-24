// Built-in schema rule packs. These are plain `Rule`s (the same structs the
// user-facing custom-rule format uses) plus a few hand-written cross-checks
// for things a path selector cannot express (OR-shaped requirements, label
// overlap). The packs fire only when the document looks like their kind —
// every rule is gated by a `when` on the document's own markers.

use crate::cond::Cond;
use crate::finding::{Finding, Severity};
use crate::rules::{Rule, RuleSet, When};
use crate::select::Selector;
use crate::value::{Document, Format, Value};

fn rule(id: &str, description: &str, severity: Severity, select: &str, cond: Cond) -> Rule {
    Rule {
        id: id.into(),
        description: description.into(),
        severity,
        select: Selector::parse(select).expect("built-in selector must parse"),
        when: None,
        cond,
    }
}

fn gated(id: &str, description: &str, severity: Severity, select: &str, cond: Cond, when: When) -> Rule {
    Rule {
        id: id.into(),
        description: description.into(),
        severity,
        select: Selector::parse(select).expect("built-in selector must parse"),
        when: Some(when),
        cond,
    }
}

fn when_kind(kind: &str) -> When {
    When {
        select: Selector::parse("kind").expect("static selector must parse"),
        cond: Cond::Equals(Value::from(kind)),
    }
}

fn when_kind_exists() -> When {
    When {
        select: Selector::parse("kind").expect("static selector must parse"),
        cond: Cond::Exists,
    }
}

const DNS_1123: &str = r"^[a-z0-9]([-a-z0-9]*[a-z0-9])?$";
const IMAGE_TAGGED: &str = r"(:[A-Za-z0-9._-]+|@sha256:[a-f0-9]{12,64})$";

/// Kubernetes manifests (any doc carrying `kind`). Coverage: the fields the
/// app's audience most commonly get wrong — required metadata/selector/
/// template plumbing on Deployments and Services, replica typing, image
/// pinning, and name hygiene.
pub fn kubernetes_rules() -> RuleSet {
    let rules = vec![
        // Any manifest.
        rule(
            "k8s-api-version",
            "Kubernetes manifests need `apiVersion`",
            Severity::Error,
            "apiVersion",
            Cond::Exists,
        ),
        rule(
            "k8s-kind",
            "Kubernetes manifests need `kind`",
            Severity::Error,
            "kind",
            Cond::Exists,
        ),
        gated(
            "k8s-kind-known",
            "`kind` should be a known workload/config kind (set `severity: info` in a custom rule to silence)",
            Severity::Warning,
            "kind",
            Cond::OneOf(vec![
                "Pod".into(), "Deployment".into(), "StatefulSet".into(), "DaemonSet".into(),
                "Job".into(), "CronJob".into(), "ReplicaSet".into(), "Service".into(),
                "ConfigMap".into(), "Secret".into(), "Ingress".into(), "PersistentVolume".into(),
                "PersistentVolumeClaim".into(), "ServiceAccount".into(), "Role".into(),
                "RoleBinding".into(), "ClusterRole".into(), "ClusterRoleBinding".into(),
                "Namespace".into(), "HorizontalPodAutoscaler".into(), "NetworkPolicy".into(),
            ]),
            when_kind_exists(),
        ),
        gated(
            "k8s-metadata-name",
            "Manifests need `metadata.name`",
            Severity::Error,
            "metadata.name",
            Cond::Exists,
            when_kind_exists(),
        ),
        gated(
            "k8s-name-dns",
            "`metadata.name` must be DNS-1123 compliant (lowercase alphanumerics and `-`)",
            Severity::Error,
            "metadata.name",
            Cond::Matches(DNS_1123.into()),
            when_kind_exists(),
        ),
        // Deployments (apps/v1).
        gated(
            "k8s-selector-required",
            "apps/v1 Deployments require `spec.selector.matchLabels`",
            Severity::Error,
            "spec.selector.matchLabels",
            Cond::Exists,
            when_kind("Deployment"),
        ),
        gated(
            "k8s-template-labels-required",
            "Deployments require `spec.template.metadata.labels` (must match the selector)",
            Severity::Error,
            "spec.template.metadata.labels",
            Cond::Exists,
            when_kind("Deployment"),
        ),
        gated(
            "k8s-containers-required",
            "Deployments require a non-empty `spec.template.spec.containers` list",
            Severity::Error,
            "spec.template.spec.containers",
            Cond::IsA("array".into()),
            when_kind("Deployment"),
        ),
        gated(
            "k8s-container-name",
            "Every container needs a `name`",
            Severity::Error,
            "spec.template.spec.containers[*].name",
            Cond::Exists,
            when_kind("Deployment"),
        ),
        gated(
            "k8s-container-image",
            "Every container needs an `image`",
            Severity::Error,
            "spec.template.spec.containers[*].image",
            Cond::Exists,
            when_kind("Deployment"),
        ),
        gated(
            "k8s-image-tagged",
            "Container images should pin an explicit tag or digest, not float",
            Severity::Warning,
            "spec.template.spec.containers[*].image",
            Cond::Matches(IMAGE_TAGGED.into()),
            when_kind("Deployment"),
        ),
        gated(
            "k8s-replicas-number",
            "`spec.replicas` must be a number",
            Severity::Error,
            "spec.replicas",
            Cond::IsA("number".into()),
            when_kind("Deployment"),
        ),
        gated(
            "k8s-replicas-non-negative",
            "`spec.replicas` must be >= 0",
            Severity::Error,
            "spec.replicas",
            Cond::Gte(0.0),
            when_kind("Deployment"),
        ),
        // Services.
        gated(
            "k8s-service-ports",
            "Services require `spec.ports`",
            Severity::Error,
            "spec.ports",
            Cond::Exists,
            when_kind("Service"),
        ),
        gated(
            "k8s-service-port-range",
            "Service `port` must be 1-65535",
            Severity::Error,
            "spec.ports[*].port",
            Cond::Gte(1.0),
            when_kind("Service"),
        ),
        gated(
            "k8s-service-port-max",
            "Service `port` must be 1-65535",
            Severity::Error,
            "spec.ports[*].port",
            Cond::Lte(65535.0),
            when_kind("Service"),
        ),
        gated(
            "k8s-service-type",
            "`spec.type` must be one of ClusterIP, NodePort, LoadBalancer, ExternalName",
            Severity::Error,
            "spec.type",
            Cond::OneOf(vec![
                "ClusterIP".into(), "NodePort".into(), "LoadBalancer".into(), "ExternalName".into(),
            ]),
            when_kind("Service"),
        ),
    ];
    RuleSet::from_rules("Kubernetes", rules)
}

/// GitHub Actions workflows (any doc with a top-level `jobs` object).
pub fn github_actions_rules() -> RuleSet {
    let rules = vec![
        rule(
            "gha-jobs-required",
            "Workflows need a `jobs` map",
            Severity::Error,
            "jobs",
            Cond::Exists,
        ),
        rule(
            "gha-jobs-object",
            "`jobs` must be a map of job id → job",
            Severity::Error,
            "jobs",
            Cond::IsA("object".into()),
        ),
        gated(
            "gha-runs-on",
            "Every run job needs `runs-on`",
            Severity::Error,
            "jobs[*].runs-on",
            Cond::Exists,
            When {
                select: Selector::parse("jobs[*].uses").expect("static selector must parse"),
                cond: Cond::Missing,
            },
        ),
        gated(
            "gha-steps-required",
            "Every run job needs a `steps` list",
            Severity::Error,
            "jobs[*].steps",
            Cond::Exists,
            When {
                select: Selector::parse("jobs[*].uses").expect("static selector must parse"),
                cond: Cond::Missing,
            },
        ),
        gated(
            "gha-steps-array",
            "`steps` must be a list",
            Severity::Error,
            "jobs[*].steps",
            Cond::IsA("array".into()),
            When {
                select: Selector::parse("jobs[*].uses").expect("static selector must parse"),
                cond: Cond::Missing,
            },
        ),
    ];
    RuleSet::from_rules("GitHub Actions", rules)
}

/// Cross-checks a path selector cannot express. These are hand-written and
/// run for matching documents after the DSL rules.
pub fn extra_checks(doc: &Document) -> Vec<Finding> {
    let mut findings = Vec::new();
    match doc.format {
        Format::Yaml | Format::Json => {
            k8s_label_overlap(doc, &mut findings);
            gha_step_actions(doc, &mut findings);
        }
        _ => {}
    }
    findings
}

/// apps/v1 Deployments: selector.matchLabels must overlap (or be a subset
/// relationship — selector labels ⊆ template labels).
fn k8s_label_overlap(doc: &Document, findings: &mut Vec<Finding>) {
    let is_deploy = doc.root.get("kind").and_then(Value::as_str) == Some("Deployment");
    if !is_deploy {
        return;
    }
    let selector = doc
        .root
        .get("spec")
        .and_then(|s| s.get("selector"))
        .and_then(|s| s.get("matchLabels"));
    let template_labels = doc
        .root
        .get("spec")
        .and_then(|s| s.get("template"))
        .and_then(|t| t.get("metadata"))
        .and_then(|m| m.get("labels"));
    let (Some(Value::Obj(selector)), Some(Value::Obj(labels))) = (selector, template_labels) else {
        return; // missing sections are the DSL rules' job
    };
    for key in selector.keys() {
        if !labels.contains_key(key) {
            findings.push(Finding::new(
                "k8s-selector-overlap",
                Severity::Error,
                "spec.template.metadata.labels",
                format!(
                    "selector label `{key}` is missing from `spec.template.metadata.labels` — no pods would match this Deployment"
                ),
            ));
        }
    }
}

/// GitHub Actions: every step needs `run` or `uses` (OR-shaped; the DSL
/// cannot say "one of these two keys" per step).
fn gha_step_actions(doc: &Document, findings: &mut Vec<Finding>) {
    let Some(Value::Obj(jobs)) = doc.root.get("jobs") else {
        return;
    };
    for (job_id, job) in jobs {
        let Value::Obj(job) = job else {
            continue;
        };
        // Reusable-workflow call jobs have no steps by design.
        if job.contains_key("uses") {
            continue;
        }
        let Some(Value::Arr(steps)) = job.get("steps") else {
            continue;
        };
        for (i, step) in steps.iter().enumerate() {
            let Value::Obj(step) = step else {
                continue;
            };
            if !step.contains_key("run") && !step.contains_key("uses") {
                findings.push(Finding::new(
                    "gha-step-action",
                    Severity::Error,
                    format!("jobs.{job_id}.steps[{i}]"),
                    format!("step {i} of job `{job_id}` needs a `run` or `uses` key"),
                ));
            }
        }
    }
}
