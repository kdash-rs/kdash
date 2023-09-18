use async_trait::async_trait;
use k8s_openapi::{api::apps::v1::ReplicaSet, chrono::Utc};
use ratatui::{
  backend::Backend,
  layout::{Constraint, Rect},
  widgets::{Cell, Row},
  Frame,
};

use super::{
  models::{AppResource, KubeResource},
  utils::{self},
  ActiveBlock, App,
};
use crate::{
  draw_resource_tab,
  network::Network,
  ui::utils::{
    draw_describe_block, draw_resource_block, get_describe_active, get_resource_title,
    style_primary, title_with_dual_style, ResourceTableProps, COPY_HINT, DESCRIBE_AND_YAML_HINT,
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

impl KubeResource<ReplicaSet> for KubeReplicaSet {
  fn get_name(&self) -> &String {
    &self.name
  }
  fn get_k8s_obj(&self) -> &ReplicaSet {
    &self.k8s_obj
  }
}

static REPLICA_SETS_TITLE: &str = "ReplicaSets";

pub struct ReplicaSetResource {}

#[async_trait]
impl AppResource for ReplicaSetResource {
  fn render<B: Backend>(block: ActiveBlock, f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
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

fn draw_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let title = get_resource_title(
    app,
    REPLICA_SETS_TITLE,
    "",
    app.data.replica_sets.items.len(),
  );

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_AND_YAML_HINT.into(),
      resource: &mut app.data.replica_sets,
      table_headers: vec!["Namespace", "Name", "Desired", "Current", "Ready", "Age"],
      column_widths: vec![
        Constraint::Percentage(25),
        Constraint::Percentage(35),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
      ],
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
    app.is_loading,
    app.data.selected.filter.to_owned(),
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
