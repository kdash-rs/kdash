use async_trait::async_trait;
use k8s_openapi::{api::apps::v1::StatefulSet, chrono::Utc};
use ratatui::{
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
    draw_describe_block, draw_resource_block, draw_yaml_block, get_describe_active,
    get_resource_title, style_primary, title_with_dual_style, ResourceTableProps, COPY_HINT,
    DESCRIBE_AND_YAML_HINT,
  },
};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeStatefulSet {
  pub name: String,
  pub namespace: String,
  pub ready: String,
  pub service: String,
  pub age: String,
  k8s_obj: StatefulSet,
}

impl From<StatefulSet> for KubeStatefulSet {
  fn from(stfs: StatefulSet) -> Self {
    let ready = match &stfs.status {
      Some(s) => format!("{}/{}", s.ready_replicas.unwrap_or_default(), s.replicas),
      _ => "".into(),
    };

    KubeStatefulSet {
      name: stfs.metadata.name.clone().unwrap_or_default(),
      namespace: stfs.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(stfs.metadata.creation_timestamp.as_ref(), Utc::now()),
      service: stfs
        .spec
        .as_ref()
        .map_or("n/a".into(), |spec| spec.service_name.to_owned()),
      ready,
      k8s_obj: utils::sanitize_obj(stfs),
    }
  }
}

impl KubeResource<StatefulSet> for KubeStatefulSet {
  fn get_name(&self) -> &String {
    &self.name
  }
  fn get_k8s_obj(&self) -> &StatefulSet {
    &self.k8s_obj
  }
}

static STFS_TITLE: &str = "StatefulSets";

pub struct StatefulSetResource {}

#[async_trait]
impl AppResource for StatefulSetResource {
  fn render(block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      STFS_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_block,
      app.data.stateful_sets
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeStatefulSet> = nw.get_namespaced_resources(StatefulSet::into).await;

    let mut app = nw.app.lock().await;
    app.data.stateful_sets.set_items(items);
  }
}

fn draw_block(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, STFS_TITLE, "", app.data.stateful_sets.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_AND_YAML_HINT.into(),
      resource: &mut app.data.stateful_sets,
      table_headers: vec!["Namespace", "Name", "Ready", "Service", "Age"],
      column_widths: vec![
        Constraint::Percentage(25),
        Constraint::Percentage(30),
        Constraint::Percentage(10),
        Constraint::Percentage(25),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.ready.to_owned()),
        Cell::from(c.service.to_owned()),
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
  fn test_stateful_sets_from_api() {
    let (stfs, stfs_list): (Vec<KubeStatefulSet>, Vec<_>) = convert_resource_from_file("stfs");

    assert_eq!(stfs.len(), 1);
    assert_eq!(
      stfs[0],
      KubeStatefulSet {
        name: "web".into(),
        namespace: "default".into(),
        age: utils::to_age(Some(&get_time("2021-04-25T14:23:47Z")), Utc::now()),
        k8s_obj: stfs_list[0].clone(),
        service: "nginx".into(),
        ready: "2/2".into(),
      }
    );
  }
}
