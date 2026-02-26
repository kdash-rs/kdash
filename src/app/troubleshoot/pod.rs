//! Pod troubleshoot checks for unhealthy phases.
//! Ref: <https://kubernetes.io/docs/reference/generated/kubernetes-api/v1.35/#podstatus-v1-core>

use k8s_openapi::api::core::v1::PodCondition;

use crate::app::{models::KubeResource, pods::KubePod};

use super::{DisplayFinding, Finding, IntoDisplayFinding, ResourceKind};

// ---------------------------------------------------------------------------
// PodFinding — resource-specific finding data for pods
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct PodFinding {
  pub id: String,
  pub reason: String,
  pub namespace: String,
  pub pod_name: String,
  pub message: String,
  pub age: String,
}

// ---------------------------------------------------------------------------
// Finding<PodFinding> → DisplayFinding conversion
// ---------------------------------------------------------------------------

impl IntoDisplayFinding for Finding<PodFinding> {
  fn into_display_finding(self) -> DisplayFinding {
    let severity = self.severity_tag();
    let inner = self.into_inner();
    DisplayFinding {
      severity,
      reason: inner.reason,
      resource_kind: ResourceKind::Pod,
      namespace: Some(inner.namespace.clone()),
      resource_name: inner.pod_name.clone(),
      message: inner.message,
      age: inner.age,
      describe_kind: "pod".into(),
      describe_name: inner.pod_name,
      describe_namespace: Some(inner.namespace),
      k8s_obj: (),
    }
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Pod phase or "Unknown".
fn pod_phase(pod: &KubePod) -> &str {
  pod
    .get_k8s_obj()
    .status
    .as_ref()
    .and_then(|s| s.phase.as_deref())
    .unwrap_or("Unknown")
}

/// Newest condition by `last_transition_time`.
fn latest_condition(pod: &KubePod) -> Option<&PodCondition> {
  let mut conditions: Vec<&PodCondition> = pod
    .get_k8s_obj()
    .status
    .as_ref()
    .and_then(|s| s.conditions.as_ref())
    .map(|c| c.iter().collect())
    .unwrap_or_default();

  conditions.sort_by(|a, b| b.last_transition_time.cmp(&a.last_transition_time));

  conditions.into_iter().next()
}

/// Latest condition reason or "N/A".
fn pod_status_reason(pod: &KubePod) -> String {
  latest_condition(pod)
    .and_then(|c| c.reason.as_deref())
    .unwrap_or("N/A")
    .into()
}

/// Latest condition message or "N/A".
fn pod_status_message(pod: &KubePod) -> String {
  latest_condition(pod)
    .and_then(|c| c.message.as_deref())
    .unwrap_or("N/A")
    .into()
}

// ---------------------------------------------------------------------------
// Check type alias
// ---------------------------------------------------------------------------

/// Check a pod; optionally returns a finding.
pub type PodCheck = fn(&KubePod) -> Option<Finding<PodFinding>>;

// ---------------------------------------------------------------------------
// Individual pod checks
// ---------------------------------------------------------------------------

/// Flag Failed/Unknown/Pending phases.
/// Ref: <https://kubernetes.io/docs/concepts/workloads/pods/pod-lifecycle/#pod-phase>
fn check_pod_phase(pod: &KubePod) -> Option<Finding<PodFinding>> {
  let phase = pod_phase(pod);

  let (id, finding_ctor): (&str, fn(PodFinding) -> Finding<PodFinding>) = match phase {
    "Failed" => ("pod.phase.failed", Finding::Error),
    "Unknown" => ("pod.phase.unknown", Finding::Warn),
    "Pending" => ("pod.phase.pending", Finding::Info),
    _ => return None,
  };

  Some(finding_ctor(PodFinding {
    id: id.into(),
    reason: pod_status_reason(pod),
    namespace: pod.namespace.clone(),
    pod_name: pod.name.clone(),
    message: pod_status_message(pod),
    age: pod.age.clone(),
  }))
}

// ---------------------------------------------------------------------------
// Registry of all pod checks
// ---------------------------------------------------------------------------

/// Returns all registered pod checks. Add new checks here.
fn all_pod_checks() -> Vec<PodCheck> {
  vec![check_pod_phase]
}

// ---------------------------------------------------------------------------
// Pod evaluation entry point
// ---------------------------------------------------------------------------

/// Run every registered pod check against every pod and return the flattened
/// display findings.
pub fn evaluate_pod_findings(pods: &[KubePod]) -> Vec<DisplayFinding> {
  let checks = all_pod_checks();

  pods
    .iter()
    .flat_map(|pod| {
      checks
        .iter()
        .filter_map(move |check| check(pod).map(|f| f.into_display_finding()))
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use k8s_openapi::api::core::v1::{Pod, PodCondition, PodStatus};
  use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

  use crate::app::test_utils::get_time;

  fn build_pod(phase: Option<&str>, conditions: Vec<PodCondition>) -> KubePod {
    let pod = Pod {
      metadata: ObjectMeta {
        name: Some("pod-1".into()),
        namespace: Some("ns-1".into()),
        creation_timestamp: Some(get_time("2023-01-01T00:00:00Z")),
        ..Default::default()
      },
      status: Some(PodStatus {
        phase: phase.map(str::to_string),
        conditions: if conditions.is_empty() {
          None
        } else {
          Some(conditions)
        },
        ..Default::default()
      }),
      ..Default::default()
    };

    KubePod::from(pod)
  }

  #[test]
  fn test_pod_phase_fallback_and_value() {
    let pod_unknown = build_pod(None, vec![]);
    assert_eq!(pod_phase(&pod_unknown), "Unknown");

    let pod_running = build_pod(Some("Running"), vec![]);
    assert_eq!(pod_phase(&pod_running), "Running");
  }

  #[test]
  fn test_latest_condition_picks_most_recent() {
    let older = PodCondition {
      last_transition_time: Some(get_time("2026-01-01T00:00:00Z")),
      reason: Some("Older".into()),
      message: Some("Older message".into()),
      ..Default::default()
    };
    let newer = PodCondition {
      last_transition_time: Some(get_time("2026-02-01T00:00:00Z")),
      reason: Some("Newer".into()),
      message: Some("Newer message".into()),
      ..Default::default()
    };

    let pod = build_pod(Some("Running"), vec![older, newer]);
    let latest = latest_condition(&pod).expect("expected a latest condition");

    assert_eq!(latest.reason.as_deref(), Some("Newer"));
    assert_eq!(latest.message.as_deref(), Some("Newer message"));
  }
}
