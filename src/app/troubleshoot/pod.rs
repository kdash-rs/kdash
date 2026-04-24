//! Pod troubleshoot checks for unhealthy phases.
//! Ref: <https://kubernetes.io/docs/reference/generated/kubernetes-api/v1.35/#podstatus-v1-core>

use k8s_openapi::api::core::v1::PodCondition;

use crate::app::{models::KubeResource, pods::KubePod};

use super::{DisplayFinding, ResourceKind, Severity};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Pod phase or "Unknown".
fn phase(pod: &KubePod) -> &str {
  pod
    .get_k8s_obj()
    .status
    .as_ref()
    .and_then(|s| s.phase.as_deref())
    .unwrap_or("Unknown")
}

/// Newest condition by `last_transition_time`.
fn latest_condition(pod: &KubePod) -> Option<&PodCondition> {
  pod
    .get_k8s_obj()
    .status
    .as_ref()
    .and_then(|s| s.conditions.as_deref())
    .and_then(|c| c.iter().max_by_key(|cond| &cond.last_transition_time))
}

/// Extract (reason, message) from the newest condition, defaulting to "N/A".
fn condition_summary(pod: &KubePod) -> (String, String) {
  match latest_condition(pod) {
    Some(c) => (
      c.reason.clone().unwrap_or_else(|| "N/A".into()),
      c.message.clone().unwrap_or_else(|| "N/A".into()),
    ),
    None => ("N/A".into(), "N/A".into()),
  }
}

fn finding(pod: &KubePod, severity: Severity, reason: String, message: String) -> DisplayFinding {
  DisplayFinding {
    severity,
    reason,
    resource_kind: ResourceKind::Pod,
    namespace: Some(pod.namespace.clone()),
    resource_name: pod.name.clone(),
    message,
    age: pod.age.clone(),
  }
}

// ---------------------------------------------------------------------------
// Individual pod checks
// ---------------------------------------------------------------------------

/// Flag Failed/Unknown/Pending phases.
/// Ref: <https://kubernetes.io/docs/concepts/workloads/pods/pod-lifecycle/#pod-phase>
fn check_phase(pod: &KubePod) -> Option<DisplayFinding> {
  let current_phase = phase(pod);

  let severity = match current_phase {
    "Failed" => Severity::Error,
    "Unknown" => Severity::Warn,
    "Pending" => Severity::Info,
    _ => return None,
  };

  let (reason, message) = condition_summary(pod);
  Some(finding(pod, severity, reason, message))
}

// ---------------------------------------------------------------------------
// Evaluation entry point
// ---------------------------------------------------------------------------

/// Run all pod checks and collect findings.
pub fn evaluate(items: &[KubePod]) -> Vec<DisplayFinding> {
  items.iter().filter_map(check_phase).collect()
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
    assert_eq!(phase(&pod_unknown), "Unknown");

    let pod_running = build_pod(Some("Running"), vec![]);
    assert_eq!(phase(&pod_running), "Running");
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
