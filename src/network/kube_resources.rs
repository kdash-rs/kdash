use super::super::app::{KubeContext, KubeNode, KubeNs, KubePods, KubeSvs, NodeMetrics};
use super::{Network, UNKNOWN};

use anyhow::anyhow;
use duct::cmd;
use k8s_openapi::{
  api::core::v1::{Namespace, Node, Pod, Service, ServicePort},
  apimachinery::pkg::apis::meta::v1::Time,
  chrono::{DateTime, Utc},
};
use kube::{api::ListParams, config::Kubeconfig, Api, Resource};

impl<'a> Network<'a> {
  pub async fn get_kube_config(&mut self) {
    match Kubeconfig::read() {
      Ok(config) => {
        let mut app = self.app.lock().await;
        app.set_contexts(get_contexts(&config));
        app.data.kubeconfig = Some(config);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  // ideally should be done using API but the kube-rs crate doesn't support metrics yet so this is a temporary and dirty work around and is definitely the worst way to do this (yes, I almost cried doing this) as top command cannot output json or yaml
  pub async fn get_top_node(&mut self) {
    if let Ok(out) = cmd!("kubectl", "top", "node").read() {
      // the output would be a table so lets try to split into rows
      let rows: Vec<&str> = out.split('\n').collect();
      // lets discard first row as its header and any empty rows
      let rows: Vec<NodeMetrics> = rows
        .iter()
        .filter_map(|v| {
          if !v.trim().is_empty() && !v.trim().starts_with("NAME") {
            let cols: Vec<&str> = v.trim().split(' ').collect();
            let cols: Vec<&str> = cols
              .iter()
              .filter_map(|c| {
                if !c.trim().is_empty() {
                  Some(c.trim())
                } else {
                  None
                }
              })
              .collect();
            Some(NodeMetrics {
              name: cols[0].to_string(),
              cpu: cols[1].to_string(),
              cpu_percent: cols[2].to_string(),
              mem: cols[3].to_string(),
              mem_percent: cols[4].to_string(),
              cpu_percent_i: convert_to_f64(cols[2].trim_end_matches('%')),
              mem_percent_i: convert_to_f64(cols[4].trim_end_matches('%')),
            })
          } else {
            None
          }
        })
        .collect();
      let mut app = self.app.lock().await;
      app.data.node_metrics = rows;
    }
  }

  pub async fn get_nodes(&mut self) {
    let node_label_prefix = "node-role.kubernetes.io/";
    let node_label_role = "kubernetes.io/role";
    let none_role = "<none>";
    let lp = ListParams::default();
    let pods: Api<Pod> = Api::all(self.client.clone());
    let nodes: Api<Node> = Api::all(self.client.clone());

    match nodes.list(&lp).await {
      Ok(node_list) => {
        let pods_list = pods.list(&lp).await;

        let mut app = self.app.lock().await;

        let render_nodes = node_list
          .iter()
          .map(|node| {
            let unschedulable = &node
              .spec
              .as_ref()
              .map_or(false, |s| s.unschedulable.unwrap_or(false));

            let (status, version, cpu, mem) = match &node.status {
              Some(stat) => {
                let status = if *unschedulable {
                  Some("Unschedulable".to_string())
                } else {
                  match &stat.conditions {
                    Some(conds) => match conds
                      .iter()
                      .find(|c| c.type_ == "Ready" && c.status == "True")
                    {
                      Some(cond) => Some(cond.type_.clone()),
                      _ => Some("Not Ready".to_string()),
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
                let node_name = p.spec.as_ref().and_then(|s| s.node_name.clone());
                node_name.map_or(acc, |v| if v == node.name() { acc + 1 } else { acc })
              }),
              _ => 0,
            };

            let role = match &node.metadata.labels {
              Some(labels) => labels
                .iter()
                .filter_map(|(k, v)| {
                  return if k.starts_with(node_label_prefix) {
                    Some(k.trim_start_matches(node_label_prefix))
                  } else if k == node_label_role && !v.is_empty() {
                    Some(v)
                  } else {
                    None
                  };
                })
                .collect::<Vec<_>>()
                .join(","),
              None => none_role.to_string(),
            };

            let (cpu_percent, mem_percent) =
              match app.data.node_metrics.iter().find(|nm| nm.name == node.name()) {
                Some(nm) => (nm.cpu_percent.clone(), nm.mem_percent.clone()),
                None => (String::default(), String::default()),
              };

            KubeNode {
              name: node.name(),
              status: status.unwrap_or_else(|| UNKNOWN.to_string()),
              cpu: cpu.unwrap_or_default(),
              mem: kb_to_mb(mem.unwrap_or_default()),
              role: if role.is_empty() {
                none_role.to_string()
              } else {
                role
              },
              version: version.unwrap_or_default(),
              pods: pod_count,
              age: to_age(node.metadata.creation_timestamp.as_ref(), Utc::now()),
              cpu_percent,
              mem_percent,
            }
          })
          .collect::<Vec<_>>();

        app.data.nodes.set_items(render_nodes);
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
        app.data.namespaces.set_items(nss);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  pub async fn get_pods(&mut self) {
    let pods: Api<Pod> = self.get_namespaced_api().await;

    let lp = ListParams::default();
    match pods.list(&lp).await {
      Ok(pod_list) => {
        let render_pods = pod_list
          .iter()
          .map(|pod| {
            let (status, cr, restarts, c_stats_len) = match &pod.status {
              Some(stat) => {
                let (mut cr, mut rc) = (0, 0);
                let c_stats_len = match stat.container_statuses.as_ref() {
                  Some(c_stats) => {
                    c_stats.iter().for_each(|cs| {
                      if cs.ready {
                        cr += 1;
                      }
                      rc += cs.restart_count;
                    });
                    c_stats.len()
                  }
                  None => 0,
                };
                let status = match &stat.phase {
                  Some(phase) => phase.clone(),
                  _ => UNKNOWN.to_string(),
                };
                let status = match &stat.reason {
                  Some(r) => {
                    if r == "NodeLost" && pod.metadata.deletion_timestamp.is_some() {
                      "Unknown".to_string()
                    } else {
                      status
                    }
                  }
                  None => status,
                };
                // TODO handle more status possibilities from init-containers and containers

                (status, cr, rc, c_stats_len)
              }
              _ => (UNKNOWN.to_string(), 0, 0, 0),
            };

            KubePods {
              namespace: pod.namespace().unwrap_or_default(),
              name: pod.name(),
              ready: format!("{}/{}", cr, c_stats_len),
              restarts,
              // TODO implement pod metrics
              cpu: String::default(),
              mem: String::default(),
              status,
              age: to_age(pod.metadata.creation_timestamp.as_ref(), Utc::now()),
            }
          })
          .collect::<Vec<_>>();
        let mut app = self.app.lock().await;
        app.data.pods.set_items(render_pods);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  pub async fn get_services(&mut self) {
    let svc: Api<Service> = self.get_namespaced_api().await;

    let lp = ListParams::default();
    match svc.list(&lp).await {
      Ok(svc_list) => {
        let render_services = svc_list
          .iter()
          .map(|service| {
            let (type_, cluster_ip, external_ip, ports) = match &service.spec {
              Some(spec) => {
                let type_ = match &spec.type_ {
                  Some(type_) => type_.clone(),
                  _ => UNKNOWN.to_string(),
                };

                let external_ips = match type_.as_str() {
                  "ClusterIP" | "NodePort" => spec.external_ips.clone(),
                  "LoadBalancer" => Some(get_lb_ext_ips(service, spec.external_ips.clone())),
                  "ExternalName" => Some(vec![spec.external_name.clone().unwrap_or_default()]),
                  _ => None,
                }
                .unwrap_or_else(|| {
                  if type_ == "LoadBalancer" {
                    vec!["<pending>".to_string()]
                  } else {
                    vec![String::default()]
                  }
                });

                (
                  type_,
                  spec
                    .cluster_ip
                    .as_ref()
                    .unwrap_or(&"None".to_string())
                    .clone(),
                  external_ips.join(","),
                  get_ports(spec.ports.clone()).join(" "),
                )
              }
              _ => (
                UNKNOWN.to_string(),
                String::default(),
                String::default(),
                String::default(),
              ),
            };

            KubeSvs {
              name: service.name(),
              type_,
              namespace: service.namespace().unwrap_or_default(),
              cluster_ip,
              external_ip,
              ports,
              age: to_age(service.metadata.creation_timestamp.as_ref(), Utc::now()),
            }
          })
          .collect::<Vec<_>>();
        let mut app = self.app.lock().await;
        app.data.services.set_items(render_services);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_namespaced_api<K: Resource>(&mut self) -> Api<K>
  where
    <K as Resource>::DynamicType: Default,
  {
    let app = self.app.lock().await;
    match &app.data.selected_ns {
      Some(ns) => Api::namespaced(self.client.clone(), &ns),
      None => Api::all(self.client.clone()),
    }
  }
}

fn get_ports(sports: Option<Vec<ServicePort>>) -> Vec<String> {
  match sports {
    Some(ports) => ports
      .iter()
      .map(|s| {
        let mut port = String::new();
        if s.name.is_some() {
          port = format!("{}:", s.name.clone().unwrap());
        }
        port = format!("{}{}►{}", port, s.port, s.node_port.unwrap_or(0));
        if s.protocol.is_some() && s.protocol.clone().unwrap() == "TCP" {
          port = format!("{}/{}", port, s.protocol.clone().unwrap());
        }
        port
      })
      .collect(),
    None => vec![],
  }
}

fn get_lb_ext_ips(service: &Service, external_ips: Option<Vec<String>>) -> Vec<String> {
  let mut lb_ips = match &service.status {
    Some(ss) => match &ss.load_balancer {
      Some(lb) => {
        let ing = &lb.ingress;
        ing
          .clone()
          .unwrap_or_default()
          .iter()
          .map(|it| {
            if it.ip.is_some() {
              it.ip.clone().unwrap_or_default()
            } else if it.hostname.is_some() {
              it.hostname.clone().unwrap_or_default()
            } else {
              String::default()
            }
          })
          .collect::<Vec<String>>()
      }
      None => vec![],
    },
    None => vec![],
  };
  if external_ips.is_some() && !lb_ips.is_empty() {
    lb_ips.extend(external_ips.unwrap_or_default());
    lb_ips
  } else {
    lb_ips
  }
}

fn get_contexts(config: &Kubeconfig) -> Vec<KubeContext> {
  config
    .contexts
    .iter()
    .map(|it| KubeContext {
      name: it.name.clone(),
      cluster: it.context.cluster.clone(),
      user: it.context.user.clone(),
      namespace: it.context.namespace.clone(),
      is_active: is_active_context(&it.name, &config.current_context),
    })
    .collect::<Vec<KubeContext>>()
}

fn is_active_context(name: &str, current_ctx: &Option<String>) -> bool {
  match current_ctx {
    Some(ctx) => name == ctx,
    None => false,
  }
}

fn to_age(timestamp: Option<&Time>, against: DateTime<Utc>) -> String {
  match timestamp {
    Some(t) => {
      let t = t.0;
      let duration = against.signed_duration_since(t);

      let mut out = String::new();
      if duration.num_weeks() != 0 {
        out.push_str(format!("{}w", duration.num_weeks()).as_str());
      }
      let days = duration.num_days() - (duration.num_weeks() * 7);
      if days != 0 {
        out.push_str(format!("{}d", days).as_str());
      }
      let hrs = duration.num_hours() - (duration.num_days() * 24);
      if hrs != 0 {
        out.push_str(format!("{}h", hrs).as_str());
      }
      let mins = duration.num_minutes() - (duration.num_hours() * 60);
      if mins != 0 && days == 0 && duration.num_weeks() == 0 {
        out.push_str(format!("{}m", mins).as_str());
      }
      if out.is_empty() {
        "0m".to_string()
      } else {
        out
      }
    }
    None => String::default(),
  }
}

fn kb_to_mb(v: String) -> String {
  let vint = v.trim_end_matches("Ki").parse::<i64>().unwrap_or(0);

  (vint / 1024).to_string()
}

fn convert_to_f64(s: &str) -> f64 {
  s.parse().unwrap_or(0f64)
}

#[cfg(test)]
mod tests {
  #[test]
  fn test_kb_to_mb() {
    use super::kb_to_mb;
    assert_eq!(kb_to_mb(String::from("2888180")), String::from("2820"));
    assert_eq!(kb_to_mb(String::from("2888180Ki")), String::from("2820"));
  }
  #[test]
  fn test_to_age() {
    use super::to_age;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
    use k8s_openapi::chrono::TimeZone;
    use k8s_openapi::chrono::{DateTime, Utc};
    use std::time::SystemTime;

    fn get_time(s: &str) -> Time {
      Time(to_utc(s))
    }

    fn to_utc(s: &str) -> DateTime<Utc> {
      Utc.datetime_from_str(s, "%d-%m-%Y %H:%M:%S").unwrap()
    }

    assert_eq!(
      to_age(Some(&Time(Utc::now())), Utc::now()),
      String::from("0m")
    );
    assert_eq!(
      to_age(
        Some(&get_time("15-4-2021 14:09:00")),
        to_utc("15-4-2021 14:10:00")
      ),
      String::from("1m")
    );
    assert_eq!(
      to_age(
        Some(&get_time("15-4-2021 13:50:00")),
        to_utc("15-4-2021 14:10:00")
      ),
      String::from("20m")
    );
    assert_eq!(
      to_age(
        Some(&get_time("15-4-2021 13:50:10")),
        to_utc("15-4-2021 14:10:0")
      ),
      String::from("19m")
    );
    assert_eq!(
      to_age(
        Some(&get_time("15-4-2021 10:50:10")),
        to_utc("15-4-2021 14:10:0")
      ),
      String::from("3h19m")
    );
    assert_eq!(
      to_age(
        Some(&get_time("14-4-2021 15:10:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("23h")
    );
    assert_eq!(
      to_age(
        Some(&get_time("14-4-2021 14:11:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("23h59m")
    );
    assert_eq!(
      to_age(
        Some(&get_time("14-4-2021 14:10:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("1d")
    );
    assert_eq!(
      to_age(
        Some(&get_time("12-4-2021 14:10:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("3d")
    );
    assert_eq!(
      to_age(
        Some(&get_time("12-4-2021 13:50:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("3d")
    );
    assert_eq!(
      to_age(
        Some(&get_time("12-4-2021 11:10:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("3d3h")
    );
    assert_eq!(
      to_age(
        Some(&get_time("12-4-2021 10:50:10")),
        to_utc("15-4-2021 14:10:0")
      ),
      String::from("3d3h")
    );
    assert_eq!(
      to_age(
        Some(&get_time("08-4-2021 14:10:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("1w")
    );
    assert_eq!(
      to_age(
        Some(&get_time("05-4-2021 12:30:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("1w3d1h")
    );
    assert_eq!(
      to_age(
        Some(&Time(DateTime::from(SystemTime::UNIX_EPOCH))),
        to_utc("15-4-2021 14:10:0")
      ),
      String::from("2676w14h")
    );
  }
}
