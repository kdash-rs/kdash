// adapted from https://github.com/Rigellute/spotify-tui
use crate::app::{self, App, KubeContext, KubeNode, KubeNs, KubePods, KubeSvs, StatefulTable};
use crate::config::ClientConfig;
use anyhow::{anyhow, Result};
use duct::cmd;
use k8s_openapi::api::core::v1::{Event, Namespace, Node, Pod, Service};
use kube::{
  api::{Api, ListParams, Resource},
  config::Kubeconfig,
  Client,
};
use kube_runtime::{reflector, utils::try_flatten_applied, watcher};
use regex::Regex;
use serde_json::{map::Map, Value as JValue};
use serde_yaml::Value as YValue;
use std::{
  sync::Arc,
  time::{Duration, Instant, SystemTime},
};
use tokio::sync::Mutex;
use tokio::try_join;

#[derive(Debug)]
pub enum IoEvent {
  GetCLIInfo,
  GetKubeConfig,
  GetNodes,
  GetNamespaces,
  GetPods,
  GetServices,
}

pub async fn get_client() -> kube::Result<Client> {
  Client::try_default().await
}

#[derive(Clone)]
pub struct Network<'a> {
  pub client: Client,
  pub client_config: ClientConfig,
  pub app: &'a Arc<Mutex<App>>,
}

impl<'a> Network<'a> {
  pub fn new(client: Client, client_config: ClientConfig, app: &'a Arc<Mutex<App>>) -> Self {
    Network {
      client,
      client_config,
      app,
    }
  }

  #[allow(clippy::cognitive_complexity)]
  pub async fn handle_network_event(&mut self, io_event: IoEvent) {
    match io_event {
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
    let not_found = String::from("Not found");
    let mut app = self.app.lock().await;

    let (version, status) = match cmd!("kubectl", "version", "--client", "-o", "json").read() {
      Ok(out) => {
        let v: serde_json::Result<JValue> = serde_json::from_str(&*out);
        match v {
          Ok(val) => (val["clientVersion"]["gitVersion"].to_string(), true),
          _ => (not_found.clone(), false),
        }
      }
      _ => (not_found.clone(), false),
    };

    app.clis.push(app::CLI {
      name: "kubectl".to_string(),
      version: version.replace('"', ""),
      status,
    });

    let (version, status) =
      match cmd!("docker", "version", "--format", "'{{.Client.Version}}'").read() {
        Ok(out) => (out, true),
        _ => (not_found.clone(), false),
      };

    app.clis.push(app::CLI {
      name: "docker".to_string(),
      version: format!("v{}", version.replace("'", "")),
      status,
    });

    let (version, status) = match cmd!("docker-compose", "version", "--short").read() {
      Ok(out) => (out, true),
      _ => (not_found.clone(), false),
    };

    app.clis.push(app::CLI {
      name: "docker-compose".to_string(),
      version: format!("v{}", version.replace("'", "")),
      status,
    });

    let (version, status) =
      get_info_by_regex("kind", &vec!["version"], r"(v[0-9.]+)", not_found.clone());

    app.clis.push(app::CLI {
      name: "kind".to_string(),
      version,
      status,
    });

    let (version, status) = get_info_by_regex(
      "helm",
      &vec!["version", "-c"],
      r"(v[0-9.]+)",
      not_found.clone(),
    );

    app.clis.push(app::CLI {
      name: "helm".to_string(),
      version,
      status,
    });

    let (version, status) = get_info_by_regex(
      "istioctl",
      &vec!["version"],
      r"([0-9.]+)",
      not_found.clone(),
    );

    app.clis.push(app::CLI {
      name: "istioctl".to_string(),
      version: format!("v{}", version),
      status,
    });

    app.clis.push(app::CLI {
      name: "kdash".to_string(),
      version: format!("v{}", env!("CARGO_PKG_VERSION")),
      status,
    });
  }

  async fn get_kube_config(&mut self) {
    match Kubeconfig::read() {
      Ok(config) => {
        let mut app = self.app.lock().await;
        let contexts = get_contexts(&config);
        let active_context = contexts.clone().into_iter().find(|it| it.is_active);
        app.contexts = StatefulTable::with_items(contexts);
        app.active_context = active_context;
        app.kubeconfig = Some(config);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_nodes(&mut self) {
    let nodes: Api<Node> = Api::all(self.client.clone());
    let unknown: String = String::from("Unknown");

    let lp = ListParams::default();
    match nodes.list(&lp).await {
      Ok(node_list) => {
        let mut app = self.app.lock().await;
        let nodes = node_list
          .iter()
          .map(|it| {
            let status = match &it.status {
              Some(stat) => match &stat.conditions {
                Some(conds) => match conds.into_iter().last() {
                  Some(cond) => cond.type_.clone(),
                  _ => unknown.clone(),
                },
                _ => unknown.clone(),
              },
              _ => unknown.clone(),
            };
            KubeNode {
              name: it.name(),
              status,
              cpu: 0,
              mem: 0,
            }
          })
          .collect::<Vec<_>>();
        app.nodes = nodes;
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_namespaces(&mut self) {
    let ns: Api<Namespace> = Api::all(self.client.clone());
    let unknown: String = String::from("Unknown");

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
                _ => unknown.clone(),
              },
              _ => unknown.clone(),
            };

            KubeNs {
              name: it.name(),
              status,
            }
          })
          .collect::<Vec<_>>();
        app.namespaces = nss;
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_pods(&mut self) {
    let pods: Api<Pod> = Api::all(self.client.clone());
    let unknown: String = String::from("Unknown");

    let lp = ListParams::default();
    match pods.list(&lp).await {
      Ok(pod_list) => {
        let mut app = self.app.lock().await;
        let pods = pod_list
          .iter()
          .map(|it| {
            let status = match &it.status {
              Some(stat) => match &stat.phase {
                Some(phase) => phase.clone(),
                _ => unknown.clone(),
              },
              _ => unknown.clone(),
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
        app.pods = pods;
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_services(&mut self) {
    let svs: Api<Service> = Api::all(self.client.clone());
    let unknown: String = String::from("Unknown");

    let lp = ListParams::default();
    match svs.list(&lp).await {
      Ok(svc_list) => {
        let mut app = self.app.lock().await;
        let svs = svc_list
          .iter()
          .map(|it| {
            let type_ = match &it.spec {
              Some(spec) => match &spec.type_ {
                Some(type_) => type_.clone(),
                _ => unknown.clone(),
              },
              _ => unknown.clone(),
            };

            KubeSvs {
              name: it.name(),
              type_,
            }
          })
          .collect::<Vec<_>>();
        app.services = svs;
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
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
      is_active: is_active_context(it.name.clone(), config.current_context.clone()),
    })
    .collect::<Vec<KubeContext>>()
}

fn is_active_context(name: String, current_ctx: Option<String>) -> bool {
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
