pub mod shell;

use std::sync::Arc;

use anyhow::anyhow;
use duct::cmd;
use log::{error, info};
use regex::Regex;
use serde_json::Value as JValue;
use tokio::sync::Mutex;

use crate::app::{self, models::ScrollableTxt, App, Cli};

#[derive(Clone, Debug, Eq, PartialEq)]
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

pub(crate) fn is_valid_kubectl_arg(s: &str) -> bool {
  !s.contains('\n')
    && !s.contains('\r')
    && !s.contains('\0')
    && !s.contains(';')
    && !s.contains('|')
    && !s.contains('&')
    && !s.contains('`')
    && !s.contains('$')
}

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
    app.loading_complete();
  }

  async fn handle_error(&self, e: anyhow::Error) {
    error!("{:?}", e);
    let mut app = self.app.lock().await;
    app.handle_error(e);
  }

  async fn get_cli_info(&self) {
    let clis = tokio::task::spawn_blocking(|| {
      let mut clis: Vec<Cli> = vec![];

      let (version_c, version_s) = match cmd!("kubectl", "version", "-o", "json")
        .stderr_null()
        .read()
      {
        Ok(out) => {
          info!("kubectl version: {}", out);
          let v: serde_json::Result<JValue> = serde_json::from_str(&out);
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
        .stderr_null()
        .read()
        .map_or(None, |out| {
          if out.is_empty() {
            None
          } else {
            Some(format!("v{}", out.replace('\'', "")))
          }
        });

      clis.push(build_cli("docker", version));

      let version = cmd!("docker-compose", "version", "--short")
        .stderr_null()
        .read()
        .map_or(None, |out| {
          if out.is_empty() {
            cmd!("docker", "compose", "version", "--short")
              .stderr_null()
              .read()
              .map_or(None, |out| {
                if out.is_empty() {
                  None
                } else {
                  Some(format!("v{}", out.replace('\'', "")))
                }
              })
          } else {
            Some(format!("v{}", out.replace('\'', "")))
          }
        });

      clis.push(build_cli("docker-compose", version));

      let version = get_info_by_regex("kind", &["version"], r"(v[0-9.]+)");

      clis.push(build_cli("kind", version));

      let version = get_info_by_regex("helm", &["version", "-c"], r"(v[0-9.]+)");

      clis.push(build_cli("helm", version));

      let version = get_info_by_regex("istioctl", &["version"], r"([0-9.]+)");

      clis.push(build_cli("istioctl", version.map(|v| format!("v{}", v))));

      clis
    })
    .await
    .unwrap_or_default();

    let mut app = self.app.lock().await;
    app.data.clis = clis;
  }

  // TODO temp solution, should build this from API response
  async fn get_describe(&self, kind: String, value: String, ns: Option<String>) {
    if !is_valid_kubectl_arg(&kind) || !is_valid_kubectl_arg(&value) {
      self
        .handle_error(anyhow!("Invalid characters in resource kind or name"))
        .await;
      return;
    }
    if let Some(ref ns) = ns {
      if !is_valid_kubectl_arg(ns) {
        self
          .handle_error(anyhow!("Invalid characters in namespace"))
          .await;
        return;
      }
    }

    let kind_clone = kind.clone();
    let result = tokio::task::spawn_blocking(move || {
      let mut args = vec!["describe", kind.as_str(), value.as_str()];

      if let Some(ns) = ns.as_ref() {
        args.push("-n");
        args.push(ns.as_str());
      }

      duct::cmd("kubectl", &args).stderr_null().read()
    })
    .await;

    match result {
      Ok(Ok(out)) => {
        let mut app = self.app.lock().await;
        app.data.describe_out = ScrollableTxt::with_string(out);
      }
      Ok(Err(e)) => {
        self
          .handle_error(anyhow!(
            "Error running {} describe. Make sure you have kubectl installed: {:?}",
            kind_clone,
            e
          ))
          .await
      }
      Err(e) => {
        self
          .handle_error(anyhow!("Describe task panicked: {:?}", e))
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
  match cmd(command, args).stderr_null().read() {
    Ok(out) => match Regex::new(regex) {
      Ok(re) => match re.captures(out.as_str()) {
        Some(cap) => cap.get(1).map(|text| text.as_str().into()),
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

  #[test]
  fn test_is_valid_arg_accepts_normal_input() {
    // Normal k8s resource names should pass
    assert!(super::is_valid_kubectl_arg("pod"));
    assert!(super::is_valid_kubectl_arg("my-deployment"));
    assert!(super::is_valid_kubectl_arg("my_namespace"));
    assert!(super::is_valid_kubectl_arg("kube-system"));
    assert!(super::is_valid_kubectl_arg(
      "nginx-ingress-controller-abc123"
    ));
    assert!(super::is_valid_kubectl_arg("default"));
    assert!(super::is_valid_kubectl_arg("my.resource.name"));
  }

  #[test]
  fn test_is_valid_arg_rejects_injection_attempts() {
    // Shell injection attempts should be rejected
    assert!(!super::is_valid_kubectl_arg("pod; rm -rf /"));
    assert!(!super::is_valid_kubectl_arg("pod | cat /etc/passwd"));
    assert!(!super::is_valid_kubectl_arg("pod & malicious-cmd"));
    assert!(!super::is_valid_kubectl_arg("pod `whoami`"));
    assert!(!super::is_valid_kubectl_arg("pod\nmalicious"));
    assert!(!super::is_valid_kubectl_arg("pod\rmalicious"));
    assert!(!super::is_valid_kubectl_arg("pod\0malicious"));
    assert!(!super::is_valid_kubectl_arg("$HOME"));
    assert!(!super::is_valid_kubectl_arg("$(whoami)"));
  }

  #[test]
  fn test_is_valid_arg_empty_string() {
    // Empty string should be considered valid (kubectl will error separately)
    assert!(super::is_valid_kubectl_arg(""));
  }
}
