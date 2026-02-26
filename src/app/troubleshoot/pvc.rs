//! PVC troubleshoot checks for unhealthy phases.
//! Ref: <https://kubernetes.io/docs/reference/generated/kubernetes-api/v1.35/#persistentvolumeclaimstatus-v1-core>

use crate::app::{models::KubeResource, pvcs::KubePVC};

use super::{DisplayFinding, Finding, IntoDisplayFinding, ResourceKind};

// ---------------------------------------------------------------------------
// PvcFinding — resource-specific finding data for PVCs
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct PvcFinding {
  pub id: String,
  pub reason: String,
  pub namespace: String,
  pub pvc_name: String,
  pub message: String,
  pub age: String,
}

// ---------------------------------------------------------------------------
// Finding<PvcFinding> → DisplayFinding conversion
// ---------------------------------------------------------------------------

impl IntoDisplayFinding for Finding<PvcFinding> {
  fn into_display_finding(self) -> DisplayFinding {
    let severity = self.severity_tag();
    let inner = self.into_inner();
    DisplayFinding {
      severity,
      reason: inner.reason,
      resource_kind: ResourceKind::Pvc,
      namespace: Some(inner.namespace.clone()),
      resource_name: inner.pvc_name.clone(),
      message: inner.message,
      age: inner.age,
      describe_kind: "persistentvolumeclaim".into(),
      describe_name: inner.pvc_name,
      describe_namespace: Some(inner.namespace),
      k8s_obj: (),
    }
  }
}

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

// ---------------------------------------------------------------------------
// Check type alias
// ---------------------------------------------------------------------------

/// Check a PVC; optionally returns a finding.
pub type PvcCheck = fn(&KubePVC) -> Option<Finding<PvcFinding>>;

// ---------------------------------------------------------------------------
// Individual PVC checks
// ---------------------------------------------------------------------------

/// Flag non-Bound phase.
fn check_pvc_phase(pvc: &KubePVC) -> Option<Finding<PvcFinding>> {
  let phase = pvc_phase(pvc);

  if phase == "Bound" {
    return None;
  }

  Some(Finding::Warn(PvcFinding {
    id: "pvc.phase.not_bound".into(),
    reason: phase.into(),
    namespace: pvc.namespace.clone(),
    pvc_name: pvc.name.clone(),
    message: format!("PVC phase is {}", phase),
    age: pvc.age.clone(),
  }))
}

// ---------------------------------------------------------------------------
// Registry of all PVC checks
// ---------------------------------------------------------------------------

/// Returns all registered PVC checks. Add new checks here.
fn all_pvc_checks() -> Vec<PvcCheck> {
  vec![check_pvc_phase]
}

// ---------------------------------------------------------------------------
// PVC evaluation entry point
// ---------------------------------------------------------------------------

/// Run every registered PVC check against every PVC and return the flattened
/// display findings.
pub fn evaluate_pvc_findings(pvcs: &[KubePVC]) -> Vec<DisplayFinding> {
  let checks = all_pvc_checks();

  pvcs
    .iter()
    .flat_map(|pvc| {
      checks
        .iter()
        .filter_map(move |check| check(pvc).map(|f| f.into_display_finding()))
    })
    .collect()
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
