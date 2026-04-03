use async_trait::async_trait;
use ratatui::{
  layout::{Constraint, Rect},
  widgets::{Cell, Row},
  Frame,
};
use strum::Display;

use super::{
  models::{AppResource, KubeResource},
  pods::KubePod,
  pvcs::KubePVC,
  replicasets::KubeReplicaSet,
  ActiveBlock, App,
};
use k8s_openapi::api::apps::v1::ReplicaSet;
use k8s_openapi::api::core::v1::{PersistentVolumeClaim, Pod};

mod pod;
mod pvc;
mod rs;

use crate::ui::utils::{
  draw_describe_block, draw_resource_block, draw_yaml_block, get_describe_active,
  get_resource_title, style_failure, style_primary, style_warning, title_with_dual_style,
  ResourceTableProps, COPY_HINT, DESCRIBE_AND_YAML_HINT,
};

// ---------------------------------------------------------------------------
// Core generic finding type
// ---------------------------------------------------------------------------

/// Severity-tagged finding; variant order defines sort priority.
#[derive(Clone, Debug, Display, Eq, Ord, PartialEq, PartialOrd)]
pub enum Finding<R> {
  Error(R),
  Warn(R),
  Info(R),
}

impl<R> Finding<R> {
  /// Severity-only copy for type-erased storage.
  pub fn severity_tag(&self) -> Finding<()> {
    match self {
      Finding::Error(_) => Finding::Error(()),
      Finding::Warn(_) => Finding::Warn(()),
      Finding::Info(_) => Finding::Info(()),
    }
  }

  /// Return inner payload.
  pub fn into_inner(self) -> R {
    match self {
      Finding::Info(r) | Finding::Warn(r) | Finding::Error(r) => r,
    }
  }
}

// ---------------------------------------------------------------------------
// Display enums shared across resource-specific findings
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Display, Eq, PartialEq)]
pub enum ResourceKind {
  Pod,
  #[strum(serialize = "PVC")]
  Pvc,
  ReplicaSet,
}

// ---------------------------------------------------------------------------
// DisplayFinding — the concrete, type-erased row
// ---------------------------------------------------------------------------

/// Flattened UI row for a finding.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayFinding {
  pub severity: Finding<()>,
  pub reason: String,
  pub resource_kind: ResourceKind,
  pub namespace: Option<String>,
  pub resource_name: String,
  pub message: String,
  pub age: String,
  pub describe_kind: String,
  pub describe_name: String,
  pub describe_namespace: Option<String>,
  // Unit k8s_obj kept for KubeResource trait compatibility
  pub(crate) k8s_obj: (),
}

impl DisplayFinding {
  pub fn resource_ref(&self) -> String {
    match &self.namespace {
      Some(ns) if !ns.is_empty() => format!("{}/{}", ns, self.resource_name),
      _ => self.resource_name.clone(),
    }
  }

  pub fn describe_target(&self) -> (&str, &str, Option<&str>) {
    (
      self.describe_kind.as_str(),
      self.describe_name.as_str(),
      self.describe_namespace.as_deref(),
    )
  }
}

impl KubeResource<()> for DisplayFinding {
  fn get_name(&self) -> &String {
    &self.resource_name
  }

  fn get_k8s_obj(&self) -> &() {
    &self.k8s_obj
  }
}

// ---------------------------------------------------------------------------
// Conversion trait — resource findings → display findings
// ---------------------------------------------------------------------------

/// Convert resource findings into display rows.
pub trait IntoDisplayFinding {
  fn into_display_finding(self) -> DisplayFinding;
}

// ---------------------------------------------------------------------------
// Evaluation orchestrator
// ---------------------------------------------------------------------------

pub fn evaluate_findings(
  pods: &[KubePod],
  pvcs: &[KubePVC],
  replica_sets: &[KubeReplicaSet],
) -> Vec<DisplayFinding> {
  let mut findings: Vec<DisplayFinding> = Vec::new();

  // Collect pod findings
  findings.extend(pod::evaluate_pod_findings(pods));

  // Collect PVC findings
  findings.extend(pvc::evaluate_pvc_findings(pvcs));

  // Collect ReplicaSet findings
  findings.extend(rs::evaluate_rs_findings(replica_sets));

  // Future: add node/deployment checks.

  findings.sort_by(|a, b| {
    a.severity
      .cmp(&b.severity)
      .then_with(|| a.resource_name.cmp(&b.resource_name))
  });

  findings
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

pub fn render_troubleshoot(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let light_theme = app.light_theme;
  let is_loading = app.is_loading();
  let filter = app.data.selected.filter.to_owned();
  let title = get_resource_title(
    app,
    "Troubleshoot",
    "",
    app.data.troubleshoot_findings.items.len(),
  );
  let findings = &mut app.data.troubleshoot_findings;

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: format!("| resource <enter> | {} ", DESCRIBE_AND_YAML_HINT),
      resource: findings,
      table_headers: vec!["Severity", "Type", "Reason", "Resource", "Message", "Age"],
      column_widths: vec![
        Constraint::Percentage(7),
        Constraint::Percentage(6),
        Constraint::Percentage(13),
        Constraint::Percentage(18),
        Constraint::Percentage(44),
        Constraint::Percentage(12),
      ],
    },
    |c| {
      let style = match c.severity {
        Finding::Error(()) => style_failure(light_theme),
        Finding::Warn(()) => style_warning(light_theme),
        Finding::Info(()) => style_primary(light_theme),
      };

      Row::new(vec![
        Cell::from(c.severity.to_string()),
        Cell::from(c.resource_kind.to_string()),
        Cell::from(c.reason.clone()),
        Cell::from(c.resource_ref()),
        Cell::from(c.message.clone()),
        Cell::from(c.age.clone()),
      ])
      .style(style)
    },
    light_theme,
    is_loading,
    filter,
  );
}

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
          format!("{} | Troubleshoot <esc> ", COPY_HINT),
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
          format!("{} | Troubleshoot <esc> ", COPY_HINT),
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

  /// Verifies type-erasing the payload into `()` while preserving severity.
  #[test]
  fn test_finding_severity_tag() {
    let error = Finding::Error("x").severity_tag();
    let warn = Finding::Warn("x").severity_tag();
    let info = Finding::Info("x").severity_tag();

    assert_eq!(error, Finding::Error(()));
    assert_eq!(warn, Finding::Warn(()));
    assert_eq!(info, Finding::Info(()));
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
    assert_eq!(findings[0].severity, Finding::Error(()));
    assert_eq!(findings[0].resource_name, "z-pod");
    assert_eq!(findings[1].severity, Finding::Warn(()));
    assert_eq!(findings[1].resource_name, "a-rs");
    assert_eq!(findings[2].severity, Finding::Warn(()));
    assert_eq!(findings[2].resource_name, "b-pvc");
  }
}
