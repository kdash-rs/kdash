// adapted from https://github.com/Rigellute/spotify-tui
use crate::app::{self, App, KubeContext, KubeNode, KubeNs, KubePods, KubeSvs, CLI};
use anyhow::anyhow;
use duct::cmd;
use k8s_openapi::{
  api::core::v1::{Namespace, Node, Pod, Service},
  apimachinery::pkg::apis::meta::v1::Time,
  chrono,
};
use kube::{
  api::{Api, ListParams, Resource},
  config::Kubeconfig,
  Client,
};
use regex::Regex;
use serde_json::Value as JValue;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
pub enum IoEvent {
  GetCLIInfo,
  GetKubeConfig,
  GetNodes,
  GetNamespaces,
  GetPods,
  GetServices,
  RefreshClient,
}

pub async fn get_client() -> kube::Result<Client> {
  Client::try_default().await
}

#[derive(Clone)]
pub struct Network<'a> {
  pub client: Client,
  pub app: &'a Arc<Mutex<App>>,
}

static UNKNOWN: &'static str = "Unknown";
static NOT_FOUND: &'static str = "Not found";

impl<'a> Network<'a> {
  pub fn new(client: Client, app: &'a Arc<Mutex<App>>) -> Self {
    Network { client, app }
  }

  pub async fn refresh_client(&mut self) {
    // TODO find a better way to do this
    match get_client().await {
      Ok(client) => {
        self.client = client;
        let mut app = self.app.lock().await;
        app.reset();
      }
      Err(e) => self.handle_error(anyhow!(e)).await,
    };
  }

  #[allow(clippy::cognitive_complexity)]
  pub async fn handle_network_event(&mut self, io_event: IoEvent) {
    match io_event {
      IoEvent::RefreshClient => {
        self.refresh_client().await;
      }
      IoEvent::GetCLIInfo => {
        self.get_cli_info().await;
      }
      IoEvent::GetKubeConfig => {
        self.get_kube_config().await;
      }
      IoEvent::GetNodes => {
        self.get_nodes().await;
      }
      IoEvent::GetNamespaces => {
        self.get_namespaces().await;
      }
      IoEvent::GetPods => {
        self.get_pods().await;
      }
      IoEvent::GetServices => {
        self.get_services().await;
      }
    };

    let mut app = self.app.lock().await;
    app.is_loading = false;
  }

  async fn handle_error(&mut self, e: anyhow::Error) {
    let mut app = self.app.lock().await;
    app.handle_error(e);
  }

  async fn get_cli_info(&mut self) {
    let mut clis: Vec<CLI> = vec![];

    let (version, status) = match cmd!("kubectl", "version", "--client", "-o", "json").read() {
      Ok(out) => {
        let v: serde_json::Result<JValue> = serde_json::from_str(&*out);
        match v {
          Ok(val) => (val["clientVersion"]["gitVersion"].to_string(), true),
          _ => (NOT_FOUND.to_string(), false),
        }
      }
      _ => (NOT_FOUND.to_string(), false),
    };

    clis.push(app::CLI {
      name: "kubectl".to_string(),
      version: version.replace('"', ""),
      status,
    });

    let (version, status) = cmd!("docker", "version", "--format", "'{{.Client.Version}}'")
      .read()
      .map_or((NOT_FOUND.to_string(), false), |out| (out, true));

    clis.push(app::CLI {
      name: "docker".to_string(),
      version: format!("v{}", version.replace("'", "")),
      status,
    });

    let (version, status) = cmd!("docker-compose", "version", "--short")
      .read()
      .map_or((NOT_FOUND.to_string(), false), |out| (out, true));

    clis.push(app::CLI {
      name: "docker-compose".to_string(),
      version: format!("v{}", version.replace("'", "")),
      status,
    });

    let (version, status) = get_info_by_regex(
      "kind",
      &vec!["version"],
      r"(v[0-9.]+)",
      NOT_FOUND.to_string(),
    );

    clis.push(app::CLI {
      name: "kind".to_string(),
      version,
      status,
    });

    let (version, status) = get_info_by_regex(
      "helm",
      &vec!["version", "-c"],
      r"(v[0-9.]+)",
      NOT_FOUND.to_string(),
    );

    clis.push(app::CLI {
      name: "helm".to_string(),
      version,
      status,
    });

    let (version, status) = get_info_by_regex(
      "istioctl",
      &vec!["version"],
      r"([0-9.]+)",
      NOT_FOUND.to_string(),
    );

    clis.push(app::CLI {
      name: "istioctl".to_string(),
      version: format!("v{}", version),
      status,
    });

    let mut app = self.app.lock().await;
    app.clis = clis;
  }

  async fn get_kube_config(&mut self) {
    match Kubeconfig::read() {
      Ok(config) => {
        let mut app = self.app.lock().await;
        app.set_contexts(get_contexts(&config));
        app.kubeconfig = Some(config);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_nodes(&mut self) {
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

  // TODO find a way to do this as the kube-rs lib doesn't support metrics yet
  //   async fn get_node_metrics(&mut self) {
  //     let m: Api<ResourceMetricSource> = Api::all(self.client.clone());
  //     let lp = ListParams::default();

  //     let a = m.list(lp).await.unwrap();
  //   }

  async fn get_namespaces(&mut self) {
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

  async fn get_pods_api(&mut self) -> Api<Pod> {
    let app = self.app.lock().await;
    match &app.selected_ns {
      Some(ns) => Api::namespaced(self.client.clone(), &ns),
      None => Api::all(self.client.clone()),
    }
  }

  async fn get_pods(&mut self) {
    let pods = self.get_pods_api().await;

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

  async fn get_services(&mut self) {
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

// utils
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

fn is_active_context(name: &String, current_ctx: &Option<String>) -> bool {
  match current_ctx {
    Some(ctx) => name == ctx,
    None => false,
  }
}

// execute a command and get info from it using regex
fn get_info_by_regex(command: &str, args: &[&str], regex: &str, default: String) -> (String, bool) {
  match cmd(command, args).read() {
    Ok(out) => match Regex::new(regex) {
      Ok(re) => match re.captures(out.as_str()) {
        Some(cap) => match cap.get(1) {
          Some(text) => (text.as_str().to_string(), true),
          _ => (default, false),
        },
        _ => (default, false),
      },
      _ => (default, false),
    },
    _ => (default, false),
  }
}

fn to_age(timestamp: Option<&Time>) -> String {
  match timestamp {
    Some(t) => {
      let t = t.0.time();
      let now = chrono::Utc::now().time();
      let diff = now - t;
      diff.num_minutes().to_string()
    }
    None => "".to_string(),
  }
}
