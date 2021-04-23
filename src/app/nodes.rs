use k8s_openapi::{
  api::core::v1::{Node, Pod},
  chrono::Utc,
};
use kube::{
  api::{DynamicObject, ObjectList},
  Error,
};
use tokio::sync::MutexGuard;

use super::{
  utils::{self, UNKNOWN},
  App,
};

#[derive(Clone)]
pub struct NodeMetrics {
  pub name: String,
  pub cpu: String,
  pub cpu_percent: f64,
  pub mem: String,
  pub mem_percent: f64,
}

#[derive(Clone)]
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
}

static NODE_LABEL_PREFIX: &str = "node-role.kubernetes.io/";
static NODE_LABEL_ROLE: &str = "kubernetes.io/role";
static NONE_ROLE: &str = "<none>";

impl KubeNode {
  pub fn from_api(
    node: &Node,
    pods_list: &Result<ObjectList<Pod>, Error>,
    app: &mut MutexGuard<App>,
  ) -> Self {
    let node_name = node.metadata.name.clone().unwrap_or_default();
    let unschedulable = &node
      .spec
      .as_ref()
      .map_or(false, |s| s.unschedulable.unwrap_or(false));

    let (status, version, cpu_a, mem_a) = match &node.status {
      Some(stat) => {
        let status = if *unschedulable {
          Some("Unschedulable".into())
        } else {
          match &stat.conditions {
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
        let version = stat.node_info.as_ref().map(|i| i.kubelet_version.clone());

        let (cpu, mem) = stat.allocatable.as_ref().map_or((None, None), |a| {
          (
            a.get("cpu").map(|i| i.0.clone()),
            a.get("memory").map(|i| i.0.clone()),
          )
        });

        (status, version, cpu, mem)
      }
      None => (None, None, None, None),
    };

    let pod_count = match &pods_list {
      Ok(ps) => ps.iter().fold(0, |acc, p| {
        let p_node_name = p.spec.as_ref().and_then(|s| s.node_name.clone());
        p_node_name.map_or(acc, |v| if v == node_name { acc + 1 } else { acc })
      }),
      _ => 0,
    };

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
    }
  }
}

impl NodeMetrics {
  pub fn from_api(metric: &DynamicObject, app: &MutexGuard<App>) -> Self {
    let name = metric.metadata.name.clone().unwrap_or_default();

    let (cpu_percent, mem_percent) = match app.data.node_metrics.iter().find(|it| it.name == name) {
      Some(nm) => (nm.cpu_percent, nm.mem_percent),
      None => (0f64, 0f64),
    };

    NodeMetrics {
      name: metric.metadata.name.clone().unwrap_or_default(),
      cpu: utils::cpu_to_milli(
        metric.data["usage"]["cpu"]
          .to_string()
          .trim_matches('"')
          .into(),
      ),
      mem: utils::mem_to_mi(
        metric.data["usage"]["memory"]
          .to_string()
          .trim_matches('"')
          .into(),
      ),
      cpu_percent,
      mem_percent,
    }
  }
}
