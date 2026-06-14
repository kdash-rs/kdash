use std::process::{Command, ExitStatus, Stdio};

use super::is_valid_kubectl_arg;

/// The resource to open in `$EDITOR` via `kubectl edit`. `namespace` is `None`
/// for cluster-scoped kinds (nodes, PVs, cluster roles, …).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditTarget {
  pub namespace: Option<String>,
  pub kind: String,
  pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditCommand {
  pub program: String,
  pub args: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::enum_variant_names)] // each variant names the offending field
pub enum EditPrepareError {
  InvalidNamespace,
  InvalidKind,
  InvalidName,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditRunError {
  Spawn(String),
  Wait(String),
  Exit(String),
}

impl std::fmt::Display for EditPrepareError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::InvalidNamespace => write!(f, "Invalid namespace for edit"),
      Self::InvalidKind => write!(f, "Invalid resource kind for edit"),
      Self::InvalidName => write!(f, "Invalid resource name for edit"),
    }
  }
}

impl std::fmt::Display for EditRunError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Spawn(message) => write!(f, "Unable to start kubectl edit: {message}"),
      Self::Wait(message) => write!(f, "Unable to wait for kubectl edit: {message}"),
      Self::Exit(message) => write!(f, "kubectl edit exited unsuccessfully: {message}"),
    }
  }
}

/// Validate the target and build the `kubectl edit <kind> <name> [-n <ns>]`
/// command. kubectl drops the user into `$EDITOR` and applies the saved changes.
pub fn prepare_edit(target: &EditTarget) -> Result<EditCommand, EditPrepareError> {
  validate_target(target)?;
  Ok(build_edit_command(target))
}

pub fn run_edit(command: &EditCommand) -> Result<(), EditRunError> {
  let mut child = Command::new(&command.program);
  child
    .args(&command.args)
    .stdin(Stdio::inherit())
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit());

  let mut child = child
    .spawn()
    .map_err(|error| EditRunError::Spawn(error.to_string()))?;
  let status = child
    .wait()
    .map_err(|error| EditRunError::Wait(error.to_string()))?;

  if status.success() {
    Ok(())
  } else {
    Err(EditRunError::Exit(format_exit_status(status)))
  }
}

fn validate_target(target: &EditTarget) -> Result<(), EditPrepareError> {
  validate_component(&target.kind, EditPrepareError::InvalidKind)?;
  validate_component(&target.name, EditPrepareError::InvalidName)?;
  if let Some(namespace) = target.namespace.as_ref() {
    validate_component(namespace, EditPrepareError::InvalidNamespace)?;
  }
  Ok(())
}

fn validate_component(value: &str, error: EditPrepareError) -> Result<(), EditPrepareError> {
  if value.trim().is_empty() || !is_valid_kubectl_arg(value) {
    Err(error)
  } else {
    Ok(())
  }
}

fn build_edit_command(target: &EditTarget) -> EditCommand {
  let mut args = vec!["edit".into(), target.kind.clone(), target.name.clone()];
  if let Some(namespace) = target.namespace.as_ref() {
    args.push("-n".into());
    args.push(namespace.clone());
  }
  EditCommand {
    program: "kubectl".into(),
    args,
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

  #[test]
  fn test_prepare_edit_builds_namespaced_command() {
    let command = prepare_edit(&EditTarget {
      namespace: Some("default".into()),
      kind: "deployment".into(),
      name: "web".into(),
    })
    .expect("edit command should prepare");

    assert_eq!(command.program, "kubectl");
    assert_eq!(
      command.args,
      vec!["edit", "deployment", "web", "-n", "default"]
    );
  }

  #[test]
  fn test_prepare_edit_omits_namespace_for_cluster_scoped() {
    let command = prepare_edit(&EditTarget {
      namespace: None,
      kind: "node".into(),
      name: "node-1".into(),
    })
    .expect("edit command should prepare");

    assert_eq!(command.args, vec!["edit", "node", "node-1"]);
  }

  #[test]
  fn test_prepare_edit_rejects_invalid_values() {
    assert_eq!(
      prepare_edit(&EditTarget {
        namespace: Some("default; rm -rf /".into()),
        kind: "deployment".into(),
        name: "web".into(),
      }),
      Err(EditPrepareError::InvalidNamespace)
    );

    assert_eq!(
      prepare_edit(&EditTarget {
        namespace: Some("default".into()),
        kind: "deploy`whoami`".into(),
        name: "web".into(),
      }),
      Err(EditPrepareError::InvalidKind)
    );

    assert_eq!(
      prepare_edit(&EditTarget {
        namespace: Some("default".into()),
        kind: "deployment".into(),
        name: String::new(),
      }),
      Err(EditPrepareError::InvalidName)
    );
  }
}
