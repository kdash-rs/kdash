//! PVC troubleshoot checks for unhealthy phases.
//! Ref: <https://kubernetes.io/docs/reference/generated/kubernetes-api/v1.35/#persistentvolumeclaimstatus-v1-core>

use crate::app::{models::KubeResource, pvcs::KubePVC};

use super::{Diagnostic, Finding, HealthCheck, RawFinding, ResourceKind};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// PVC phase or "Unknown".
fn pvc_phase(pvc: &KubePVC) -> &str {
  pvc
    .get_k8s_obj()
    .status
    .as_ref()
    .and_then(|s| s.phase.as_deref())
    .unwrap_or("Unknown")
}

impl Diagnostic for KubePVC {
  fn resource_kind(&self) -> ResourceKind {
    ResourceKind::Pvc
  }

  fn describe_kind(&self) -> String {
    "persistentvolumeclaim".into()
  }

  fn name(&self) -> &str {
    &self.name
  }
  fn namespace(&self) -> Option<&str> {
    Some(&self.namespace)
  }
  fn age(&self) -> &str {
    &self.age
  }
}

// ---------------------------------------------------------------------------
// Individual PVC checks
// ---------------------------------------------------------------------------

/// Flag non-Bound phase.
fn check_pvc_phase(pvc: &KubePVC) -> Option<Finding<RawFinding>> {
  let phase = pvc_phase(pvc);

  if phase == "Bound" {
    return None;
  }

  Some(Finding::Warn(RawFinding {
    id: "pvc.phase.not_bound".into(),
    reason: phase.into(),
    message: format!("PVC phase is {}", phase),
  }))
}

// ---------------------------------------------------------------------------
// Registry of all PVC checks
// ---------------------------------------------------------------------------

/// Returns all registered PVC checks. Add new checks here.
pub fn all_pvc_checks() -> Vec<HealthCheck<KubePVC>> {
  vec![check_pvc_phase]
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
    assert_eq!(pvc_phase(&pvc), "Unknown");
  }

  #[test]
  fn test_check_pvc_phase_bound_is_none() {
    let pvc = build_pvc(Some("Bound"));
    assert!(check_pvc_phase(&pvc).is_none());
  }
}
