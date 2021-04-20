use super::{
  app::{self, Cli},
  Network, NOT_FOUND,
};

use duct::cmd;
use regex::Regex;
use serde_json::Value as JValue;

impl<'a> Network<'a> {
  pub async fn get_cli_info(&mut self) {
    let mut clis: Vec<Cli> = vec![];

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

    clis.push(app::Cli {
      name: "kubectl".to_string(),
      version: version.replace('"', ""),
      status,
    });

    let (version, status) = cmd!("docker", "version", "--format", "'{{.Client.Version}}'")
      .read()
      .map_or((NOT_FOUND.to_string(), false), |out| (out, true));

    clis.push(app::Cli {
      name: "docker".to_string(),
      version: format!("v{}", version.replace("'", "")),
      status,
    });

    let (version, status) = cmd!("docker-compose", "version", "--short")
      .read()
      .map_or((NOT_FOUND.to_string(), false), |out| (out, true));

    clis.push(app::Cli {
      name: "docker-compose".to_string(),
      version: format!("v{}", version.replace("'", "")),
      status,
    });

    let (version, status) =
      get_info_by_regex("kind", &["version"], r"(v[0-9.]+)", NOT_FOUND.to_string());

    clis.push(app::Cli {
      name: "kind".to_string(),
      version,
      status,
    });

    let (version, status) = get_info_by_regex(
      "helm",
      &["version", "-c"],
      r"(v[0-9.]+)",
      NOT_FOUND.to_string(),
    );

    clis.push(app::Cli {
      name: "helm".to_string(),
      version,
      status,
    });

    let (version, status) = get_info_by_regex(
      "istioctl",
      &["version"],
      r"([0-9.]+)",
      NOT_FOUND.to_string(),
    );

    clis.push(app::Cli {
      name: "istioctl".to_string(),
      version: format!("v{}", version),
      status,
    });

    let mut app = self.app.lock().await;
    app.data.clis = clis;
  }
}

// utils

/// execute a command and get info from it using regex
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
