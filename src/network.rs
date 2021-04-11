// adapted from https://github.com/Rigellute/spotify-tui
use crate::app::{self, App};
use crate::config::ClientConfig;
use anyhow::{anyhow, Result};
use duct::cmd;
use kube::{api::ListParams, Api, Client};
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
  GetPods,
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
      IoEvent::GetPods => {
        self.get_pods().await;
      }
      IoEvent::GetCLIInfo => {
        self.get_cli_info().await;
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
    let NOT_FOUND: String = String::from("Not found");
    let mut app = self.app.lock().await;

    let (version, status) = match cmd!("kubectl", "version", "--client", "-o", "json").read() {
      Ok(out) => {
        let v: serde_json::Result<JValue> = serde_json::from_str(&*out);
        match v {
          Ok(val) => (val["clientVersion"]["gitVersion"].to_string(), true),
          _ => (NOT_FOUND.clone(), false),
        }
      }
      _ => (NOT_FOUND.clone(), false),
    };

    app.CLIs.push(app::CLI {
      name: "kubectl".to_string(),
      version: version.replace('"', ""),
      status,
    });

    let (version, status) =
      match cmd!("docker", "version", "--format", "'{{.Client.Version}}'").read() {
        Ok(out) => (out, true),
        _ => (NOT_FOUND.clone(), false),
      };

    app.CLIs.push(app::CLI {
      name: "docker".to_string(),
      version: format!("v{}", version.replace("'", "")),
      status,
    });

    let (version, status) = match cmd!("docker-compose", "version", "--short").read() {
      Ok(out) => (out, true),
      _ => (NOT_FOUND.clone(), false),
    };

    app.CLIs.push(app::CLI {
      name: "docker-compose".to_string(),
      version: format!("v{}", version.replace("'", "")),
      status,
    });

    let (version, status) =
      get_info_by_regex("kind", &vec!["version"], r"(v[0-9.]+)", NOT_FOUND.clone());

    app.CLIs.push(app::CLI {
      name: "kind".to_string(),
      version,
      status,
    });

    let (version, status) = get_info_by_regex(
      "helm",
      &vec!["version", "-c"],
      r"(v[0-9.]+)",
      NOT_FOUND.clone(),
    );

    app.CLIs.push(app::CLI {
      name: "helm".to_string(),
      version,
      status,
    });

    let (version, status) = get_info_by_regex(
      "istioctl",
      &vec!["version"],
      r"([0-9.]+)",
      NOT_FOUND.clone(),
    );

    app.CLIs.push(app::CLI {
      name: "istioctl".to_string(),
      version: format!("v{}", version),
      status,
    });

    app.CLIs.push(app::CLI {
      name: "kdash".to_string(),
      version: format!("v{}", env!("CARGO_PKG_VERSION")),
      status,
    });
  }

  async fn get_pods(&mut self) {
    // match self.client.current_user().await {
    //   Ok(user) => {
    //     let mut app = self.app.lock().await;
    //     app.user = Some(user);
    //   }
    //   Err(e) => {
    //     self.handle_error(anyhow!(e)).await;
    //   }
    // }
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
