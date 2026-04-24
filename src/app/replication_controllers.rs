use std::collections::BTreeMap;

use async_trait::async_trait;
use chrono::Utc;
use k8s_openapi::api::core::v1::ReplicationController;
use ratatui::{
  layout::{Constraint, Rect},
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
    action_hint, describe_yaml_logs_and_esc_hint, draw_describe_block, draw_resource_block,
    draw_yaml_block, get_describe_active, get_resource_title, help_bold_line, style_primary,
    title_with_dual_style, ResourceTableProps,
  },
};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeReplicationController {
  pub name: String,
  pub namespace: String,
  pub desired: i32,
  pub current: i32,
  pub ready: i32,
  pub containers: String,
  pub images: String,
  pub selector: String,
  pub age: String,
  k8s_obj: ReplicationController,
}

impl From<ReplicationController> for KubeReplicationController {
  fn from(rplc: ReplicationController) -> Self {
    let (current, ready) = match rplc.status.as_ref() {
      Some(s) => (s.replicas, s.ready_replicas.unwrap_or_default()),
      _ => (0, 0),
    };

    let (desired, selector, (containers, images)) = match rplc.spec.as_ref() {
      Some(spec) => (
        spec.replicas.unwrap_or_default(),
        spec
          .selector
          .as_ref()
          .unwrap_or(&BTreeMap::new())
          .iter()
          .map(|(key, val)| format!("{}={}", key, val))
          .collect::<Vec<String>>()
          .join(","),
        match spec.template.as_ref() {
          Some(tmpl) => match tmpl.spec.as_ref() {
            Some(pspec) => (
              pspec
                .containers
                .iter()
                .map(|c| c.name.to_owned())
                .collect::<Vec<String>>()
                .join(","),
              pspec
                .containers
                .iter()
                .filter_map(|c| c.image.to_owned())
                .collect::<Vec<String>>()
                .join(","),
            ),
            None => ("".into(), "".into()),
          },
          None => ("".into(), "".into()),
        },
      ),
      None => (0, "".into(), ("".into(), "".into())),
    };

    KubeReplicationController {
      name: rplc.metadata.name.clone().unwrap_or_default(),
      namespace: rplc.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(rplc.metadata.creation_timestamp.as_ref(), Utc::now()),
      desired,
      current,
      ready,
      containers,
      images,
      selector,
      k8s_obj: utils::sanitize_obj(rplc),
    }
  }
}

impl Named for KubeReplicationController {
  fn get_name(&self) -> &String {
    &self.name
  }
}

impl KubeResource<ReplicationController> for KubeReplicationController {
  fn get_k8s_obj(&self) -> &ReplicationController {
    &self.k8s_obj
  }
}

impl models::HasPodSelector for KubeReplicationController {
  fn pod_label_selector(&self) -> Option<String> {
    self
      .k8s_obj
      .spec
      .as_ref()
      .and_then(|s| s.selector.as_ref())
      .filter(|labels| !labels.is_empty())
      .map(models::labels_to_selector)
  }
}

static RPL_CTRL_TITLE: &str = "ReplicationControllers";

pub struct ReplicationControllerResource {}

#[async_trait]
impl AppResource for ReplicationControllerResource {
  fn render(block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      RPL_CTRL_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_block,
      app.data.replication_controllers
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeReplicationController> = nw
      .get_namespaced_resources(ReplicationController::into)
      .await;

    let mut app = nw.app.lock().await;
    app.data.replication_controllers.set_items(items);
  }
}

fn draw_block(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let is_loading = app.is_loading();
  let title = get_resource_title(
    app,
    RPL_CTRL_TITLE,
    "",
    app.data.replication_controllers.items.len(),
  );

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: help_bold_line(
        format!(
          "{} | {}",
          action_hint("pods", DEFAULT_KEYBINDING.submit.key),
          describe_yaml_logs_and_esc_hint()
        ),
        app.light_theme,
      ),
      resource: &mut app.data.replication_controllers,
      table_headers: vec![
        "Namespace",
        "Name",
        "Desired",
        "Current",
        "Ready",
        "Containers",
        "Images",
        "Selector",
        "Age",
      ],
      column_widths: vec![
        Constraint::Percentage(15),
        Constraint::Percentage(15),
        Constraint::Percentage(10),
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
        Cell::from(c.containers.to_owned()),
        Cell::from(c.images.to_owned()),
        Cell::from(c.selector.to_owned()),
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
  use crate::app::models::HasPodSelector;
  use crate::app::test_utils::*;

  #[test]
  fn test_replica_sets_from_api() {
    let (rplc, rplc_list): (Vec<KubeReplicationController>, Vec<_>) =
      convert_resource_from_file("replication_controllers");

    assert_eq!(rplc.len(), 2);
    assert_eq!(
      rplc[0],
      KubeReplicationController {
        name: "nginx".into(),
        namespace: "default".into(),
        age: utils::to_age(Some(&get_time("2021-07-27T14:37:49Z")), Utc::now()),
        k8s_obj: rplc_list[0].clone(),
        desired: 3,
        current: 3,
        ready: 3,
        containers: "nginx".into(),
        images: "nginx".into(),
        selector: "app=nginx".into(),
      }
    );
    assert_eq!(
      rplc[1],
      KubeReplicationController {
        name: "nginx-new".into(),
        namespace: "default".into(),
        age: utils::to_age(Some(&get_time("2021-07-27T14:45:24Z")), Utc::now()),
        k8s_obj: rplc_list[1].clone(),
        desired: 3,
        current: 3,
        ready: 0,
        containers: "nginx,nginx2".into(),
        images: "nginx,nginx".into(),
        selector: "app=nginx".into(),
      }
    );
  }

  #[test]
  fn test_replication_controller_pod_label_selector() {
    let (rplc, _): (Vec<KubeReplicationController>, Vec<_>) =
      convert_resource_from_file("replication_controllers");

    // nginx RC has selector: {app: nginx}
    let selector = rplc[0].pod_label_selector();
    assert!(selector.is_some());
    assert_eq!(selector.unwrap(), "app=nginx");
  }
}
