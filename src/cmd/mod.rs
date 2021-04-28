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

    let (version_c, version_s) = match cmd!("kubectl", "version", "-o", "json").read() {
      Ok(out) => {
        let v: serde_json::Result<JValue> = serde_json::from_str(&*out);
        match v {
          Ok(val) => (
            Some(
              val["clientVersion"]["gitVersion"]
                .to_string()
                .replace('"', ""),
            ),
            Some(
              val["serverVersion"]["gitVersion"]
                .to_string()
                .replace('"', ""),
            ),
          ),
          _ => (None, None),
        }
      }
      _ => (None, None),
    };

    clis.push(build_cli("kubectl client", version_c));
    clis.push(build_cli("kubectl server", version_s));

    let version = cmd!("docker", "version", "--format", "'{{.Client.Version}}'")
      .read()
      .map_or(None, |out| Some(format!("v{}", out.replace("'", ""))));

    clis.push(build_cli("docker", version));

    let version = cmd!("docker-compose", "version", "--short")
      .read()
      .map_or(None, |out| Some(format!("v{}", out.replace("'", ""))));

    clis.push(build_cli("docker-compose", version));

    let version = get_info_by_regex("kind", &["version"], r"(v[0-9.]+)");

    clis.push(build_cli("kind", version));

    let version = get_info_by_regex("helm", &["version", "-c"], r"(v[0-9.]+)");

    clis.push(build_cli("helm", version));

    let version = get_info_by_regex("istioctl", &["version"], r"([0-9.]+)");

    clis.push(build_cli("istioctl", version.map(|v| format!("v{}", v))));

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

fn build_cli(name: &str, version: Option<String>) -> app::Cli {
  app::Cli {
    name: name.to_owned(),
    status: version.is_some(),
    version: version.unwrap_or_else(|| NOT_FOUND.into()),
  }
}

/// execute a command and get info from it using regex
fn get_info_by_regex(command: &str, args: &[&str], regex: &str) -> Option<String> {
  match cmd(command, args).read() {
    Ok(out) => match Regex::new(regex) {
      Ok(re) => match re.captures(out.as_str()) {
        Some(cap) => match cap.get(1) {
          Some(text) => Some(text.as_str().into()),
          _ => None,
        },
        _ => None,
      },
      _ => None,
    },
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn test_get_info_by_regex() {
    use super::get_info_by_regex;
    assert_eq!(
      get_info_by_regex(
        "echo",
        &["Client: &version.Version{SemVer:\"v2.17.0\", GitCommit:\"a690bad98af45b015bd3da1a41f6218b1a451dbe\", GitTreeState:\"clean\"} \n Error: could not find tiller"],
        r"(v[0-9.]+)"
      ),
      Some("v2.17.0".into())
    );
    assert_eq!(
      get_info_by_regex(
        "echo",
        &["no running Istio pods in \"istio-system\"\n1.8.2"],
        r"([0-9.]+)"
      ),
      Some("1.8.2".into())
    );
  }
}
