use super::super::app::{KubeContext, KubeNode, KubeNs, KubePods, KubeSvs, NodeMetrics};
use super::{Network, UNKNOWN};

use anyhow::anyhow;
use k8s_openapi::{
  api::core::v1::{Namespace, Node, Pod, Service, ServicePort},
  apimachinery::pkg::apis::meta::v1::Time,
  chrono::{DateTime, Utc},
};
use kube::{
  api::{DynamicObject, GroupVersionKind, ListParams},
  config::Kubeconfig,
  Api, Resource,
};

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

  pub async fn get_top_node(&mut self) {
    let gvk = GroupVersionKind::gvk("metrics.k8s.io", "v1beta1", "nodemetrics").unwrap();
    let node_metrics: Api<DynamicObject> = Api::all_with(self.client.clone(), &gvk);
    match node_metrics.list(&ListParams::default()).await {
      Ok(metrics) => {
        let mut app = self.app.lock().await;

        let rows = metrics
          .items
          .iter()
          .map(|it| {
            let name = it.metadata.name.clone().unwrap_or_default();

            let (cpu_percent, mem_percent) =
              match app.data.node_metrics.iter().find(|it| it.name == name) {
                Some(nm) => (nm.cpu_percent, nm.mem_percent),
                None => (0f64, 0f64),
              };

            NodeMetrics {
              name: it.metadata.name.clone().unwrap_or_default(),
              cpu: cpu_to_milli(
                it.data["usage"]["cpu"]
                  .to_string()
                  .trim_matches('"')
                  .to_string(),
              ),
              mem: mem_to_mi(
                it.data["usage"]["memory"]
                  .to_string()
                  .trim_matches('"')
                  .to_string(),
              ),
              cpu_percent,
              mem_percent,
            }
          })
          .collect();

        app.data.node_metrics = rows;
      }
      Err(_) => {
        let mut app = self.app.lock().await;
        app.data.node_metrics = vec![];
      }
    };
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
        self.get_top_node().await;
        let pods_list = pods.list(&lp).await;

        let mut app = self.app.lock().await;

        let render_nodes = node_list
          .iter()
          .map(|node| {
            let unschedulable = &node
              .spec
              .as_ref()
              .map_or(false, |s| s.unschedulable.unwrap_or(false));

            let (status, version, cpu_a, mem_a) = match &node.status {
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

            let (cpu, cpu_percent, mem, mem_percent) = match app
              .data
              .node_metrics
              .iter_mut()
              .find(|nm| nm.name == node.name())
            {
              Some(nm) => {
                let cpu_percent = to_cpu_percent(
                  nm.cpu.clone(),
                  cpu_to_milli(cpu_a.clone().unwrap_or_default()),
                );
                nm.cpu_percent = cpu_percent;
                let mem_percent =
                  to_mem_percent(nm.mem.clone(), mem_to_mi(mem_a.clone().unwrap_or_default()));
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
              name: node.name(),
              status: status.unwrap_or_else(|| UNKNOWN.to_string()),
              role: if role.is_empty() {
                none_role.to_string()
              } else {
                role
              },
              version: version.unwrap_or_default(),
              pods: pod_count,
              age: to_age(node.metadata.creation_timestamp.as_ref(), Utc::now()),
              cpu,
              mem,
              cpu_a: cpu_to_milli(cpu_a.unwrap_or_default()),
              mem_a: mem_to_mi(mem_a.unwrap_or_default()),
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

fn mem_to_mi(v: String) -> String {
  if v.ends_with("Ki") {
    let v_int = v.trim_end_matches("Ki").parse::<i64>().unwrap_or(0);
    format!("{}Mi", v_int / 1024)
  } else if v.ends_with("Gi") {
    let v_int = v.trim_end_matches("Gi").parse::<i64>().unwrap_or(0);
    format!("{}Mi", v_int * 1024)
  } else {
    v
  }
}
fn cpu_to_milli(v: String) -> String {
  if v.ends_with('m') {
    v
  } else if v.ends_with('n') {
    format!(
      "{}m",
      (convert_to_f64(v.trim_end_matches('n')) / 1000000f64).floor()
    )
  } else {
    format!("{}m", (convert_to_f64(&v) * 1000f64).floor())
  }
}

fn to_cpu_percent(used: String, total: String) -> f64 {
  // convert from nano cpu to milli cpu
  let used = convert_to_f64(used.trim_end_matches('m'));
  let total = convert_to_f64(total.trim_end_matches('m'));

  to_percent(used, total)
}

fn to_mem_percent(used: String, total: String) -> f64 {
  let used = convert_to_f64(used.trim_end_matches("Mi"));
  let total = convert_to_f64(total.trim_end_matches("Mi"));

  to_percent(used, total)
}

fn to_percent(used: f64, total: f64) -> f64 {
  ((used / total) * 100f64).floor()
}

fn convert_to_f64(s: &str) -> f64 {
  s.parse().unwrap_or(0f64)
}

#[cfg(test)]
mod tests {
  #[test]
  fn test_mem_to_mi() {
    use super::mem_to_mi;
    assert_eq!(mem_to_mi(String::from("2820Mi")), String::from("2820Mi"));
    assert_eq!(mem_to_mi(String::from("2888180Ki")), String::from("2820Mi"));
    assert_eq!(mem_to_mi(String::from("5Gi")), String::from("5120Mi"));
    assert_eq!(mem_to_mi(String::from("5")), String::from("5"));
  }
  #[test]
  fn test_to_cpu_percent() {
    use super::to_cpu_percent;
    assert_eq!(
      to_cpu_percent(String::from("126m"), String::from("940m")),
      13f64
    );
  }
  #[test]
  fn test_to_mem_percent() {
    use super::to_mem_percent;
    assert_eq!(
      to_mem_percent(String::from("645784Mi"), String::from("2888184Mi")),
      22f64
    );
  }
  #[test]
  fn test_cpu_to_milli() {
    use super::cpu_to_milli;
    assert_eq!(cpu_to_milli(String::from("645m")), String::from("645m"));
    assert_eq!(
      cpu_to_milli(String::from("126632173n")),
      String::from("126m")
    );
    assert_eq!(cpu_to_milli(String::from("8")), String::from("8000m"));
    assert_eq!(cpu_to_milli(String::from("0")), String::from("0m"));
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
