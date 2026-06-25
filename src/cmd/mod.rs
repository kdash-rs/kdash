pub mod edit;
pub mod port_forward;
pub mod shell;

use std::{collections::BTreeSet, io, process::Stdio, sync::Arc};

use anyhow::anyhow;
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

/// Append `--context <name>` when an in-app context is selected, so kubectl
/// targets the same cluster the `Shift+C` context switch points at rather than
/// the kubeconfig's `current-context` (#532). No-op when no context is selected,
/// matching the kube-rs client which then infers from the kubeconfig.
pub(crate) fn push_context_arg(args: &mut Vec<String>, context: Option<&str>) {
  if let Some(context) = context {
    args.push("--context".into());
    args.push(context.into());
  }
}

const VERSION_REGEX: &str = r"\b(v[0-9]+\.[0-9]+\.[0-9]+)\b";
const REGEX_PROBES: [(&str, &[&str], &str); 7] = [
  (
    "docker",
    &["docker", "version", "--format", "v{{.Client.Version}}"],
    VERSION_REGEX,
  ),
  (
    "docker-compose",
    &["docker-compose", "version"],
    r"\b(v?[0-9]+\.[0-9]+\.[0-9]+)\b",
  ),
  (
    "docker compose",
    &["docker", "compose", "version"],
    VERSION_REGEX,
  ),
  (
    "podman",
    &["podman", "version", "--format", "v{{.Client.Version}}"],
    VERSION_REGEX,
  ),
  ("containerd", &["containerd", "--version"], VERSION_REGEX),
  ("helm", &["helm", "version", "--short"], VERSION_REGEX),
  ("kind", &["kind", "version"], VERSION_REGEX),
];

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

    let mut clis: Vec<Cli> = vec![];
    {
      let disabled_defaults = disabled_default_set(&cli_info_config);
      let hide_missing_binaries = cli_info_config.hide_missing_binaries;

      if !disabled_defaults.contains("kubectl client")
        || !disabled_defaults.contains("kubectl server")
      {
        let kubectl_probe = match run_cmd("kubectl", &["version", "-o", "json"]).await {
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

      let mut join_set = tokio::task::JoinSet::new();
      for (label, command, regex) in REGEX_PROBES
        .iter()
        .filter(|(label, _, _)| !disabled_defaults.contains(*label))
      {
        let info_entry = CliInfoEntry {
          label: label.to_string(),
          command: command.iter().map(|s| s.to_string()).collect(),
          regex: Some(Regex::new(regex).unwrap()),
        };
        //let hide_missing_binaries = hide_missing_binaries;
        join_set.spawn(async move {
          build_cli_for_probe(
            &info_entry.label,
            run_cli_entry(&info_entry).await,
            hide_missing_binaries,
          )
        });
      }
      for entry in &cli_info_config.custom {
        if entry.command.is_empty() {
          continue;
        }
        let entry = entry.clone();
        join_set.spawn(async move {
          build_cli_for_probe(
            &entry.label,
            run_cli_entry(&entry).await,
            hide_missing_binaries,
          )
        });
      }
      for cli in join_set.join_all().await.into_iter().flatten() {
        clis.push(cli);
      }
    }

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

    let context = {
      let app = self.app.lock().await;
      app.data.selected.context.clone()
    };
    if let Some(ref context) = context {
      if !is_valid_kubectl_arg(context) {
        self
          .handle_error(anyhow!("Invalid characters in context"))
          .await;
        return;
      }
    }

    let kind_clone = kind.clone();

    let mut args = vec!["describe".to_string(), kind, value];

    if let Some(ns) = ns {
      args.push("-n".to_string());
      args.push(ns);
    }
    push_context_arg(&mut args, context.as_deref());

    let result = tokio::process::Command::new("kubectl")
      .args(&args)
      .kill_on_drop(true)
      .stderr(Stdio::null())
      .output()
      .await
      .map(|output| String::from_utf8_lossy(&output.stdout).to_string());

    match result {
      Ok(out) => {
        let mut app = self.app.lock().await;
        app.data.describe_out = ScrollableTxt::with_string(out);
      }
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
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

async fn run_cli_entry(entry: &CliInfoEntry) -> CliProbe {
  let Some((program, args)) = entry.command.split_first() else {
    return CliProbe::Version(None);
  };
  let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
  run_cmd(program, &arg_refs)
    .await
    .map(|value| {
      if let Some(ref re) = entry.regex {
        return re
          .captures(&value)
          .and_then(|cap| cap.get(1))
          .map(|matched| matched.as_str().trim().to_string());
      }

      value
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .map(str::to_string)
    })
    .map_or(CliProbe::MissingBinary, |version| {
      CliProbe::Version(version)
    })
}

async fn run_cmd(cmd: &str, args: &[&str]) -> Result<String, io::Error> {
  let output = tokio::process::Command::new(cmd)
    .args(args)
    .kill_on_drop(true)
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .spawn()?
    .wait_with_output()
    .await?
    .stdout;
  Ok(String::from_utf8_lossy(&output).to_string())
}

#[cfg(test)]
mod tests {
  use regex::Regex;

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

  fn sync_probe(entry: &CliInfoEntry) -> CliProbe {
    tokio::runtime::Builder::new_current_thread()
      .enable_io()
      .build()
      .unwrap()
      .block_on(super::run_cli_entry(entry))
  }

  #[test]
  fn test_run_custom_cli_command_uses_first_non_empty_line() {
    let entry = CliInfoEntry {
      label: "rustc".into(),
      command: vec!["rustc".into(), "--version".into()],
      regex: None,
    };

    assert!(matches!(
      sync_probe(&entry),
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

    assert_eq!(sync_probe(&entry), CliProbe::Version(None));
  }

  #[test]
  fn test_run_custom_cli_command_uses_regex_when_configured() {
    let entry = CliInfoEntry {
      label: "echo".into(),
      command: vec!["echo".into(), "release=1.2.3".into()],
      regex: Some(Regex::new(r"release=([0-9.]+)").unwrap()),
    };

    assert_eq!(sync_probe(&entry), CliProbe::Version(Some("1.2.3".into())));
  }

  #[test]
  fn test_run_custom_cli_command_returns_none_when_regex_does_not_match() {
    let entry = CliInfoEntry {
      label: "echo".into(),
      command: vec!["echo".into(), "release=1.2.3".into()],
      regex: Some(Regex::new(r"version=([0-9.]+)").unwrap()),
    };

    assert_eq!(sync_probe(&entry), CliProbe::Version(None));
  }

  #[test]
  fn test_run_cli_entries_tries_fallback_commands() {
    let entries = [
      CliInfoEntry {
        label: String::new(),
        command: vec!["definitely-not-installed-kdash".into()],
        regex: None,
      },
      CliInfoEntry {
        label: String::new(),
        command: vec!["echo".into(), "release=1.2.3".into()],
        regex: Some(Regex::new(r"release=([0-9.]+)").unwrap()),
      },
    ];

    assert_eq!(
      sync_probe(&entries[1]),
      CliProbe::Version(Some("1.2.3".into()))
    );
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
