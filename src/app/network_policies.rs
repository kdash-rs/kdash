use std::vec;

use async_trait::async_trait;
use k8s_openapi::{api::networking::v1::NetworkPolicy, chrono::Utc};
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
    DESCRIBE_YAML_AND_ESC_HINT,
  },
};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeNetworkPolicy {
  pub name: String,
  pub namespace: String,
  pub pod_selector: String,
  pub policy_types: String,
  pub age: String,
  k8s_obj: NetworkPolicy,
}

impl From<NetworkPolicy> for KubeNetworkPolicy {
  fn from(nw_policy: NetworkPolicy) -> Self {
    let pod_selector = match &nw_policy.spec {
      Some(s) => {
        let mut pod_selector = vec![];
        if let Some(match_labels) = &s.pod_selector.match_labels {
          for (k, v) in match_labels {
            pod_selector.push(format!("{}={}", k, v));
          }
        }
        pod_selector
      }
      _ => vec![],
    };

    Self {
      name: nw_policy.metadata.name.clone().unwrap_or_default(),
      namespace: nw_policy.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(nw_policy.metadata.creation_timestamp.as_ref(), Utc::now()),
      pod_selector: pod_selector.join(","),
      policy_types: nw_policy.spec.as_ref().map_or_else(
        || "".into(),
        |s| s.policy_types.clone().unwrap_or_default().join(","),
      ),
      k8s_obj: utils::sanitize_obj(nw_policy),
    }
  }
}

impl KubeResource<NetworkPolicy> for KubeNetworkPolicy {
  fn get_name(&self) -> &String {
    &self.name
  }
  fn get_k8s_obj(&self) -> &NetworkPolicy {
    &self.k8s_obj
  }
}

static NW_POLICY_TITLE: &str = "NetworkPolicies";

pub struct NetworkPolicyResource {}

#[async_trait]
impl AppResource for NetworkPolicyResource {
  fn render(block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      NW_POLICY_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_block,
      app.data.nw_policies
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeNetworkPolicy> = nw.get_namespaced_resources(NetworkPolicy::into).await;

    let mut app = nw.app.lock().await;
    app.data.nw_policies.set_items(items);
  }
}

fn draw_block(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, NW_POLICY_TITLE, "", app.data.nw_policies.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_YAML_AND_ESC_HINT.into(),
      resource: &mut app.data.nw_policies,
      table_headers: vec!["Namespace", "Name", "Pod Selector", "Policy Types", "Age"],
      column_widths: vec![
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(30),
        Constraint::Percentage(20),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.pod_selector.to_owned()),
        Cell::from(c.policy_types.to_owned()),
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
  fn test_nw_policys_from_api() {
    let (nw_policys, nw_policy_list): (Vec<KubeNetworkPolicy>, Vec<_>) =
      convert_resource_from_file("network_policy");

    assert_eq!(nw_policys.len(), 4);
    assert_eq!(
      nw_policys[3],
      KubeNetworkPolicy {
        name: "sample-network-policy-4".into(),
        namespace: "default".into(),
        age: utils::to_age(Some(&get_time("2023-07-04T17:04:33Z")), Utc::now()),
        k8s_obj: nw_policy_list[3].clone(),
        pod_selector: "app=webapp,app3=webapp3".into(),
        policy_types: "Egress,Ingress".into(),
      }
    );
  }
}
