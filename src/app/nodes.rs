use k8s_openapi::{
  api::core::v1::{Node, Pod},
  chrono::Utc,
};
use kube::api::ObjectList;
use tokio::sync::MutexGuard;

use super::{
  models::KubeResource,
  utils::{self, UNKNOWN},
  App,
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
    app: &mut MutexGuard<App>,
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
      k8s_obj: node.to_owned(),
    }
  }
}

impl KubeResource<Node> for KubeNode {
  fn get_k8s_obj(&self) -> &Node {
    &self.k8s_obj
  }

  fn from_api(_item: &Node) -> Self {
    unimplemented!()
  }
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
