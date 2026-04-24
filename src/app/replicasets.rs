use async_trait::async_trait;
use chrono::Utc;
use k8s_openapi::api::apps::v1::ReplicaSet;
use ratatui::{
  layout::Rect,
  widgets::{Cell, Row},
  Frame,
};

use super::{
  models::{self, AppResource, KubeResource, Named},
  utils::{self},
  ActiveBlock, App,
};
use crate::{
  app::key_binding::DEFAULT_KEYBINDING,
  draw_resource_tab,
  network::Network,
  ui::utils::{
    action_hint, describe_yaml_and_logs_hint, draw_describe_block, draw_resource_block,
    draw_yaml_block, get_describe_active, get_resource_title, help_bold_line, responsive_columns,
    style_primary, title_with_dual_style, ColumnDef, ResourceTableProps, ViewTier,
  },
};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeReplicaSet {
  pub name: String,
  pub namespace: String,
  pub desired: i32,
  pub current: i32,
  pub ready: i32,
  pub age: String,
  k8s_obj: ReplicaSet,
}

impl From<ReplicaSet> for KubeReplicaSet {
  fn from(rps: ReplicaSet) -> Self {
    let (current, ready) = match rps.status.as_ref() {
      Some(s) => (s.replicas, s.ready_replicas.unwrap_or_default()),
      _ => (0, 0),
    };

    KubeReplicaSet {
      name: rps.metadata.name.clone().unwrap_or_default(),
      namespace: rps.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(rps.metadata.creation_timestamp.as_ref(), Utc::now()),
      desired: rps
        .spec
        .as_ref()
        .map_or(0, |s| s.replicas.unwrap_or_default()),
      current,
      ready,
      k8s_obj: utils::sanitize_obj(rps),
    }
  }
}

impl Named for KubeReplicaSet {
  fn get_name(&self) -> &String {
    &self.name
  }
}

impl KubeResource<ReplicaSet> for KubeReplicaSet {
  fn get_k8s_obj(&self) -> &ReplicaSet {
    &self.k8s_obj
  }
}

impl models::HasPodSelector for KubeReplicaSet {
  fn pod_label_selector(&self) -> Option<String> {
    self
      .k8s_obj
      .spec
      .as_ref()
      .and_then(|s| s.selector.match_labels.as_ref())
      .filter(|labels| !labels.is_empty())
      .map(models::labels_to_selector)
  }
}

static REPLICA_SETS_TITLE: &str = "ReplicaSets";

pub struct ReplicaSetResource {}

#[async_trait]
impl AppResource for ReplicaSetResource {
  fn render(block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      REPLICA_SETS_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_block,
      app.data.replica_sets
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeReplicaSet> = nw.get_namespaced_resources(ReplicaSet::into).await;

    let mut app = nw.app.lock().await;
    app.data.replica_sets.set_items(items);
  }
}

const RS_COLUMNS: [ColumnDef; 6] = [
  ColumnDef::all("Namespace", 25, 25, 25),
  ColumnDef::all("Name", 35, 35, 35),
  ColumnDef::all("Desired", 10, 10, 10),
  ColumnDef::all("Current", 10, 10, 10),
  ColumnDef::all("Ready", 10, 10, 10),
  ColumnDef::all("Age", 10, 10, 10),
];

fn draw_block(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let is_loading = app.is_loading();
  let title = get_resource_title(
    app,
    REPLICA_SETS_TITLE,
    "",
    app.data.replica_sets.items.len(),
  );

  let (headers, widths) = responsive_columns(&RS_COLUMNS, ViewTier::Compact);

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: help_bold_line(
        format!(
          "{} | {}",
          action_hint("pods", DEFAULT_KEYBINDING.submit.key),
          describe_yaml_and_logs_hint()
        ),
        app.light_theme,
      ),
      resource: &mut app.data.replica_sets,
      table_headers: headers,
      column_widths: widths,
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.desired.to_string()),
        Cell::from(c.current.to_string()),
        Cell::from(c.ready.to_string()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary(app.light_theme))
    },
    app.light_theme,
    is_loading,
  );
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_utils::*;

  #[test]
  fn test_replica_sets_from_api() {
    let (rpls, rpls_list): (Vec<KubeReplicaSet>, Vec<_>) =
      convert_resource_from_file("replicasets");

    assert_eq!(rpls.len(), 4);
    assert_eq!(
      rpls[0],
      KubeReplicaSet {
        name: "metrics-server-86cbb8457f".into(),
        namespace: "kube-system".into(),
        age: utils::to_age(Some(&get_time("2021-05-10T21:48:19Z")), Utc::now()),
        k8s_obj: rpls_list[0].clone(),
        desired: 1,
        current: 1,
        ready: 1,
      }
    );
  }
}
