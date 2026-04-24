use async_trait::async_trait;
use chrono::Utc;
use k8s_openapi::api::apps::v1::DaemonSet;
use ratatui::{
  layout::Rect,
  widgets::{Cell, Row},
  Frame,
};

use super::{
  models::{self, AppResource, KubeResource, Named},
  utils, ActiveBlock, App,
};
use crate::{
  app::key_binding::DEFAULT_KEYBINDING,
  draw_resource_tab,
  network::Network,
  ui::utils::{
    action_hint, describe_yaml_and_logs_hint, draw_describe_block, draw_resource_block,
    draw_yaml_block, get_describe_active, get_resource_title, help_bold_line, responsive_columns,
    style_primary, title_with_dual_style, wide_hint, ColumnDef, ResourceTableProps, ViewTier,
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
  pub node_selector: String,
  pub containers: String,
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

    let node_selector = ds
      .spec
      .as_ref()
      .and_then(|s| s.template.spec.as_ref())
      .and_then(|ps| ps.node_selector.as_ref())
      .map_or(String::new(), |ns| {
        ns.iter()
          .map(|(k, v)| format!("{}={}", k, v))
          .collect::<Vec<_>>()
          .join(",")
      });
    let containers = ds
      .spec
      .as_ref()
      .and_then(|s| s.template.spec.as_ref())
      .map_or(String::new(), |ps| {
        ps.containers
          .iter()
          .map(|c| c.name.clone())
          .collect::<Vec<_>>()
          .join(",")
      });

    KubeDaemonSet {
      name: ds.metadata.name.clone().unwrap_or_default(),
      namespace: ds.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(ds.metadata.creation_timestamp.as_ref(), Utc::now()),
      desired,
      current,
      ready,
      up_to_date,
      available,
      node_selector,
      containers,
      k8s_obj: utils::sanitize_obj(ds),
    }
  }
}

impl Named for KubeDaemonSet {
  fn get_name(&self) -> &String {
    &self.name
  }
}

impl KubeResource<DaemonSet> for KubeDaemonSet {
  fn get_k8s_obj(&self) -> &DaemonSet {
    &self.k8s_obj
  }
}

impl models::HasPodSelector for KubeDaemonSet {
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

const DS_COLUMNS: [ColumnDef; 10] = [
  ColumnDef::all("Namespace", 20, 15, 12),
  ColumnDef::all("Name", 20, 18, 15),
  ColumnDef::all("Desired", 10, 8, 7),
  ColumnDef::all("Current", 10, 8, 7),
  ColumnDef::all("Ready", 10, 8, 7),
  ColumnDef::all("Up-to-date", 10, 8, 8),
  ColumnDef::all("Available", 10, 8, 8),
  ColumnDef::standard("Node Selector", 19, 14),
  ColumnDef::wide("Containers", 14),
  ColumnDef::all("Age", 10, 8, 8),
];

fn draw_block(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let is_loading = app.is_loading();
  let title = get_resource_title(app, DAEMON_SETS_TITLE, "", app.data.daemon_sets.items.len());

  let tier = ViewTier::from_width(area.width, app.wide_columns);
  let (headers, widths) = responsive_columns(&DS_COLUMNS, tier);

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: help_bold_line(
        format!(
          "{} | {} | {}",
          action_hint("pods", DEFAULT_KEYBINDING.submit.key),
          describe_yaml_and_logs_hint(),
          wide_hint()
        ),
        app.light_theme,
      ),
      resource: &mut app.data.daemon_sets,
      table_headers: headers,
      column_widths: widths,
    },
    |c| {
      let mut cells = vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.desired.to_string()),
        Cell::from(c.current.to_string()),
        Cell::from(c.ready.to_string()),
        Cell::from(c.up_to_date.to_string()),
        Cell::from(c.available.to_string()),
      ];
      if tier >= ViewTier::Standard {
        cells.push(Cell::from(c.node_selector.to_owned()));
      }
      if tier >= ViewTier::Wide {
        cells.push(Cell::from(c.containers.to_owned()));
      }
      cells.push(Cell::from(c.age.to_owned()));
      Row::new(cells).style(style_primary(app.light_theme))
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
        node_selector: "".into(),
        containers: "lb-port-80,lb-port-443".into(),
      }
    );
  }
}
