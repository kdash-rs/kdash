// Based on https://github.com/davidB/kubectl-view-allocations

use kube::api::ObjectMeta;
use kubectl_view_allocations::metrics::Usage;
use serde::{Deserialize, Serialize};
use tokio::sync::MutexGuard;

use super::{utils, App};

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
  pub fn from_api(metric: &NodeMetrics, app: &MutexGuard<App>) -> Self {
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

#[cfg(test)]
mod tests {
  use tokio::sync::Mutex;

  use crate::app::test_utils::load_resource_from_file;

  use super::*;

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
