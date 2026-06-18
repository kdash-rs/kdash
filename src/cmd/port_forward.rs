use super::{is_valid_kubectl_arg, push_context_arg};

/// A pod or service to forward to a local port. `kind` is the kubectl resource
/// type (`pods` / `services`); ports are validated as `u16` so they never need
/// shell-escaping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortForwardTarget {
  pub kind: String,
  pub namespace: String,
  pub name: String,
  pub local_port: u16,
  pub remote_port: u16,
  /// In-app selected context, or `None` to use the kubeconfig default (#532).
  pub context: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortForwardCommand {
  pub program: String,
  pub args: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::enum_variant_names)] // each variant names the offending field
pub enum PortForwardPrepareError {
  InvalidKind,
  InvalidNamespace,
  InvalidName,
  InvalidContext,
}

impl std::fmt::Display for PortForwardPrepareError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::InvalidKind => write!(f, "Invalid resource kind for port-forward"),
      Self::InvalidNamespace => write!(f, "Invalid namespace for port-forward"),
      Self::InvalidName => write!(f, "Invalid resource name for port-forward"),
      Self::InvalidContext => write!(f, "Invalid context for port-forward"),
    }
  }
}

/// Validate the target and build the
/// `kubectl port-forward <kind>/<name> -n <ns> <local>:<remote>` command. The
/// child is run in the background (it stays open until killed), unlike the
/// foreground `kubectl edit` / `exec` commands.
pub fn prepare_port_forward(
  target: &PortForwardTarget,
) -> Result<PortForwardCommand, PortForwardPrepareError> {
  validate_component(&target.kind, PortForwardPrepareError::InvalidKind)?;
  validate_component(&target.namespace, PortForwardPrepareError::InvalidNamespace)?;
  validate_component(&target.name, PortForwardPrepareError::InvalidName)?;
  if let Some(context) = target.context.as_deref() {
    if !is_valid_kubectl_arg(context) {
      return Err(PortForwardPrepareError::InvalidContext);
    }
  }

  let mut args = vec![
    "port-forward".into(),
    format!("{}/{}", target.kind, target.name),
    "-n".into(),
    target.namespace.clone(),
    format!("{}:{}", target.local_port, target.remote_port),
  ];
  push_context_arg(&mut args, target.context.as_deref());

  Ok(PortForwardCommand {
    program: "kubectl".into(),
    args,
  })
}

fn validate_component(
  value: &str,
  error: PortForwardPrepareError,
) -> Result<(), PortForwardPrepareError> {
  if value.trim().is_empty() || !is_valid_kubectl_arg(value) {
    Err(error)
  } else {
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn target() -> PortForwardTarget {
    PortForwardTarget {
      kind: "pods".into(),
      namespace: "default".into(),
      name: "web".into(),
      local_port: 8080,
      remote_port: 80,
      context: None,
    }
  }

  #[test]
  fn test_prepare_port_forward_builds_command() {
    let command = prepare_port_forward(&target()).expect("command should prepare");

    assert_eq!(command.program, "kubectl");
    assert_eq!(
      command.args,
      vec!["port-forward", "pods/web", "-n", "default", "8080:80"]
    );
  }

  #[test]
  fn test_prepare_port_forward_forwards_services() {
    let mut svc = target();
    svc.kind = "services".into();
    svc.name = "api".into();
    let command = prepare_port_forward(&svc).expect("command should prepare");

    assert_eq!(
      command.args,
      vec!["port-forward", "services/api", "-n", "default", "8080:80"]
    );
  }

  #[test]
  fn test_prepare_port_forward_includes_selected_context() {
    let mut with_context = target();
    with_context.context = Some("prod".into());
    let command = prepare_port_forward(&with_context).expect("command should prepare");

    assert_eq!(
      command.args,
      vec![
        "port-forward",
        "pods/web",
        "-n",
        "default",
        "8080:80",
        "--context",
        "prod"
      ]
    );
  }

  #[test]
  fn test_prepare_port_forward_rejects_injection() {
    let mut invalid = target();
    invalid.name = "web; rm -rf /".into();
    assert_eq!(
      prepare_port_forward(&invalid),
      Err(PortForwardPrepareError::InvalidName)
    );

    let mut invalid = target();
    invalid.namespace = "ns`whoami`".into();
    assert_eq!(
      prepare_port_forward(&invalid),
      Err(PortForwardPrepareError::InvalidNamespace)
    );

    let mut invalid = target();
    invalid.kind = String::new();
    assert_eq!(
      prepare_port_forward(&invalid),
      Err(PortForwardPrepareError::InvalidKind)
    );

    let mut invalid = target();
    invalid.context = Some("prod;reboot".into());
    assert_eq!(
      prepare_port_forward(&invalid),
      Err(PortForwardPrepareError::InvalidContext)
    );
  }
}
