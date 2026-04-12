use async_trait::async_trait;
use ratatui::layout::Rect;
use ratatui::Frame;

use super::{
  models::AppResource, pods::KubePod, pvcs::KubePVC, replicasets::KubeReplicaSet, ActiveBlock, App,
};
use k8s_openapi::api::apps::v1::ReplicaSet;
use k8s_openapi::api::core::v1::{PersistentVolumeClaim, Pod};

use crate::ui::utils::{
  copy_and_escape_title_line, draw_describe_block, draw_yaml_block, get_describe_active,
  get_resource_title, title_with_dual_style,
};

mod engine;
mod render;
mod types;

pub use engine::evaluate_resource;
pub use render::render_troubleshoot;
pub use types::{Diagnostic, DisplayFinding, HealthCheck, RawFinding, ResourceKind, Severity};

// ---------------------------------------------------------------------------
// impl_diagnostic macro — shared boilerplate for Diagnostic impls
// ---------------------------------------------------------------------------

/// Implements [`Diagnostic`] for a resource type.
///
/// The type **must** implement [`Named`](crate::app::models::Named) (used for
/// `name()`) and have public fields `namespace: String` and `age: String`.
macro_rules! impl_diagnostic {
  ($ty:ty, $kind:expr) => {
    impl $crate::app::troubleshoot::Diagnostic for $ty {
      fn resource_kind(&self) -> $crate::app::troubleshoot::ResourceKind {
        $kind
      }
      fn name(&self) -> &str {
        $crate::app::models::Named::get_name(self)
      }
      fn namespace(&self) -> Option<&str> {
        Some(&self.namespace)
      }
      fn age(&self) -> &str {
        &self.age
      }
    }
  };
}

mod pod;
mod pvc;
mod rs;

// ---------------------------------------------------------------------------
// Evaluation orchestrator
// ---------------------------------------------------------------------------

pub fn evaluate_findings(
  pods: &[KubePod],
  pvcs: &[KubePVC],
  replica_sets: &[KubeReplicaSet],
) -> Vec<DisplayFinding> {
  let mut findings: Vec<DisplayFinding> = [
    evaluate_resource(pods, pod::all_pod_checks()),
    evaluate_resource(pvcs, pvc::all_pvc_checks()),
    evaluate_resource(replica_sets, rs::all_rs_checks()),
  ]
  .into_iter()
  .flatten()
  .collect();

  // Future: add node/deployment checks.

  findings.sort_unstable_by(|a, b| {
    a.severity
      .cmp(&b.severity)
      .then_with(|| a.resource_name.cmp(&b.resource_name))
  });

  findings
}

// ---------------------------------------------------------------------------
// AppResource impl
// ---------------------------------------------------------------------------

pub struct TroubleshootResource;

#[async_trait]
impl AppResource for TroubleshootResource {
  fn render(block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect) {
    match block {
      ActiveBlock::Containers => super::pods::draw_containers_block(f, app, area),
      ActiveBlock::Logs => super::pods::draw_logs_block(f, app, area),
      ActiveBlock::Describe => draw_describe_block(
        f,
        app,
        area,
        title_with_dual_style(
          get_resource_title(
            app,
            "Troubleshoot",
            get_describe_active(block),
            app.data.troubleshoot_findings.items.len(),
          ),
          copy_and_escape_title_line("Troubleshoot", app.light_theme),
          app.light_theme,
        ),
      ),
      ActiveBlock::Yaml => draw_yaml_block(
        f,
        app,
        area,
        title_with_dual_style(
          get_resource_title(
            app,
            "Troubleshoot",
            get_describe_active(block),
            app.data.troubleshoot_findings.items.len(),
          ),
          copy_and_escape_title_line("Troubleshoot", app.light_theme),
          app.light_theme,
        ),
      ),
      _ => render_troubleshoot(f, app, area),
    }
  }

  async fn get_resource(network: &crate::network::Network<'_>) {
    let (pods, pvcs, replica_sets) = tokio::join!(
      network.get_namespaced_resources::<Pod, KubePod, _>(KubePod::from),
      network.get_namespaced_resources::<PersistentVolumeClaim, KubePVC, _>(KubePVC::from),
      network.get_namespaced_resources::<ReplicaSet, KubeReplicaSet, _>(KubeReplicaSet::from),
    );

    let findings = evaluate_findings(&pods, &pvcs, &replica_sets);

    let mut app = network.app.lock().await;
    app.data.pods.set_items(pods);
    app.data.persistent_volume_claims.set_items(pvcs);
    app.data.replica_sets.set_items(replica_sets);
    app.data.troubleshoot_findings.set_items(findings);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use k8s_openapi::api::apps::v1::{ReplicaSet, ReplicaSetSpec, ReplicaSetStatus};
  use k8s_openapi::api::core::v1::{
    PersistentVolumeClaim, PersistentVolumeClaimStatus, Pod, PodStatus,
  };
  use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

  use crate::app::{
    models::StatefulTable, pods::KubePod, pvcs::KubePVC, replicasets::KubeReplicaSet, Data,
  };

  fn build_pod_with_phase(name: &str, phase: &str) -> KubePod {
    let pod = Pod {
      metadata: ObjectMeta {
        name: Some(name.into()),
        namespace: Some("ns-1".into()),
        ..Default::default()
      },
      status: Some(PodStatus {
        phase: Some(phase.into()),
        ..Default::default()
      }),
      ..Default::default()
    };

    KubePod::from(pod)
  }

  fn build_pvc_with_phase(name: &str, phase: &str) -> KubePVC {
    let pvc = PersistentVolumeClaim {
      metadata: ObjectMeta {
        name: Some(name.into()),
        namespace: Some("ns-1".into()),
        ..Default::default()
      },
      status: Some(PersistentVolumeClaimStatus {
        phase: Some(phase.into()),
        ..Default::default()
      }),
      ..Default::default()
    };

    KubePVC::from(pvc)
  }

  fn build_rs_with_status(
    name: &str,
    replicas: i32,
    available_replicas: i32,
    fully_labeled_replicas: i32,
    ready_replicas: i32,
  ) -> KubeReplicaSet {
    let status = ReplicaSetStatus {
      replicas,
      available_replicas: Some(available_replicas),
      fully_labeled_replicas: Some(fully_labeled_replicas),
      ready_replicas: Some(ready_replicas),
      ..Default::default()
    };
    let rs = ReplicaSet {
      metadata: ObjectMeta {
        name: Some(name.into()),
        namespace: Some("ns-1".into()),
        ..Default::default()
      },
      spec: Some(ReplicaSetSpec {
        replicas: Some(replicas),
        ..Default::default()
      }),
      status: Some(status),
    };

    KubeReplicaSet::from(rs)
  }

  fn build_app_with_resources(pod: KubePod, pvc: KubePVC, rs: KubeReplicaSet) -> App {
    App {
      data: Data {
        pods: StatefulTable::with_items(vec![pod]),
        persistent_volume_claims: StatefulTable::with_items(vec![pvc]),
        replica_sets: StatefulTable::with_items(vec![rs]),
        ..Data::default()
      },
      ..App::default()
    }
  }

  #[test]
  fn test_severity_ordering() {
    assert!(Severity::Error < Severity::Warn);
    assert!(Severity::Warn < Severity::Info);
    assert!(Severity::Error < Severity::Info);
  }

  #[test]
  fn test_evaluate_findings_sorting() {
    let pod = build_pod_with_phase("z-pod", "Failed");
    let pvc = build_pvc_with_phase("b-pvc", "Pending");
    let rs = build_rs_with_status("a-rs", 2, 1, 2, 2);

    let app = build_app_with_resources(pod, pvc, rs);

    let findings = evaluate_findings(
      &app.data.pods.items,
      &app.data.persistent_volume_claims.items,
      &app.data.replica_sets.items,
    );

    // Order: severity (Error->Warn->Info), then name.
    assert_eq!(findings.len(), 3);
    assert_eq!(findings[0].severity, Severity::Error);
    assert_eq!(findings[0].resource_name, "z-pod");
    assert_eq!(findings[1].severity, Severity::Warn);
    assert_eq!(findings[1].resource_name, "a-rs");
    assert_eq!(findings[2].severity, Severity::Warn);
    assert_eq!(findings[2].resource_name, "b-pvc");
  }

  #[test]
  fn test_display_finding_resource_ref_includes_namespace_when_present() {
    let finding = DisplayFinding {
      severity: Severity::Warn,
      reason: "Pending".into(),
      resource_kind: ResourceKind::Pod,
      namespace: Some("ns-1".into()),
      resource_name: "pod-a".into(),
      message: "pod is pending".into(),
      age: "5m".into(),
    };

    assert_eq!(finding.resource_ref(), "ns-1/pod-a");
    assert_eq!(finding.describe_target(), ("pod", "pod-a", Some("ns-1")));
  }

  #[test]
  fn test_display_finding_resource_ref_omits_empty_namespace() {
    let finding = DisplayFinding {
      severity: Severity::Info,
      reason: "Info".into(),
      resource_kind: ResourceKind::ReplicaSet,
      namespace: None,
      resource_name: "rs-a".into(),
      message: "all good".into(),
      age: "1m".into(),
    };

    assert_eq!(finding.resource_ref(), "rs-a");
    assert_eq!(finding.describe_target(), ("replicaset", "rs-a", None));
  }
}
