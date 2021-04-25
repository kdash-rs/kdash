use super::app::{self, models::ScrollableTxt, App, Cli};

use duct::cmd;
use regex::Regex;
use serde_json::Value as JValue;

use anyhow::anyhow;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
pub enum IoCmdEvent {
  GetCliInfo,
  GetDescribe {
    kind: String,
    value: String,
    ns: Option<String>,
  },
}

#[derive(Clone)]
pub struct CmdRunner<'a> {
  pub app: &'a Arc<Mutex<App>>,
}

static NOT_FOUND: &str = "Not found";

impl<'a> CmdRunner<'a> {
  pub fn new(app: &'a Arc<Mutex<App>>) -> Self {
    CmdRunner { app }
  }

  pub async fn handle_cmd_event(&mut self, io_event: IoCmdEvent) {
    match io_event {
      IoCmdEvent::GetCliInfo => {
        self.get_cli_info().await;
      }
      IoCmdEvent::GetDescribe { kind, value, ns } => {
        self.get_describe(kind, value, ns).await;
      }
    };

    let mut app = self.app.lock().await;
    app.is_loading = false;
  }

  async fn handle_error(&self, e: anyhow::Error) {
    let mut app = self.app.lock().await;
    app.handle_error(e);
  }

  async fn get_cli_info(&self) {
    let mut clis: Vec<Cli> = vec![];

    let (version, status) = match cmd!("kubectl", "version", "--client", "-o", "json").read() {
      Ok(out) => {
        let v: serde_json::Result<JValue> = serde_json::from_str(&*out);
        match v {
          Ok(val) => (val["clientVersion"]["gitVersion"].to_string(), true),
          _ => (NOT_FOUND.into(), false),
        }
      }
      _ => (NOT_FOUND.into(), false),
    };

    clis.push(app::Cli {
      name: "kubectl".to_owned(),
      version: version.replace('"', ""),
      status,
    });

    let (version, status) = cmd!("docker", "version", "--format", "'{{.Client.Version}}'")
      .read()
      .map_or((NOT_FOUND.into(), false), |out| (out, true));

    clis.push(app::Cli {
      name: "docker".to_owned(),
      version: format!("v{}", version.replace("'", "")),
      status,
    });

    let (version, status) = cmd!("docker-compose", "version", "--short")
      .read()
      .map_or((NOT_FOUND.into(), false), |out| (out, true));

    clis.push(app::Cli {
      name: "docker-compose".into(),
      version: format!("v{}", version.replace("'", "")),
      status,
    });

    let (version, status) =
      get_info_by_regex("kind", &["version"], r"(v[0-9.]+)", NOT_FOUND.into());

    clis.push(app::Cli {
      name: "kind".to_owned(),
      version,
      status,
    });

    let (version, status) =
      get_info_by_regex("helm", &["version", "-c"], r"(v[0-9.]+)", NOT_FOUND.into());

    clis.push(app::Cli {
      name: "helm".to_owned(),
      version,
      status,
    });

    let (version, status) =
      get_info_by_regex("istioctl", &["version"], r"([0-9.]+)", NOT_FOUND.into());

    clis.push(app::Cli {
      name: "istioctl".to_owned(),
      version: format!("v{}", version),
      status,
    });

    let mut app = self.app.lock().await;
    app.data.clis = clis;
  }

  // TODO temp solution, should build this from API response
  async fn get_describe(&self, kind: String, value: String, ns: Option<String>) {
    let mut args = vec!["describe", kind.as_str(), value.as_str()];

    if let Some(ns) = ns.as_ref() {
      args.push("-n");
      args.push(ns.as_str());
    }

    let out = duct::cmd("kubectl", &args).read();

    match out {
      Ok(out) => {
        let mut app = self.app.lock().await;
        app.data.describe_out = ScrollableTxt::with_string(out);
      }
      Err(e) => {
        self
          .handle_error(anyhow!(format!(
            "Error running {} describe. Make sure you have kubectl installed: {:?}",
            kind, e
          )))
          .await
      }
    }
  }
}

// utils

/// execute a command and get info from it using regex
fn get_info_by_regex(command: &str, args: &[&str], regex: &str, default: String) -> (String, bool) {
  match cmd(command, args).read() {
    Ok(out) => match Regex::new(regex) {
      Ok(re) => match re.captures(out.as_str()) {
        Some(cap) => match cap.get(1) {
          Some(text) => (text.as_str().into(), true),
          _ => (default, false),
        },
        _ => (default, false),
      },
      _ => (default, false),
    },
    _ => (default, false),
  }
}
