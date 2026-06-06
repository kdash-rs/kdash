use std::process::{Command, ExitStatus, Stdio};

use anyhow::anyhow;

use super::is_valid_kubectl_arg;

const SHELL_CANDIDATES: [&str; 2] = ["/bin/bash", "/bin/sh"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellExecTarget {
  pub namespace: String,
  pub pod: String,
  pub container: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellExecCommand {
  pub program: String,
  pub args: Vec<String>,
  pub shell: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShellProbeResult {
  Supported,
  Unsupported,
  Failed(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShellExecPrepareError {
  InvalidNamespace,
  InvalidPod,
  InvalidContainer,
  UnsupportedShell,
  ProbeFailed(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShellExecRunError {
  Spawn(String),
  Wait(String),
  Exit(String),
}

impl std::fmt::Display for ShellExecPrepareError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::InvalidNamespace => write!(f, "Invalid namespace for shell exec"),
      Self::InvalidPod => write!(f, "Invalid pod name for shell exec"),
      Self::InvalidContainer => write!(f, "Invalid container name for shell exec"),
      Self::UnsupportedShell => write!(f, "Unable to find a supported shell in the container"),
      Self::ProbeFailed(message) => write!(f, "Unable to probe container shell support: {message}"),
    }
  }
}

impl std::fmt::Display for ShellExecRunError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Spawn(message) => write!(f, "Unable to start kubectl exec: {message}"),
      Self::Wait(message) => write!(f, "Unable to wait for kubectl exec: {message}"),
      Self::Exit(message) => write!(f, "kubectl exec exited unsuccessfully: {message}"),
    }
  }
}

pub fn prepare_shell_exec(
  target: &ShellExecTarget,
) -> Result<ShellExecCommand, ShellExecPrepareError> {
  prepare_shell_exec_with_probe(target, probe_shell_support)
}

pub fn prepare_shell_exec_with_probe<F>(
  target: &ShellExecTarget,
  mut probe: F,
) -> Result<ShellExecCommand, ShellExecPrepareError>
where
  F: FnMut(&ShellExecTarget, &str) -> ShellProbeResult,
{
  validate_target(target)?;

  for shell in SHELL_CANDIDATES {
    match probe(target, shell) {
      ShellProbeResult::Supported => return Ok(build_shell_exec_command(target, shell)),
      ShellProbeResult::Unsupported => continue,
      ShellProbeResult::Failed(message) => return Err(ShellExecPrepareError::ProbeFailed(message)),
    }
  }

  Err(ShellExecPrepareError::UnsupportedShell)
}

pub fn run_shell_exec(command: &ShellExecCommand) -> Result<(), ShellExecRunError> {
  let mut child = Command::new(&command.program);
  child
    .args(&command.args)
    .stdin(Stdio::inherit())
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit());

  let mut child = child
    .spawn()
    .map_err(|error| ShellExecRunError::Spawn(error.to_string()))?;
  let status = child
    .wait()
    .map_err(|error| ShellExecRunError::Wait(error.to_string()))?;

  if status.success() {
    Ok(())
  } else {
    Err(ShellExecRunError::Exit(format_exit_status(status)))
  }
}

fn validate_target(target: &ShellExecTarget) -> Result<(), ShellExecPrepareError> {
  validate_component(&target.namespace, ShellExecPrepareError::InvalidNamespace)?;
  validate_component(&target.pod, ShellExecPrepareError::InvalidPod)?;
  validate_component(&target.container, ShellExecPrepareError::InvalidContainer)?;
  Ok(())
}

fn validate_component(
  value: &str,
  error: ShellExecPrepareError,
) -> Result<(), ShellExecPrepareError> {
  if value.trim().is_empty() || !is_valid_kubectl_arg(value) {
    Err(error)
  } else {
    Ok(())
  }
}

fn build_shell_exec_command(target: &ShellExecTarget, shell: &str) -> ShellExecCommand {
  ShellExecCommand {
    program: "kubectl".into(),
    args: vec![
      "exec".into(),
      "-it".into(),
      "-n".into(),
      target.namespace.clone(),
      target.pod.clone(),
      "-c".into(),
      target.container.clone(),
      "--".into(),
      shell.into(),
    ],
    shell: shell.into(),
  }
}

fn probe_shell_support(target: &ShellExecTarget, shell: &str) -> ShellProbeResult {
  let output = Command::new("kubectl")
    .args([
      "exec",
      "-n",
      target.namespace.as_str(),
      target.pod.as_str(),
      "-c",
      target.container.as_str(),
      "--",
      shell,
      "-c",
      "exit",
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .output();

  match output {
    Ok(output) if output.status.success() => ShellProbeResult::Supported,
    Ok(output) => {
      let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
      if is_missing_shell_error(&stderr, shell) {
        ShellProbeResult::Unsupported
      } else {
        ShellProbeResult::Failed(format_probe_error(output.status, &stderr))
      }
    }
    Err(error) => ShellProbeResult::Failed(anyhow!(error).to_string()),
  }
}

fn is_missing_shell_error(stderr: &str, shell: &str) -> bool {
  let stderr = stderr.to_ascii_lowercase();
  let shell = shell.to_ascii_lowercase();

  stderr.contains(&shell)
    && (stderr.contains("not found")
      || stderr.contains("no such file")
      || stderr.contains("executable file"))
}

fn format_probe_error(status: ExitStatus, stderr: &str) -> String {
  if stderr.is_empty() {
    format!("kubectl exec probe exited with status {status}")
  } else {
    format!("kubectl exec probe exited with status {status}: {stderr}")
  }
}

fn format_exit_status(status: ExitStatus) -> String {
  status
    .code()
    .map_or_else(|| status.to_string(), |code| code.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn target() -> ShellExecTarget {
    ShellExecTarget {
      namespace: "default".into(),
      pod: "api-123".into(),
      container: "web".into(),
    }
  }

  #[test]
  fn test_prepare_shell_exec_builds_interactive_kubectl_command() {
    let command = prepare_shell_exec_with_probe(&target(), |_, shell| {
      assert_eq!(shell, "/bin/bash");
      ShellProbeResult::Supported
    })
    .expect("shell command should prepare");

    assert_eq!(command.program, "kubectl");
    assert_eq!(
      command.args,
      vec![
        "exec",
        "-it",
        "-n",
        "default",
        "api-123",
        "-c",
        "web",
        "--",
        "/bin/bash",
      ]
    );
    assert_eq!(command.shell, "/bin/bash");
  }

  #[test]
  fn test_prepare_shell_exec_rejects_invalid_target_values() {
    let mut invalid = target();
    invalid.namespace = "default; rm -rf /".into();

    assert_eq!(
      prepare_shell_exec_with_probe(&invalid, |_, _| ShellProbeResult::Supported),
      Err(ShellExecPrepareError::InvalidNamespace)
    );

    let mut invalid = target();
    invalid.pod = String::new();
    assert_eq!(
      prepare_shell_exec_with_probe(&invalid, |_, _| ShellProbeResult::Supported),
      Err(ShellExecPrepareError::InvalidPod)
    );

    let mut invalid = target();
    invalid.container = "web\nmalicious".into();
    assert_eq!(
      prepare_shell_exec_with_probe(&invalid, |_, _| ShellProbeResult::Supported),
      Err(ShellExecPrepareError::InvalidContainer)
    );
  }

  #[test]
  fn test_prepare_shell_exec_falls_back_to_sh_when_bash_missing() {
    let mut probed = vec![];
    let command = prepare_shell_exec_with_probe(&target(), |_, shell| {
      probed.push(shell.to_string());
      if shell == "/bin/bash" {
        ShellProbeResult::Unsupported
      } else {
        ShellProbeResult::Supported
      }
    })
    .expect("shell command should fall back");

    assert_eq!(probed, vec!["/bin/bash", "/bin/sh"]);
    assert_eq!(command.shell, "/bin/sh");
    assert_eq!(
      command.args,
      vec!["exec", "-it", "-n", "default", "api-123", "-c", "web", "--", "/bin/sh",]
    );
  }

  #[test]
  fn test_prepare_shell_exec_returns_unsupported_when_no_shell_exists() {
    assert_eq!(
      prepare_shell_exec_with_probe(&target(), |_, _| ShellProbeResult::Unsupported),
      Err(ShellExecPrepareError::UnsupportedShell)
    );
  }

  #[test]
  fn test_prepare_shell_exec_returns_probe_failure() {
    assert_eq!(
      prepare_shell_exec_with_probe(&target(), |_, _| {
        ShellProbeResult::Failed("kubectl unavailable".into())
      }),
      Err(ShellExecPrepareError::ProbeFailed(
        "kubectl unavailable".into()
      ))
    );
  }

  #[test]
  fn test_is_missing_shell_error_matches_shell_specific_missing_binary_message() {
    assert!(is_missing_shell_error(
      "exec: \"/bin/bash\": stat /bin/bash: no such file or directory",
      "/bin/bash"
    ));
    assert!(!is_missing_shell_error(
      "Error from server (NotFound): pods \"api-123\" not found",
      "/bin/bash"
    ));
  }
}
