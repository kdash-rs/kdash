//! ReplicaSet-specific troubleshooting checks.
//!
//! This module inspects cached ReplicaSet state and produces [`DisplayFinding`]s for
//! RSs that are in an unhealthy or noteworthy phase.
//!
//! References:
//! - <https://kubernetes.io/docs/reference/generated/kubernetes-api/v1.35/#replicaset-v1-apps>

use crate::app::{models::KubeResource, replicasets::KubeReplicaSet};

use super::{DisplayFinding, Finding, IntoDisplayFinding, ResourceKind};

// ---------------------------------------------------------------------------
// RsFinding — resource-specific finding data for ReplicaSets
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct RsFinding {
  pub id: String,
  pub reason: String,
  pub namespace: String,
  pub rs_name: String,
  pub message: String,
  pub age: String,
}

// ---------------------------------------------------------------------------
// Finding<RsFinding> → DisplayFinding conversion
// ---------------------------------------------------------------------------

impl IntoDisplayFinding for Finding<RsFinding> {
  fn into_display_finding(self) -> DisplayFinding {
    let severity = self.severity_tag();
    let inner = self.into_inner();
    DisplayFinding {
      severity,
      reason: inner.reason,
      resource_kind: ResourceKind::ReplicaSet,
      namespace: Some(inner.namespace.clone()),
      resource_name: inner.rs_name.clone(),
      message: inner.message,
      age: inner.age,
      describe_kind: "replicaset".into(),
      describe_name: inner.rs_name,
      describe_namespace: Some(inner.namespace),
      k8s_obj: (),
    }
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Replica counts from `.status` (missing -> 0).
fn rs_replica_counts(rs: &KubeReplicaSet) -> (i32, i32, i32, i32) {
  let status = rs.get_k8s_obj().status.as_ref();
  let available = status
    .and_then(|s| s.available_replicas)
    .unwrap_or_default();
  let fully_labeled = status
    .and_then(|s| s.fully_labeled_replicas)
    .unwrap_or_default();
  let ready = status.and_then(|s| s.ready_replicas).unwrap_or_default();
  let replicas = status.map_or(0, |s| s.replicas);
  (available, fully_labeled, ready, replicas)
}

// ---------------------------------------------------------------------------
// Check type alias
// ---------------------------------------------------------------------------

/// Check an RS; optionally returns a finding.
pub type RsCheck = fn(&KubeReplicaSet) -> Option<Finding<RsFinding>>;

// ---------------------------------------------------------------------------
// Individual RS checks
// ---------------------------------------------------------------------------

/// Flag mismatched status replica counts.
fn check_rs_status(rs: &KubeReplicaSet) -> Option<Finding<RsFinding>> {
  let (available, fully_labeled, ready, replicas) = rs_replica_counts(rs);

  if available == fully_labeled && fully_labeled == ready && ready == replicas {
    return None;
  }

  Some(Finding::Warn(RsFinding {
    id: "rs.status.mismatch".into(),
    reason: "Replica counts differ".into(),
    namespace: rs.namespace.clone(),
    rs_name: rs.name.clone(),
    message: format!(
      "ReplicaSet status mismatch: available={}, fully_labeled={}, ready={}, replicas={}",
      available, fully_labeled, ready, replicas
    ),
    age: rs.age.clone(),
  }))
}

// ---------------------------------------------------------------------------
// Registry of all RS checks
// ---------------------------------------------------------------------------

/// Returns all registered RS checks. Add new checks here.
fn all_rs_checks() -> Vec<RsCheck> {
  vec![check_rs_status]
}

// ---------------------------------------------------------------------------
// RS evaluation entry point
// ---------------------------------------------------------------------------

/// Run RS checks and flatten findings.
pub fn evaluate_rs_findings(replica_sets: &[KubeReplicaSet]) -> Vec<DisplayFinding> {
  let checks = all_rs_checks();

  replica_sets
    .iter()
    .flat_map(|rs| {
      checks
        .iter()
        .filter_map(move |check| check(rs).map(|f| f.into_display_finding()))
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use k8s_openapi::api::apps::v1::{ReplicaSet, ReplicaSetSpec, ReplicaSetStatus};
  use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

  fn build_rs(status: Option<ReplicaSetStatus>) -> KubeReplicaSet {
    let rs = ReplicaSet {
      metadata: ObjectMeta {
        name: Some("rs-1".into()),
        namespace: Some("ns-1".into()),
        ..Default::default()
      },
      spec: Some(ReplicaSetSpec {
        replicas: Some(2),
        ..Default::default()
      }),
      status,
    };

    KubeReplicaSet::from(rs)
  }

  #[test]
  fn test_rs_replica_counts_defaults() {
    let rs = build_rs(None);
    assert_eq!(rs_replica_counts(&rs), (0, 0, 0, 0));
  }

  #[test]
  fn test_check_rs_status_no_finding_when_equal() {
    let status = ReplicaSetStatus {
      replicas: 2,
      available_replicas: Some(2),
      fully_labeled_replicas: Some(2),
      ready_replicas: Some(2),
      ..Default::default()
    };
    let rs = build_rs(Some(status));
    assert!(check_rs_status(&rs).is_none());
  }

  #[test]
  fn test_check_rs_status_finding_on_mismatch() {
    let status = ReplicaSetStatus {
      replicas: 2,
      available_replicas: Some(1),
      fully_labeled_replicas: Some(2),
      ready_replicas: Some(2),
      ..Default::default()
    };
    let rs = build_rs(Some(status));
    assert!(check_rs_status(&rs).is_some());
  }
}
