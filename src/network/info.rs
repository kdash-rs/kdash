use super::{
  app::{self, KubeContext, CLI},
  Network, NOT_FOUND,
};
use anyhow::anyhow;
use duct::cmd;

use kube::config::Kubeconfig;
use regex::Regex;
use serde_json::Value as JValue;

impl<'a> Network<'a> {
  pub async fn get_cli_info(&mut self) {
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

  pub async fn get_kube_config(&mut self) {
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
