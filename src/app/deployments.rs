use async_trait::async_trait;
use chrono::Utc;
use k8s_openapi::api::apps::v1::Deployment;
use ratatui::{
  layout::{Constraint, Rect},
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
    draw_yaml_block, get_describe_active, get_resource_title, help_bold_line, style_primary,
    title_with_dual_style, ResourceTableProps,
  },
};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeDeployment {
  pub name: String,
  pub namespace: String,
  pub ready: String,
  pub updated: i32,
  pub available: i32,
  pub age: String,
  k8s_obj: Deployment,
}

impl From<Deployment> for KubeDeployment {
  fn from(deployment: Deployment) -> Self {
    let (ready, available, updated) = match &deployment.status {
      Some(s) => (
        format!(
          "{}/{}",
          s.available_replicas.unwrap_or_default(),
          s.replicas.unwrap_or_default()
        ),
        s.available_replicas.unwrap_or_default(),
        s.updated_replicas.unwrap_or_default(),
      ),
      _ => ("".into(), 0, 0),
    };

    Self {
      name: deployment.metadata.name.clone().unwrap_or_default(),
      namespace: deployment.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(deployment.metadata.creation_timestamp.as_ref(), Utc::now()),
      available,
      updated,
      ready,
      k8s_obj: utils::sanitize_obj(deployment),
    }
  }
}

impl Named for KubeDeployment {
  fn get_name(&self) -> &String {
    &self.name
  }
}

impl KubeResource<Deployment> for KubeDeployment {
  fn get_k8s_obj(&self) -> &Deployment {
    &self.k8s_obj
  }
}

impl models::HasPodSelector for KubeDeployment {
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

static DEPLOYMENTS_TITLE: &str = "Deployments";

pub struct DeploymentResource {}

#[async_trait]
impl AppResource for DeploymentResource {
  fn render(block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      DEPLOYMENTS_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_block,
      app.data.deployments
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeDeployment> = nw.get_namespaced_resources(Deployment::into).await;

    let mut app = nw.app.lock().await;
    app.data.deployments.set_items(items);
  }
}

fn draw_block(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let is_loading = app.is_loading();
  let title = get_resource_title(app, DEPLOYMENTS_TITLE, "", app.data.deployments.items.len());

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
      resource: &mut app.data.deployments,
      table_headers: vec![
        "Namespace",
        "Name",
        "Ready",
        "Up-to-date",
        "Available",
        "Age",
      ],
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
        Cell::from(c.ready.to_owned()),
        Cell::from(c.updated.to_string()),
        Cell::from(c.available.to_string()),
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
  fn test_deployments_from_api() {
    let (deployments, deployment_list): (Vec<KubeDeployment>, Vec<_>) =
      convert_resource_from_file("deployments");

    assert_eq!(deployments.len(), 4);
    assert_eq!(
      deployments[0],
      KubeDeployment {
        name: "metrics-server".into(),
        namespace: "kube-system".into(),
        age: utils::to_age(Some(&get_time("2021-05-10T21:48:06Z")), Utc::now()),
        k8s_obj: deployment_list[0].clone(),
        available: 1,
        updated: 1,
        ready: "1/1".into(),
      }
    );
  }

  #[test]
  fn test_deployment_pod_label_selector() {
    let (deployments, _): (Vec<KubeDeployment>, Vec<_>) = convert_resource_from_file("deployments");

    // metrics-server has matchLabels: {k8s-app: metrics-server}
    let selector = deployments[0].pod_label_selector();
    assert!(selector.is_some());
    assert_eq!(selector.unwrap(), "k8s-app=metrics-server");
  }
}
