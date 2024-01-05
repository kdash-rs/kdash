use async_trait::async_trait;
use k8s_openapi::{api::apps::v1::DaemonSet, chrono::Utc};
use ratatui::{
  layout::{Constraint, Rect},
  widgets::{Cell, Row},
  Frame,
};

use super::{
  models::{AppResource, KubeResource},
  utils, ActiveBlock, App,
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
pub struct KubeDaemonSet {
  pub name: String,
  pub namespace: String,
  pub desired: i32,
  pub current: i32,
  pub ready: i32,
  pub up_to_date: i32,
  pub available: i32,
  pub age: String,
  k8s_obj: DaemonSet,
}
impl From<DaemonSet> for KubeDaemonSet {
  fn from(ds: DaemonSet) -> Self {
    let (desired, current, ready, up_to_date, available) = match ds.status.as_ref() {
      Some(s) => (
        s.desired_number_scheduled,
        s.current_number_scheduled,
        s.number_ready,
        s.updated_number_scheduled.unwrap_or_default(),
        s.number_available.unwrap_or_default(),
      ),
      _ => (0, 0, 0, 0, 0),
    };

    KubeDaemonSet {
      name: ds.metadata.name.clone().unwrap_or_default(),
      namespace: ds.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(ds.metadata.creation_timestamp.as_ref(), Utc::now()),
      desired,
      current,
      ready,
      up_to_date,
      available,
      k8s_obj: utils::sanitize_obj(ds),
    }
  }
}

impl KubeResource<DaemonSet> for KubeDaemonSet {
  fn get_name(&self) -> &String {
    &self.name
  }
  fn get_k8s_obj(&self) -> &DaemonSet {
    &self.k8s_obj
  }
}

static DAEMON_SETS_TITLE: &str = "DaemonSets";

pub struct DaemonSetResource {}

#[async_trait]
impl AppResource for DaemonSetResource {
  fn render(block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      DAEMON_SETS_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_block,
      app.data.daemon_sets
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeDaemonSet> = nw.get_namespaced_resources(DaemonSet::into).await;

    let mut app = nw.app.lock().await;
    app.data.daemon_sets.set_items(items);
  }
}

fn draw_block(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, DAEMON_SETS_TITLE, "", app.data.daemon_sets.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_AND_YAML_HINT.into(),
      resource: &mut app.data.daemon_sets,
      table_headers: vec![
        "Namespace",
        "Name",
        "Desired",
        "Current",
        "Ready",
        "Up-to-date",
        "Available",
        "Age",
      ],
      column_widths: vec![
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
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
        Cell::from(c.up_to_date.to_string()),
        Cell::from(c.available.to_string()),
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
  fn test_daemon_sets_from_api() {
    let (dss, dss_list): (Vec<KubeDaemonSet>, Vec<_>) = convert_resource_from_file("daemonsets");

    assert_eq!(dss.len(), 1);
    assert_eq!(
      dss[0],
      KubeDaemonSet {
        name: "svclb-traefik".into(),
        namespace: "kube-system".into(),
        age: utils::to_age(Some(&get_time("2021-07-05T09:36:45Z")), Utc::now()),
        k8s_obj: dss_list[0].clone(),
        desired: 1,
        current: 1,
        ready: 1,
        up_to_date: 1,
        available: 1,
      }
    );
  }
}
