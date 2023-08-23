use anyhow::anyhow;
use async_trait::async_trait;
use k8s_openapi::api::core::v1::{Node, Pod};
use kube::{
  api::{ListParams, ObjectMeta},
  Api,
};
use kubectl_view_allocations::{
  extract_allocatable_from_nodes, extract_allocatable_from_pods,
  extract_utilizations_from_pod_metrics, make_qualifiers, metrics::PodMetrics, Resource,
};
use kubectl_view_allocations::{metrics::Usage, qty::Qty, tree::provide_prefix};
use serde::{Deserialize, Serialize};
use tokio::sync::MutexGuard;
use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  widgets::{Cell, Row, Table},
  Frame,
};

use crate::{
  network::Network,
  ui::utils::{
    layout_block_active, loading, style_highlight, style_primary, style_success, style_warning,
    table_header_style,
  },
};

use super::{models::AppResource, utils, ActiveBlock, App};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetrics {
  metadata: kube::api::ObjectMeta,
  usage: Usage,
  timestamp: String,
  window: String,
}

// custom impl since metrics API doesn't exist on kube-rs
impl k8s_openapi::Resource for NodeMetrics {
  const GROUP: &'static str = "metrics.k8s.io";
  const KIND: &'static str = "node";
  const VERSION: &'static str = "v1beta1";
  const API_VERSION: &'static str = "metrics.k8s.io/v1beta1";
  const URL_PATH_SEGMENT: &'static str = "nodes";
  type Scope = k8s_openapi::ClusterResourceScope;
}

impl k8s_openapi::Metadata for NodeMetrics {
  type Ty = ObjectMeta;

  fn metadata(&self) -> &Self::Ty {
    &self.metadata
  }

  fn metadata_mut(&mut self) -> &mut Self::Ty {
    &mut self.metadata
  }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct KubeNodeMetrics {
  pub name: String,
  pub cpu: String,
  pub cpu_percent: f64,
  pub mem: String,
  pub mem_percent: f64,
}

impl KubeNodeMetrics {
  pub fn from_api(metric: &NodeMetrics, app: &MutexGuard<'_, App>) -> Self {
    let name = metric.metadata.name.clone().unwrap_or_default();

    let (cpu_percent, mem_percent) = match app.data.node_metrics.iter().find(|it| it.name == name) {
      Some(nm) => (nm.cpu_percent, nm.mem_percent),
      None => (0f64, 0f64),
    };

    KubeNodeMetrics {
      name,
      cpu: utils::cpu_to_milli(metric.usage.cpu.trim_matches('"').to_owned()),
      mem: utils::mem_to_mi(metric.usage.memory.trim_matches('"').to_owned()),
      cpu_percent,
      mem_percent,
    }
  }
}

pub struct UtilizationResource {}

#[async_trait]
impl AppResource for UtilizationResource {
  fn render<B: Backend>(_block: ActiveBlock, f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
    let title = format!(
      " Resource Utilization (ns: [{}], group by <g>: {:?}) ",
      app
        .data
        .selected
        .ns
        .as_ref()
        .unwrap_or(&String::from("all")),
      app.utilization_group_by
    );
    let block = layout_block_active(title.as_str(), app.light_theme);

    if !app.data.metrics.items.is_empty() {
      let data = &app.data.metrics.items;

      let prefixes = provide_prefix(data, |parent, item| parent.0.len() + 1 == item.0.len());

      // Create the table
      let mut rows: Vec<Row<'_>> = vec![];
      for ((k, oqtys), prefix) in data.iter().zip(prefixes.iter()) {
        let column0 = format!(
          "{} {}",
          prefix,
          k.last().map(|x| x.as_str()).unwrap_or("???")
        );
        if let Some(qtys) = oqtys {
          let style = if qtys.requested > qtys.limit || qtys.utilization > qtys.limit {
            style_warning(app.light_theme)
          } else if is_empty(&qtys.requested) || is_empty(&qtys.limit) {
            style_primary(app.light_theme)
          } else {
            style_success(app.light_theme)
          };

          let row = Row::new(vec![
            Cell::from(column0),
            make_table_cell(&qtys.utilization, &qtys.allocatable),
            make_table_cell(&qtys.requested, &qtys.allocatable),
            make_table_cell(&qtys.limit, &qtys.allocatable),
            make_table_cell(&qtys.allocatable, &None),
            make_table_cell(&qtys.calc_free(), &None),
          ])
          .style(style);
          rows.push(row);
        }
      }

      let table = Table::new(rows)
        .header(table_header_style(
          vec![
            "Resource",
            "Utilization",
            "Requested",
            "Limit",
            "Allocatable",
            "Free",
          ],
          app.light_theme,
        ))
        .block(block)
        .widths(&[
          Constraint::Percentage(50),
          Constraint::Percentage(10),
          Constraint::Percentage(10),
          Constraint::Percentage(10),
          Constraint::Percentage(10),
          Constraint::Percentage(10),
        ])
        .highlight_style(style_highlight());

      f.render_stateful_widget(table, area, &mut app.data.metrics.state);
    } else {
      loading(f, block, area, app.is_loading, app.light_theme);
    }
  }

  async fn get_resource(nw: &Network<'_>) {
    let mut resources: Vec<Resource> = vec![];

    let node_api: Api<Node> = Api::all(nw.client.clone());
    match node_api.list(&ListParams::default()).await {
      Ok(node_list) => {
        if let Err(e) = extract_allocatable_from_nodes(node_list, &mut resources).await {
          nw.handle_error(anyhow!(
            "Failed to extract node allocation metrics. {:?}",
            e
          ))
          .await;
        }
      }
      Err(e) => {
        nw.handle_error(anyhow!(
          "Failed to extract node allocation metrics. {:?}",
          e
        ))
        .await
      }
    }

    let pod_api: Api<Pod> = nw.get_namespaced_api().await;
    match pod_api.list(&ListParams::default()).await {
      Ok(pod_list) => {
        if let Err(e) = extract_allocatable_from_pods(pod_list, &mut resources).await {
          nw.handle_error(anyhow!("Failed to extract pod allocation metrics. {:?}", e))
            .await;
        }
      }
      Err(e) => {
        nw.handle_error(anyhow!("Failed to extract pod allocation metrics. {:?}", e))
          .await
      }
    }

    let api_pod_metrics: Api<PodMetrics> = Api::all(nw.client.clone());

    match api_pod_metrics
    .list(&ListParams::default())
    .await
    {
      Ok(pod_metrics) => {
        if let Err(e) = extract_utilizations_from_pod_metrics(pod_metrics, &mut resources).await {
          nw.handle_error(anyhow!("Failed to extract pod utilization metrics. {:?}", e)).await;
        }
      }
      Err(_e) => nw.handle_error(anyhow!("Failed to extract pod utilization metrics. Make sure you have a metrics-server deployed on your cluster.")).await,
    };

    let mut app = nw.app.lock().await;

    let data = make_qualifiers(&resources, &app.utilization_group_by, &[]);

    app.data.metrics.set_items(data);
  }
}

fn make_table_cell<'a>(oqty: &Option<Qty>, o100: &Option<Qty>) -> Cell<'a> {
  let txt = match oqty {
    None => "__".into(),
    Some(ref qty) => match o100 {
      None => format!("{}", qty.adjust_scale()),
      Some(q100) => format!("{} ({:.0}%)", qty.adjust_scale(), qty.calc_percentage(q100)),
    },
  };
  Cell::from(txt)
}

fn is_empty(oqty: &Option<Qty>) -> bool {
  match oqty {
    Some(qty) => qty.is_zero(),
    None => true,
  }
}

#[cfg(test)]
mod tests {
  use tokio::sync::Mutex;

  use super::*;
  use crate::app::test_utils::load_resource_from_file;

  #[tokio::test]
  async fn test_kube_node_metrics_from_api() {
    let node_metrics = load_resource_from_file("node_metrics");
    assert_eq!(node_metrics.items.len(), 2);

    let mut app = App::default();
    app.data.node_metrics = vec![KubeNodeMetrics {
      name: "k3d-my-kdash-cluster-server-0".into(),
      cpu: "".into(),
      cpu_percent: 10f64,
      mem: "".into(),
      mem_percent: 20f64,
    }];
    let app = Mutex::new(app);
    let app = app.lock().await;

    let metrics = node_metrics
      .iter()
      .map(|it| KubeNodeMetrics::from_api(it, &app))
      .collect::<Vec<_>>();
    assert_eq!(metrics.len(), 2);
    assert_eq!(
      metrics[0],
      KubeNodeMetrics {
        name: "k3d-my-kdash-cluster-server-0".into(),
        cpu: "162m".into(),
        cpu_percent: 10f64,
        mem: "569Mi".into(),
        mem_percent: 20f64,
      }
    );
    assert_eq!(
      metrics[1],
      KubeNodeMetrics {
        name: "k3d-my-kdash-cluster-server-1".into(),
        cpu: "102m".into(),
        cpu_percent: 0f64,
        mem: "276Mi".into(),
        mem_percent: 0f64,
      }
    );
  }
}
