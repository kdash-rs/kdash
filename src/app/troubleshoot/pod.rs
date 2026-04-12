//! Pod troubleshoot checks for unhealthy phases.
//! Ref: <https://kubernetes.io/docs/reference/generated/kubernetes-api/v1.35/#podstatus-v1-core>

use k8s_openapi::api::core::v1::PodCondition;

use crate::app::{models::KubeResource, pods::KubePod};

use super::{HealthCheck, RawFinding, ResourceKind, Severity};

impl_diagnostic!(KubePod, ResourceKind::Pod);

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
// Individual pod checks
// ---------------------------------------------------------------------------

/// Flag Failed/Unknown/Pending phases.
/// Ref: <https://kubernetes.io/docs/concepts/workloads/pods/pod-lifecycle/#pod-phase>
fn check_pod_phase(pod: &KubePod) -> Option<(Severity, RawFinding)> {
  let phase = pod_phase(pod);

  let severity = match phase {
    "Failed" => Severity::Error,
    "Unknown" => Severity::Warn,
    "Pending" => Severity::Info,
    _ => return None,
  };

  Some((
    severity,
    RawFinding {
      reason: pod_status_reason(pod),
      message: pod_status_message(pod),
    },
  ))
}

// ---------------------------------------------------------------------------
// Registry of all pod checks
// ---------------------------------------------------------------------------

/// Returns all registered pod checks. Add new checks here.
pub fn all_pod_checks() -> Vec<HealthCheck<KubePod>> {
  vec![check_pod_phase]
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
