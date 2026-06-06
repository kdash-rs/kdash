//! PVC troubleshoot checks for unhealthy phases.
//! Ref: <https://kubernetes.io/docs/reference/generated/kubernetes-api/v1.35/#persistentvolumeclaimstatus-v1-core>

use crate::app::{models::KubeResource, pvcs::KubePVC};

use super::{DisplayFinding, ResourceKind, Severity};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// PVC phase or "Unknown".
fn phase(pvc: &KubePVC) -> &str {
  pvc
    .get_k8s_obj()
    .status
    .as_ref()
    .and_then(|s| s.phase.as_deref())
    .unwrap_or("Unknown")
}

fn finding(pvc: &KubePVC, severity: Severity, reason: String, message: String) -> DisplayFinding {
  DisplayFinding {
    severity,
    reason,
    resource_kind: ResourceKind::Pvc,
    namespace: Some(pvc.namespace.clone()),
    resource_name: pvc.name.clone(),
    message,
    age: pvc.age.clone(),
  }
}

// ---------------------------------------------------------------------------
// Individual PVC checks
// ---------------------------------------------------------------------------

/// Flag non-Bound phase.
fn check_phase(pvc: &KubePVC) -> Option<DisplayFinding> {
  let current_phase = phase(pvc);

  if current_phase == "Bound" {
    return None;
  }

  Some(finding(
    pvc,
    Severity::Warn,
    current_phase.into(),
    format!("PVC phase is {}", current_phase),
  ))
}

// ---------------------------------------------------------------------------
// PVC evaluation entry point
// ---------------------------------------------------------------------------

/// Run all PVC checks and collect findings.
pub fn evaluate(items: &[KubePVC]) -> Vec<DisplayFinding> {
  items.iter().filter_map(check_phase).collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use k8s_openapi::api::core::v1::{PersistentVolumeClaim, PersistentVolumeClaimStatus};
  use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

  use crate::app::test_utils::get_time;

  fn build_pvc(phase: Option<&str>) -> KubePVC {
    let pvc = PersistentVolumeClaim {
      metadata: ObjectMeta {
        name: Some("pvc-1".into()),
        namespace: Some("ns-1".into()),
        creation_timestamp: Some(get_time("2026-01-01T00:00:00Z")),
        ..Default::default()
      },
      status: phase.map(|p| PersistentVolumeClaimStatus {
        phase: Some(p.to_string()),
        ..Default::default()
      }),
      ..Default::default()
    };

    KubePVC::from(pvc)
  }

  #[test]
  fn test_pvc_phase_fallback() {
    let pvc = build_pvc(None);
    assert_eq!(phase(&pvc), "Unknown");
  }

  #[test]
  fn test_check_pvc_phase_bound_is_none() {
    let pvc = build_pvc(Some("Bound"));
    assert!(check_phase(&pvc).is_none());
  }
}
