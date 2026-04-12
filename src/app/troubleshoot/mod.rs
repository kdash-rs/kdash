use async_trait::async_trait;
use ratatui::{
  layout::{Constraint, Rect},
  widgets::{Cell, Row},
  Frame,
};
use strum::Display;

use super::{
  models::{AppResource, FilterableTable, Named},
  pods::KubePod,
  pvcs::KubePVC,
  replicasets::KubeReplicaSet,
  ActiveBlock, App,
};
use k8s_openapi::api::apps::v1::ReplicaSet;
use k8s_openapi::api::core::v1::{PersistentVolumeClaim, Pod};

use crate::app::key_binding::DEFAULT_KEYBINDING;
use crate::ui::utils::{
  action_hint, copy_and_escape_title_line, describe_and_yaml_hint, draw_describe_block,
  draw_route_resource_block, draw_yaml_block, filter_cursor_position, filter_status_parts,
  get_describe_active, get_resource_title, help_part, mixed_bold_line, style_caution,
  style_failure, style_primary, title_with_dual_style, ResourceTableProps,
};

// ---------------------------------------------------------------------------
// Core generic finding type
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct RawFinding {
  pub reason: String,
  pub message: String,
}

/// Severity-tagged finding; variant order defines sort priority.
#[derive(Clone, Copy, Debug, Display, Eq, Ord, PartialEq, PartialOrd)]
pub enum Severity {
  Error,
  Warn,
  Info,
}

pub trait Diagnostic {
  // The human-readable kind (e.g., "Pod", "PVC")
  fn resource_kind(&self) -> ResourceKind;

  fn name(&self) -> &str;
  fn namespace(&self) -> Option<&str>;
  fn age(&self) -> &str;
}

// A generic check type that works for any Diagnostic resource
pub type HealthCheck<T> = fn(&T) -> Option<(Severity, RawFinding)>;

// ---------------------------------------------------------------------------
// impl_diagnostic macro — shared boilerplate for Diagnostic impls
// ---------------------------------------------------------------------------

macro_rules! impl_diagnostic {
  ($ty:ty, $kind:expr) => {
    impl $crate::app::troubleshoot::Diagnostic for $ty {
      fn resource_kind(&self) -> ResourceKind {
        $kind
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
  };
}

mod pod;
mod pvc;
mod rs;

// Generic orchestrator
pub fn evaluate_resource<T: Diagnostic>(
  resources: &[T],
  checks: &[HealthCheck<T>],
) -> Vec<DisplayFinding> {
  resources
    .iter()
    .flat_map(|res| {
      checks.iter().filter_map(|check| {
        check(res).map(|(severity, raw)| DisplayFinding {
          severity,
          reason: raw.reason,
          resource_kind: res.resource_kind(),
          namespace: res.namespace().map(str::to_string),
          resource_name: res.name().to_string(),
          message: raw.message,
          age: res.age().to_string(),
        })
      })
    })
    .collect()
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

impl ResourceKind {
  pub fn describe_kind(&self) -> &'static str {
    match self {
      ResourceKind::Pod => "pod",
      ResourceKind::Pvc => "persistentvolumeclaim",
      ResourceKind::ReplicaSet => "replicaset",
    }
  }
}

// ---------------------------------------------------------------------------
// DisplayFinding — the concrete, type-erased row
// ---------------------------------------------------------------------------

/// Flattened UI row for a finding.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayFinding {
  pub severity: Severity,
  pub reason: String,
  pub resource_kind: ResourceKind,
  pub namespace: Option<String>,
  pub resource_name: String,
  pub message: String,
  pub age: String,
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
      self.resource_kind.describe_kind(),
      self.resource_name.as_str(),
      self.namespace.as_deref(),
    )
  }
}

impl Named for DisplayFinding {
  fn get_name(&self) -> &String {
    &self.resource_name
  }
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

  // Each of these calls uses the generic engine and the HealthCheck<T> type
  findings.extend(evaluate_resource(pods, &pod::all_pod_checks()));
  findings.extend(evaluate_resource(pvcs, &pvc::all_pvc_checks()));
  findings.extend(evaluate_resource(replica_sets, &rs::all_rs_checks()));

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
  let title = format!(
    " Troubleshoot (ns: {}) [{}] ",
    app
      .data
      .selected
      .ns
      .as_ref()
      .unwrap_or(&String::from("all")),
    app.data.troubleshoot_findings.count_label(),
  );
  let title_width = title.chars().count();
  let findings = &mut app.data.troubleshoot_findings;
  let filter = findings.filter.clone();
  let filter_active = findings.filter_active;

  let mut inline_help = vec![];
  inline_help.extend(filter_status_parts(&filter, filter_active));
  if !filter_active {
    inline_help.extend([
      help_part(format!(
        " | {} | ",
        action_hint("resource", DEFAULT_KEYBINDING.submit.key)
      )),
      help_part(describe_and_yaml_hint()),
    ]);
  }

  draw_route_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: mixed_bold_line(inline_help, app.light_theme),
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
        Severity::Error => style_failure(light_theme),
        Severity::Warn => style_caution(light_theme),
        Severity::Info => style_primary(light_theme),
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
  );

  if filter_active {
    f.set_cursor_position(filter_cursor_position(area, title_width, &filter));
  }
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
