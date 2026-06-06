pub mod shell;

use std::{collections::BTreeSet, io, sync::Arc};

use anyhow::anyhow;
use duct::cmd;
use log::{error, info};
use regex::Regex;
use serde_json::Value as JValue;
use tokio::sync::Mutex;

use crate::{
  app::{self, models::ScrollableTxt, App, Cli},
  config::{CliInfoConfig, CliInfoEntry},
};

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

#[derive(Debug, Eq, PartialEq)]
enum CliProbe {
  MissingBinary,
  Version(Option<String>),
}

impl CliProbe {
  fn map(self, f: impl FnOnce(Option<String>) -> Option<String>) -> Self {
    match self {
      CliProbe::MissingBinary => CliProbe::MissingBinary,
      CliProbe::Version(version) => CliProbe::Version(f(version)),
    }
  }
}

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
    let cli_info_config = {
      let app = self.app.lock().await;
      app.config.cli_info.clone().unwrap_or_default()
    };

    let clis = tokio::task::spawn_blocking(move || {
      let mut clis: Vec<Cli> = vec![];
      let disabled_defaults = disabled_default_set(&cli_info_config);
      let hide_missing_binaries = cli_info_config.hide_missing_binaries;

      if !disabled_defaults.contains("kubectl client")
        || !disabled_defaults.contains("kubectl server")
      {
        let kubectl_probe = match cmd!("kubectl", "version", "-o", "json")
          .stderr_null()
          .read()
        {
          Ok(out) => {
            info!("kubectl version: {}", out);
            let v: serde_json::Result<JValue> = serde_json::from_str(&out);
            match v {
              Ok(val) => CliProbe::Version(Some(format!(
                "{}|{}",
                val["clientVersion"]["gitVersion"]
                  .to_string()
                  .replace('"', ""),
                val["serverVersion"]["gitVersion"]
                  .to_string()
                  .replace('"', ""),
              ))),
              _ => CliProbe::Version(None),
            }
          }
          Err(error) if error.kind() == io::ErrorKind::NotFound => CliProbe::MissingBinary,
          Err(_) => CliProbe::Version(None),
        };

        let versions = match kubectl_probe {
          CliProbe::Version(versions) => versions
            .as_deref()
            .and_then(|joined| joined.split_once('|'))
            .map(|(client, server)| (Some(client.to_string()), Some(server.to_string()))),
          CliProbe::MissingBinary if hide_missing_binaries => None,
          CliProbe::MissingBinary => Some((None, None)),
        };

        if let Some((version_c, version_s)) = versions {
          if !disabled_defaults.contains("kubectl client") {
            clis.push(build_cli("kubectl client", version_c));
          }
          if !disabled_defaults.contains("kubectl server") {
            clis.push(build_cli("kubectl server", version_s));
          }
        }
      }

      if !disabled_defaults.contains("docker") {
        if let Some(cli) = build_cli_for_probe(
          "docker",
          run_cli_entries(&[cli_entry(
            &["docker", "version", "--format", "v{{.Client.Version}}"],
            Some(r"\b(v[0-9]+\.[0-9]+\.[0-9]+)\b"),
          )]),
          hide_missing_binaries,
        ) {
          clis.push(cli);
        }
      }

      if !disabled_defaults.contains("docker-compose") {
        if let Some(cli) = build_cli_for_probe(
          "docker-compose",
          run_cli_entries(&[
            cli_entry(
              &["docker-compose", "version"],
              Some(r"\b(v?[0-9]+\.[0-9]+\.[0-9]+)\b"),
            ),
            cli_entry(
              &["docker", "compose", "version"],
              Some(r"\b(v?[0-9]+\.[0-9]+\.[0-9]+)\b"),
            ),
          ])
          .map(|version| version.map(normalize_version_prefix)),
          hide_missing_binaries,
        ) {
          clis.push(cli);
        }
      }

      if !disabled_defaults.contains("podman") {
        if let Some(cli) = build_cli_for_probe(
          "podman",
          run_cli_entries(&[cli_entry(
            &["podman", "version", "--format", "v{{.Client.Version}}"],
            Some(r"\b(v[0-9]+\.[0-9]+\.[0-9]+)\b"),
          )]),
          hide_missing_binaries,
        ) {
          clis.push(cli);
        }
      }

      if !disabled_defaults.contains("containerd") {
        if let Some(cli) = build_cli_for_probe(
          "containerd",
          run_cli_entries(&[cli_entry(
            &["containerd", "--version"],
            Some(r"\b(v[0-9]+\.[0-9]+\.[0-9]+)\b"),
          )]),
          hide_missing_binaries,
        ) {
          clis.push(cli);
        }
      }

      if !disabled_defaults.contains("helm") {
        if let Some(cli) = build_cli_for_probe(
          "helm",
          run_cli_entries(&[cli_entry(
            &["helm", "version", "-c"],
            Some(r"\b(v[0-9]+\.[0-9]+\.[0-9]+)\b"),
          )]),
          hide_missing_binaries,
        ) {
          clis.push(cli);
        }
      }

      if !disabled_defaults.contains("kind") {
        if let Some(cli) = build_cli_for_probe(
          "kind",
          run_cli_entries(&[cli_entry(
            &["kind", "version"],
            Some(r"\b(v[0-9]+\.[0-9]+\.[0-9]+)\b"),
          )]),
          hide_missing_binaries,
        ) {
          clis.push(cli);
        }
      }

      for entry in &cli_info_config.custom {
        if entry.command.is_empty() {
          continue;
        }
        if let Some(cli) =
          build_cli_for_probe(&entry.label, run_cli_entry(entry), hide_missing_binaries)
        {
          clis.push(cli);
        }
      }

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

fn build_cli_for_probe(
  name: &str,
  probe: CliProbe,
  hide_missing_binaries: bool,
) -> Option<app::Cli> {
  match probe {
    CliProbe::MissingBinary if hide_missing_binaries => None,
    CliProbe::MissingBinary => Some(build_cli(name, None)),
    CliProbe::Version(version) => Some(build_cli(name, version)),
  }
}

fn disabled_default_set(cli_info: &CliInfoConfig) -> BTreeSet<String> {
  cli_info
    .disable_defaults
    .iter()
    .map(|label| label.trim().to_lowercase())
    .collect()
}

fn cli_entry(command: &[&str], regex: Option<&str>) -> CliInfoEntry {
  CliInfoEntry {
    label: String::new(),
    command: command.iter().map(|value| (*value).to_string()).collect(),
    regex: regex.map(str::to_string),
  }
}

fn run_cli_entries(entries: &[CliInfoEntry]) -> CliProbe {
  let mut saw_probeable_command = false;

  for entry in entries {
    match run_cli_entry(entry) {
      CliProbe::Version(Some(version)) => return CliProbe::Version(Some(version)),
      CliProbe::Version(None) => saw_probeable_command = true,
      CliProbe::MissingBinary => {}
    }
  }

  if saw_probeable_command {
    CliProbe::Version(None)
  } else {
    CliProbe::MissingBinary
  }
}

fn run_cli_entry(entry: &CliInfoEntry) -> CliProbe {
  let Some((program, args)) = entry.command.split_first() else {
    return CliProbe::Version(None);
  };
  let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
  read_command(program, &arg_refs).map(|output| {
    output.and_then(|value| {
      if let Some(regex) = entry.regex.as_deref() {
        return Regex::new(regex).ok().and_then(|re| {
          re.captures(&value)
            .and_then(|cap| cap.get(1))
            .map(|matched| matched.as_str().trim().to_string())
        });
      }

      value
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .map(str::to_string)
    })
  })
}

fn normalize_version_prefix(version: String) -> String {
  if version.starts_with('v') {
    version
  } else {
    format!("v{}", version)
  }
}

fn read_command(command: &str, args: &[&str]) -> CliProbe {
  match cmd(command, args).stderr_null().read() {
    Ok(out) => CliProbe::Version(Some(out)),
    Err(error) if error.kind() == io::ErrorKind::NotFound => CliProbe::MissingBinary,
    Err(_) => CliProbe::Version(None),
  }
}

#[cfg(test)]
mod tests {
  use super::CliProbe;
  use crate::config::{CliInfoConfig, CliInfoEntry};

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

  #[test]
  fn test_disabled_default_set_normalizes_labels() {
    let set = super::disabled_default_set(&CliInfoConfig {
      hide_missing_binaries: true,
      disable_defaults: vec![" Docker ".into(), "kubectl client".into()],
      custom: vec![],
    });

    assert!(set.contains("docker"));
    assert!(set.contains("kubectl client"));
  }

  #[test]
  fn test_run_custom_cli_command_uses_first_non_empty_line() {
    let entry = CliInfoEntry {
      label: "rustc".into(),
      command: vec!["rustc".into(), "--version".into()],
      regex: None,
    };

    assert!(matches!(
      super::run_cli_entry(&entry),
      CliProbe::Version(Some(version)) if version.starts_with("rustc ")
    ));
  }

  #[test]
  fn test_run_custom_cli_command_returns_none_for_empty_command() {
    let entry = CliInfoEntry {
      label: "broken".into(),
      command: vec![],
      regex: None,
    };

    assert_eq!(super::run_cli_entry(&entry), CliProbe::Version(None));
  }

  #[test]
  fn test_run_custom_cli_command_uses_regex_when_configured() {
    let entry = CliInfoEntry {
      label: "echo".into(),
      command: vec!["echo".into(), "release=1.2.3".into()],
      regex: Some(r"release=([0-9.]+)".into()),
    };

    assert_eq!(
      super::run_cli_entry(&entry),
      CliProbe::Version(Some("1.2.3".into()))
    );
  }

  #[test]
  fn test_run_custom_cli_command_returns_none_when_regex_does_not_match() {
    let entry = CliInfoEntry {
      label: "echo".into(),
      command: vec!["echo".into(), "release=1.2.3".into()],
      regex: Some(r"version=([0-9.]+)".into()),
    };

    assert_eq!(super::run_cli_entry(&entry), CliProbe::Version(None));
  }

  #[test]
  fn test_run_cli_entries_tries_fallback_commands() {
    let entries = vec![
      CliInfoEntry {
        label: String::new(),
        command: vec!["definitely-not-installed-kdash".into()],
        regex: None,
      },
      CliInfoEntry {
        label: String::new(),
        command: vec!["echo".into(), "release=1.2.3".into()],
        regex: Some(r"release=([0-9.]+)".into()),
      },
    ];

    assert_eq!(
      super::run_cli_entries(&entries),
      CliProbe::Version(Some("1.2.3".into()))
    );
  }

  #[test]
  fn test_normalize_version_prefix_adds_missing_v() {
    assert_eq!(super::normalize_version_prefix("1.2.3".into()), "v1.2.3");
    assert_eq!(super::normalize_version_prefix("v1.2.3".into()), "v1.2.3");
  }

  #[test]
  fn test_build_cli_for_probe_hides_missing_binary_by_default() {
    assert!(super::build_cli_for_probe("docker", CliProbe::MissingBinary, true).is_none());
  }

  #[test]
  fn test_build_cli_for_probe_can_show_missing_binary() {
    let cli = super::build_cli_for_probe("docker", CliProbe::MissingBinary, false)
      .expect("missing binary should still render");

    assert_eq!(cli.name, "docker");
    assert_eq!(cli.version, "Not found");
    assert!(!cli.status);
  }
}
