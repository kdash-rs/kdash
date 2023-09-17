use anyhow::anyhow;
use async_trait::async_trait;
use k8s_openapi::{
  api::core::v1::{Node, Pod},
  chrono::Utc,
};
use kube::{
  api::{ListParams, ObjectList},
  core::ListMeta,
  Api,
};
use tokio::sync::MutexGuard;
use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  widgets::{Cell, Row},
  Frame,
};

use super::{
  metrics::{self, KubeNodeMetrics},
  models::{AppResource, KubeResource},
  utils::{self, UNKNOWN},
  ActiveBlock, App,
};
use crate::{
  network::Network,
  ui::utils::{
    draw_describe_block, draw_resource_block, get_cluster_wide_resource_title, get_describe_active,
    style_failure, style_primary, title_with_dual_style, ResourceTableProps, COPY_HINT,
    DESCRIBE_AND_YAML_HINT,
  },
};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeNode {
  pub name: String,
  pub status: String,
  pub role: String,
  pub version: String,
  pub pods: i32,
  pub cpu: String,
  pub mem: String,
  pub cpu_a: String,
  pub mem_a: String,
  pub cpu_percent: String,
  pub mem_percent: String,
  pub age: String,
  k8s_obj: Node,
}

static NODE_LABEL_PREFIX: &str = "node-role.kubernetes.io/";
static NODE_LABEL_ROLE: &str = "kubernetes.io/role";
static NONE_ROLE: &str = "<none>";

impl KubeNode {
  pub fn from_api_with_pods(
    node: &Node,
    pods_list: &ObjectList<Pod>,
    app: &mut MutexGuard<'_, App>,
  ) -> Self {
    let node_name = node.metadata.name.clone().unwrap_or_default();
    let unschedulable = &node
      .spec
      .as_ref()
      .map_or(false, |s| s.unschedulable.unwrap_or(false));

    let (status, version, cpu_a, mem_a) = match &node.status {
      Some(node_status) => {
        let status = if *unschedulable {
          Some("Unschedulable".into())
        } else {
          match &node_status.conditions {
            Some(conds) => match conds
              .iter()
              .find(|c| c.type_ == "Ready" && c.status == "True")
            {
              Some(cond) => Some(cond.type_.clone()),
              _ => Some("Not Ready".into()),
            },
            _ => None,
          }
        };
        let version = node_status
          .node_info
          .as_ref()
          .map(|i| i.kubelet_version.clone());

        let (cpu, mem) = node_status.allocatable.as_ref().map_or((None, None), |a| {
          (
            a.get("cpu").map(|q| q.0.clone()),
            a.get("memory").map(|q| q.0.clone()),
          )
        });

        (status, version, cpu, mem)
      }
      None => (None, None, None, None),
    };

    let pod_count = pods_list.iter().fold(0, |acc, pod| {
      let p_node_name = pod.spec.as_ref().and_then(|spec| spec.node_name.clone());
      p_node_name.map_or(acc, |v| if v == node_name { acc + 1 } else { acc })
    });

    let role = match &node.metadata.labels {
      Some(labels) => labels
        .iter()
        .filter_map(|(k, v)| {
          return if k.starts_with(NODE_LABEL_PREFIX) {
            Some(k.trim_start_matches(NODE_LABEL_PREFIX))
          } else if k == NODE_LABEL_ROLE && !v.is_empty() {
            Some(v)
          } else {
            None
          };
        })
        .collect::<Vec<_>>()
        .join(","),
      None => NONE_ROLE.into(),
    };
    let (cpu, cpu_percent, mem, mem_percent) = match app
      .data
      .node_metrics
      .iter_mut()
      .find(|nm| nm.name == node_name)
    {
      Some(nm) => {
        let cpu_percent = utils::to_cpu_percent(
          nm.cpu.clone(),
          utils::cpu_to_milli(cpu_a.clone().unwrap_or_default()),
        );
        nm.cpu_percent = cpu_percent;
        let mem_percent = utils::to_mem_percent(
          nm.mem.clone(),
          utils::mem_to_mi(mem_a.clone().unwrap_or_default()),
        );
        nm.mem_percent = mem_percent;
        (
          nm.cpu.clone(),
          cpu_percent.to_string(),
          nm.mem.clone(),
          mem_percent.to_string(),
        )
      }
      None => (
        String::from("0m"),
        String::from("0"),
        String::from("0Mi"),
        String::from("0"),
      ),
    };

    KubeNode {
      name: node_name,
      status: status.unwrap_or_else(|| UNKNOWN.into()),
      role: if role.is_empty() {
        NONE_ROLE.into()
      } else {
        role
      },
      version: version.unwrap_or_default(),
      pods: pod_count,
      age: utils::to_age(node.metadata.creation_timestamp.as_ref(), Utc::now()),
      cpu,
      mem,
      cpu_a: utils::cpu_to_milli(cpu_a.unwrap_or_default()),
      mem_a: utils::mem_to_mi(mem_a.unwrap_or_default()),
      cpu_percent,
      mem_percent,
      k8s_obj: utils::sanitize_obj(node.to_owned()),
    }
  }
}

impl KubeResource<Node> for KubeNode {
  fn get_name(&self) -> &String {
    &self.name
  }
  fn get_k8s_obj(&self) -> &Node {
    &self.k8s_obj
  }
}

static NODES_TITLE: &str = "Nodes";

pub struct NodeResource {}

#[async_trait]
impl AppResource for NodeResource {
  fn render<B: Backend>(block: ActiveBlock, f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
    match block {
      ActiveBlock::Describe | ActiveBlock::Yaml => draw_describe_block(
        f,
        app,
        area,
        title_with_dual_style(
          get_cluster_wide_resource_title(
            NODES_TITLE,
            app.data.nodes.items.len(),
            get_describe_active(block),
          ),
          format!("{} | {} <esc> ", COPY_HINT, NODES_TITLE),
          app.light_theme,
        ),
      ),
      ActiveBlock::Namespaces => Self::render(app.get_prev_route().active_block, f, app, area),
      _ => draw_block(f, app, area),
    };
  }

  async fn get_resource(nw: &Network<'_>) {
    let lp = ListParams::default();
    let api_pods: Api<Pod> = Api::all(nw.client.clone());
    let api_nodes: Api<Node> = Api::all(nw.client.clone());

    match api_nodes.list(&lp).await {
      Ok(node_list) => {
        get_node_metrics(nw).await;

        let pods_list = match api_pods.list(&lp).await {
          Ok(list) => list,
          Err(_) => ObjectList {
            metadata: ListMeta::default(),
            items: vec![],
          },
        };

        let mut app = nw.app.lock().await;

        let items = node_list
          .iter()
          .map(|node| KubeNode::from_api_with_pods(node, &pods_list, &mut app))
          .collect::<Vec<_>>();

        app.data.nodes.set_items(items);
      }
      Err(e) => {
        nw.handle_error(anyhow!("Failed to get nodes. {:?}", e))
          .await;
      }
    }
  }
}

async fn get_node_metrics(nw: &Network<'_>) {
  let api_node_metrics: Api<metrics::NodeMetrics> = Api::all(nw.client.clone());

  match api_node_metrics.list(&ListParams::default()).await {
    Ok(node_metrics) => {
      let mut app = nw.app.lock().await;

      let items = node_metrics
        .iter()
        .map(|metric| KubeNodeMetrics::from_api(metric, &app))
        .collect();

      app.data.node_metrics = items;
    }
    Err(_) => {
      let mut app = nw.app.lock().await;
      app.data.node_metrics = vec![];
      // lets not show error as it will always be showing up and be annoying
      // TODO may be show once and then disable polling
    }
  };
}

fn draw_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let title = get_cluster_wide_resource_title(NODES_TITLE, app.data.nodes.items.len(), "");

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_AND_YAML_HINT.into(),
      resource: &mut app.data.nodes,
      table_headers: vec![
        "Name", "Status", "Roles", "Version", "Pods", "CPU", "Mem", "CPU %", "Mem %", "CPU/A",
        "Mem/A", "Age",
      ],
      column_widths: vec![
        Constraint::Percentage(25),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(5),
        Constraint::Percentage(5),
        Constraint::Percentage(5),
        Constraint::Percentage(5),
        Constraint::Percentage(5),
        Constraint::Percentage(5),
        Constraint::Percentage(5),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      let style = if c.status != "Ready" {
        style_failure(app.light_theme)
      } else {
        style_primary(app.light_theme)
      };
      Row::new(vec![
        Cell::from(c.name.to_owned()),
        Cell::from(c.status.to_owned()),
        Cell::from(c.role.to_owned()),
        Cell::from(c.version.to_owned()),
        Cell::from(c.pods.to_string()),
        Cell::from(c.cpu.to_owned()),
        Cell::from(c.mem.to_owned()),
        Cell::from(c.cpu_percent.to_owned()),
        Cell::from(c.mem_percent.to_owned()),
        Cell::from(c.cpu_a.to_owned()),
        Cell::from(c.mem_a.to_owned()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style)
    },
    app.light_theme,
    app.is_loading,
    app.data.selected.filter.to_owned(),
  );
}

#[cfg(test)]
mod tests {
  use tokio::sync::Mutex;

  use super::*;
  use crate::app::{metrics::KubeNodeMetrics, test_utils::*};

  #[tokio::test]
  async fn test_nodes_from_api() {
    let nodes = load_resource_from_file("nodes");
    let node_list = nodes.items.clone();
    let pods_list = load_resource_from_file("pods");
    let mut app = App::default();
    app.data.node_metrics = vec![KubeNodeMetrics {
      name: "gke-hello-hipster-default-pool-9e6f6ffb-q16l".into(),
      cpu: "1414m".into(),
      cpu_percent: 0f64,
      mem: "590Mi".into(),
      mem_percent: 0f64,
    }];
    let app = Mutex::new(app);
    let mut app = app.lock().await;

    let nodes = nodes
      .iter()
      .map(|it| KubeNode::from_api_with_pods(it, &pods_list, &mut app))
      .collect::<Vec<_>>();

    assert_eq!(nodes.len(), 1);
    assert_eq!(
      nodes[0],
      KubeNode {
        name: "gke-hello-hipster-default-pool-9e6f6ffb-q16l".into(),
        age: utils::to_age(Some(&get_time("2021-05-10T21:48:07Z")), Utc::now()),
        k8s_obj: node_list[0].clone(),
        status: "Ready".into(),
        role: "control-plane,master".into(),
        version: "v1.20.6+k3s1".into(),
        pods: 5,
        cpu: "1414m".into(),
        mem: "590Mi".into(),
        cpu_a: "8000m".into(),
        mem_a: "31967Mi".into(),
        cpu_percent: "17".into(),
        mem_percent: "1".into(),
      }
    );
  }
}
