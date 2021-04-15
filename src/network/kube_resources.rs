use crate::app::{KubeNode, KubeNs, KubePods, KubeSvs};
use anyhow::anyhow;
use k8s_openapi::{
  api::core::v1::{Namespace, Node, Pod, Service},
  apimachinery::pkg::apis::meta::v1::Time,
  chrono::Utc,
};
use kube::{api::ListParams, Api, Resource};

use super::{Network, UNKNOWN};

impl<'a> Network<'a> {
  pub async fn get_nodes(&mut self) {
    let nodes: Api<Node> = Api::all(self.client.clone());

    let lp = ListParams::default();
    let pods: Api<Pod> = Api::all(self.client.clone());
    match nodes.list(&lp).await {
      Ok(node_list) => {
        let pods = pods.list(&lp).await;

        let nodes = node_list
          .iter()
          .map(|it| {
            let unschedulable = &it
              .spec
              .as_ref()
              .map_or(false, |s| s.unschedulable.unwrap_or(false));

            let (status, version, cpu, mem) = match &it.status {
              Some(stat) => {
                let status = if *unschedulable {
                  "Unschedulable".to_string()
                } else {
                  match &stat.conditions {
                    Some(conds) => match conds
                      .into_iter()
                      .find(|c| c.type_ == "Ready" && c.status == "True")
                    {
                      Some(cond) => cond.type_.clone(),
                      _ => "Not Ready".to_string(),
                    },
                    _ => UNKNOWN.to_string(),
                  }
                };
                let version = stat
                  .node_info
                  .as_ref()
                  .map_or(String::new(), |i| i.kernel_version.clone());

                let (cpu, mem) = stat.allocatable.as_ref().map_or((0, 0), |a| {
                  (
                    a.get("cpu")
                      .map_or(0, |i| i.0.as_str().parse::<i64>().unwrap_or(0)),
                    a.get("mem")
                      .map_or(0, |i| i.0.as_str().parse::<i64>().unwrap_or(0)),
                  )
                });

                (status, version, cpu, mem)
              }
              _ => (UNKNOWN.to_string(), String::new(), 0, 0),
            };

            let pod_count = match &pods {
              Ok(ps) => ps.iter().fold(0, |acc, p| {
                let node = p.spec.as_ref().map_or(None, |s| s.node_name.clone());
                node.map_or(acc, |v| if v == it.name() { acc + 1 } else { acc })
              }),
              _ => 0,
            };

            KubeNode {
              name: it.name(),
              status,
              cpu,
              mem,
              role: String::new(),
              version,
              pods: pod_count,
              age: to_age(it.metadata.creation_timestamp.as_ref()),
            }
          })
          .collect::<Vec<_>>();
        let mut app = self.app.lock().await;
        app.nodes.set_items(nodes);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  pub async fn get_namespaces(&mut self) {
    let ns: Api<Namespace> = Api::all(self.client.clone());

    let lp = ListParams::default();
    match ns.list(&lp).await {
      Ok(ns_list) => {
        let mut app = self.app.lock().await;
        let nss = ns_list
          .iter()
          .map(|it| {
            let status = match &it.status {
              Some(stat) => match &stat.phase {
                Some(phase) => phase.clone(),
                _ => UNKNOWN.to_string(),
              },
              _ => UNKNOWN.to_string(),
            };

            KubeNs {
              name: it.name(),
              status,
            }
          })
          .collect::<Vec<_>>();
        app.namespaces.set_items(nss);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  pub async fn get_pods(&mut self) {
    let pods = get_pods_api(self).await;

    let lp = ListParams::default();
    match pods.list(&lp).await {
      Ok(pod_list) => {
        let pods = pod_list
          .iter()
          .map(|it| {
            let status = match &it.status {
              Some(stat) => match &stat.phase {
                Some(phase) => phase.clone(),
                _ => UNKNOWN.to_string(),
              },
              _ => UNKNOWN.to_string(),
            };

            KubePods {
              name: it.name(),
              namespace: it.namespace().unwrap_or("".to_string()),
              ready: "".to_string(),
              restarts: 0,
              cpu: "".to_string(),
              mem: "".to_string(),
              status,
            }
          })
          .collect::<Vec<_>>();
        let mut app = self.app.lock().await;
        app.pods.set_items(pods);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  pub async fn get_services(&mut self) {
    let svs: Api<Service> = Api::all(self.client.clone());

    let lp = ListParams::default();
    match svs.list(&lp).await {
      Ok(svc_list) => {
        let svs = svc_list
          .iter()
          .map(|it| {
            let type_ = match &it.spec {
              Some(spec) => match &spec.type_ {
                Some(type_) => type_.clone(),
                _ => UNKNOWN.to_string(),
              },
              _ => UNKNOWN.to_string(),
            };

            KubeSvs {
              name: it.name(),
              type_,
            }
          })
          .collect::<Vec<_>>();
        let mut app = self.app.lock().await;
        app.services.set_items(svs);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }
}

async fn get_pods_api<'a>(network: &mut Network<'a>) -> Api<Pod> {
  let app = network.app.lock().await;
  match &app.selected_ns {
    Some(ns) => Api::namespaced(network.client.clone(), &ns),
    None => Api::all(network.client.clone()),
  }
}

fn to_age(timestamp: Option<&Time>) -> String {
  match timestamp {
    Some(t) => {
      let t = t.0.time();
      let now = Utc::now().time();
      let diff = now - t;
      diff.num_minutes().to_string()
    }
    None => "".to_string(),
  }
}

// TODO find a way to do this as the kube-rs lib doesn't support metrics yet
//   async fn get_node_metrics(&mut self) {
//     let m: Api<ResourceMetricSource> = Api::all(self.client.clone());
//     let lp = ListParams::default();

//     let a = m.list(lp).await.unwrap();
//   }
