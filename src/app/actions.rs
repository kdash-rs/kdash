//! Central registry of resource actions and the overlay modal state that backs
//! the `m` action menu and the confirmation / input dialogs.
//!
//! `actions_for` is the single source of truth for "what can I do to the
//! selected item" so the menu, hotkey hints, and handlers can never drift.
use crate::app::key_binding::DEFAULT_KEYBINDING;
use crate::app::ActiveBlock;
use crate::event::Key;
use crate::network::{IoEvent, ResourcePatch};

/// An action that can be performed on the selected resource. Surfaced both as a
/// hotkey (with a hint in the hint area) and as an entry in the `m` action menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceAction {
  Describe,
  Yaml,
  Edit,
  Logs,
  Shell,
  PortForward,
  PreviousLogs,
  Restart,
  Scale,
  Cordon,
  Suspend,
  Trigger,
  DecodeSecret,
  Delete,
}

impl ResourceAction {
  /// Human-readable label shown in the action menu.
  pub fn label(self) -> &'static str {
    match self {
      ResourceAction::Describe => "Describe",
      ResourceAction::Yaml => "YAML",
      ResourceAction::Edit => "Edit",
      ResourceAction::Logs => "Logs",
      ResourceAction::Shell => "Shell",
      ResourceAction::PortForward => "Port-forward",
      ResourceAction::PreviousLogs => "Previous logs",
      ResourceAction::Restart => "Rollout restart",
      ResourceAction::Scale => "Scale",
      ResourceAction::Cordon => "Cordon / Uncordon",
      ResourceAction::Suspend => "Suspend / Resume",
      ResourceAction::Trigger => "Trigger now",
      ResourceAction::DecodeSecret => "Decode secret",
      ResourceAction::Delete => "Delete",
    }
  }

  /// The hotkey that triggers this action for the given block, if it has one.
  /// Hotkey-backed actions are replayed through the normal handler when selected
  /// from the menu so the menu and hotkeys share one dispatch path; menu-only
  /// actions return `None` and are dispatched directly by the menu handler.
  /// `block` is needed because a few actions (logs) map to different keys
  /// depending on the view.
  pub fn hotkey(self, block: ActiveBlock) -> Option<Key> {
    match self {
      ResourceAction::Describe => Some(DEFAULT_KEYBINDING.describe_resource.key),
      ResourceAction::Yaml => Some(DEFAULT_KEYBINDING.resource_yaml.key),
      ResourceAction::Edit => Some(DEFAULT_KEYBINDING.edit_resource.key),
      // In the Containers view Enter opens the selected container's logs; for
      // pods and workloads logs come from the aggregate-logs key.
      ResourceAction::Logs => Some(match block {
        ActiveBlock::Containers => DEFAULT_KEYBINDING.submit.key,
        _ => DEFAULT_KEYBINDING.aggregate_logs.key,
      }),
      ResourceAction::Shell => Some(DEFAULT_KEYBINDING.shell_exec.key),
      ResourceAction::PortForward => Some(DEFAULT_KEYBINDING.port_forward.key),
      ResourceAction::PreviousLogs => Some(DEFAULT_KEYBINDING.previous_logs.key),
      ResourceAction::Restart => Some(DEFAULT_KEYBINDING.restart_resource.key),
      ResourceAction::DecodeSecret => Some(DEFAULT_KEYBINDING.decode_secret.key),
      ResourceAction::Delete => Some(DEFAULT_KEYBINDING.delete_resource.key),
      // Menu-only actions: they need a value (scale) or a derived direction
      // (cordon/suspend) so they open an input/confirm overlay from the menu
      // rather than firing a single hotkey.
      ResourceAction::Scale
      | ResourceAction::Cordon
      | ResourceAction::Suspend
      | ResourceAction::Trigger => None,
    }
  }
}

/// Returns the ordered list of actions available for the selected item in the
/// given block. Empty for blocks that have no item-level actions (menus, help,
/// logs, etc.).
pub fn actions_for(block: ActiveBlock) -> Vec<ResourceAction> {
  use ResourceAction::*;
  match block {
    ActiveBlock::Containers => vec![Logs, PreviousLogs, Shell],
    ActiveBlock::Pods => vec![
      Describe,
      Yaml,
      Edit,
      Logs,
      PreviousLogs,
      PortForward,
      Delete,
    ],
    // Services are port-forwardable but not pod-bearing (no logs/shell).
    ActiveBlock::Services => vec![Describe, Yaml, Edit, PortForward, Delete],
    ActiveBlock::Secrets => vec![Describe, Yaml, Edit, DecodeSecret, Delete],
    // Deployments and statefulsets are both rollout-restartable and scalable.
    ActiveBlock::Deployments | ActiveBlock::StatefulSets => {
      vec![Describe, Yaml, Edit, Logs, Restart, Scale, Delete]
    }
    // Daemonsets are restartable but not scalable (no replica count).
    ActiveBlock::DaemonSets => vec![Describe, Yaml, Edit, Logs, Restart, Delete],
    // Replicasets and replicationcontrollers are scalable but not restartable.
    ActiveBlock::ReplicaSets | ActiveBlock::ReplicationControllers => {
      vec![Describe, Yaml, Edit, Logs, Scale, Delete]
    }
    ActiveBlock::Jobs => vec![Describe, Yaml, Edit, Logs, Delete],
    ActiveBlock::Nodes => vec![Describe, Yaml, Edit, Cordon, Delete],
    ActiveBlock::CronJobs => vec![Describe, Yaml, Edit, Logs, Suspend, Trigger, Delete],
    // Troubleshoot findings support describe/yaml (handled by the troubleshoot
    // route), so the `m` hint shown on that pane is honest.
    ActiveBlock::Troubleshoot => vec![Describe, Yaml],
    ActiveBlock::ConfigMaps
    | ActiveBlock::StorageClasses
    | ActiveBlock::Roles
    | ActiveBlock::RoleBindings
    | ActiveBlock::ClusterRoles
    | ActiveBlock::ClusterRoleBindings
    | ActiveBlock::Ingresses
    | ActiveBlock::PersistentVolumeClaims
    | ActiveBlock::PersistentVolumes
    | ActiveBlock::NetworkPolicies
    | ActiveBlock::ServiceAccounts
    | ActiveBlock::Events
    | ActiveBlock::DynamicResource => vec![Describe, Yaml, Edit, Delete],
    _ => vec![],
  }
}

/// Transient confirmation overlay drawn over the current view. Not part of the
/// navigation stack — it consumes keys first while active and clears on
/// confirm/cancel. The whole safety model for impactful actions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Modal {
  pub title: String,
  pub prompt: String,
  /// Event dispatched when the modal is confirmed.
  pub on_confirm: IoEvent,
}

impl Modal {
  /// Build a confirmation modal that dispatches `on_confirm` when accepted.
  pub fn confirm(title: impl Into<String>, prompt: impl Into<String>, on_confirm: IoEvent) -> Self {
    Modal {
      title: title.into(),
      prompt: prompt.into(),
      on_confirm,
    }
  }
}

/// Transient single-line text-input overlay for actions that need a value
/// (e.g. a replica count). Like [`Modal`] it sits outside the navigation stack
/// and consumes keys first while active: printable chars edit the buffer,
/// `Enter` validates, `Esc` cancels. A valid submit chains into a confirmation
/// [`Modal`] so the safety model still applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputModal {
  pub title: String,
  pub prompt: String,
  pub buffer: String,
  /// Inline validation error shown under the input; cleared on the next edit.
  pub error: Option<String>,
  /// What the validated value feeds into.
  pub action: InputAction,
}

/// The action an [`InputModal`] feeds once its value validates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputAction {
  /// Scale a workload to a replica count.
  Scale {
    block: ActiveBlock,
    name: String,
    namespace: Option<String>,
    /// Human-readable kind label for the confirmation prompt.
    kind: String,
  },
  /// Port-forward a pod/service; the buffer is `local:remote` (or a single port).
  PortForward {
    /// kubectl resource type (`pods` / `services`).
    kind: String,
    namespace: String,
    name: String,
  },
}

/// What a validated [`InputModal`] feeds into. Impactful actions chain into a
/// confirmation [`Modal`]; non-destructive ones (port-forward) fire directly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputSubmit {
  Confirm(Modal),
  StartPortForward {
    kind: String,
    namespace: String,
    name: String,
    local_port: u16,
    remote_port: u16,
  },
}

impl InputModal {
  /// Validate the current buffer for the pending action. On success returns what
  /// to do next ([`InputSubmit`]); on failure returns an inline error and the
  /// input modal stays open.
  pub fn validate(&self) -> Result<InputSubmit, String> {
    match &self.action {
      InputAction::Scale {
        block,
        name,
        namespace,
        kind,
      } => {
        let replicas: u32 = self
          .buffer
          .trim()
          .parse()
          .map_err(|_| "Enter a non-negative whole number".to_owned())?;
        let prompt = match namespace {
          Some(ns) => format!(
            "Scale {} '{}' in namespace '{}' to {} replica(s)?",
            kind, name, ns, replicas
          ),
          None => format!("Scale {} '{}' to {} replica(s)?", kind, name, replicas),
        };
        Ok(InputSubmit::Confirm(Modal::confirm(
          "Confirm scale",
          prompt,
          IoEvent::PatchResource {
            block: *block,
            name: name.clone(),
            namespace: namespace.clone(),
            patch: ResourcePatch::SetReplicas(replicas),
          },
        )))
      }
      InputAction::PortForward {
        kind,
        namespace,
        name,
      } => {
        let (local_port, remote_port) = parse_port_mapping(&self.buffer)?;
        Ok(InputSubmit::StartPortForward {
          kind: kind.clone(),
          namespace: namespace.clone(),
          name: name.clone(),
          local_port,
          remote_port,
        })
      }
    }
  }
}

/// Parse a `local:remote` port mapping, or a single `port` (local == remote).
/// Ports must be non-zero `u16`s.
fn parse_port_mapping(buffer: &str) -> Result<(u16, u16), String> {
  let buffer = buffer.trim();
  let err = || "Enter ports as local:remote (e.g. 8080:80) or a single port".to_owned();

  let (local, remote) = match buffer.split_once(':') {
    Some((local, remote)) => (local.trim(), remote.trim()),
    None => (buffer, buffer),
  };

  let local: u16 = local.parse().map_err(|_| err())?;
  let remote: u16 = remote.parse().map_err(|_| err())?;
  if local == 0 || remote == 0 {
    return Err("Ports must be between 1 and 65535".to_owned());
  }
  Ok((local, remote))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_actions_for_containers_offers_logs_and_shell() {
    assert_eq!(
      actions_for(ActiveBlock::Containers),
      vec![
        ResourceAction::Logs,
        ResourceAction::PreviousLogs,
        ResourceAction::Shell
      ]
    );
  }

  #[test]
  fn test_actions_for_workloads_offer_logs() {
    assert!(actions_for(ActiveBlock::Deployments).contains(&ResourceAction::Logs));
    assert!(actions_for(ActiveBlock::Pods).contains(&ResourceAction::Logs));
    assert!(actions_for(ActiveBlock::Jobs).contains(&ResourceAction::Logs));
    // Non-pod-bearing resources do not offer logs.
    assert!(!actions_for(ActiveBlock::ConfigMaps).contains(&ResourceAction::Logs));
    assert!(!actions_for(ActiveBlock::Nodes).contains(&ResourceAction::Logs));
  }

  #[test]
  fn test_actions_for_secrets_offers_decode() {
    assert_eq!(
      actions_for(ActiveBlock::Secrets),
      vec![
        ResourceAction::Describe,
        ResourceAction::Yaml,
        ResourceAction::Edit,
        ResourceAction::DecodeSecret,
        ResourceAction::Delete
      ]
    );
  }

  #[test]
  fn test_actions_for_pods_has_no_pod_only_decode() {
    let actions = actions_for(ActiveBlock::Pods);
    assert!(actions.contains(&ResourceAction::Describe));
    assert!(actions.contains(&ResourceAction::Yaml));
    assert!(!actions.contains(&ResourceAction::DecodeSecret));
  }

  #[test]
  fn test_actions_for_scalable_workloads_offer_scale() {
    for block in [
      ActiveBlock::Deployments,
      ActiveBlock::StatefulSets,
      ActiveBlock::ReplicaSets,
      ActiveBlock::ReplicationControllers,
    ] {
      assert!(
        actions_for(block).contains(&ResourceAction::Scale),
        "{:?} should offer Scale",
        block
      );
    }
    // Daemonsets, jobs and pods have no replica count to scale.
    assert!(!actions_for(ActiveBlock::DaemonSets).contains(&ResourceAction::Scale));
    assert!(!actions_for(ActiveBlock::Jobs).contains(&ResourceAction::Scale));
    assert!(!actions_for(ActiveBlock::Pods).contains(&ResourceAction::Scale));
  }

  #[test]
  fn test_scale_is_menu_only() {
    assert_eq!(ResourceAction::Scale.hotkey(ActiveBlock::Deployments), None);
  }

  #[test]
  fn test_actions_for_editable_blocks_offer_edit() {
    for block in [
      ActiveBlock::Pods,
      ActiveBlock::Deployments,
      ActiveBlock::Nodes,
      ActiveBlock::ConfigMaps,
      ActiveBlock::CronJobs,
      ActiveBlock::Secrets,
    ] {
      assert!(
        actions_for(block).contains(&ResourceAction::Edit),
        "{:?} should offer Edit",
        block
      );
    }
    // Containers are not a standalone resource, and troubleshoot findings route
    // elsewhere, so neither offers an in-place edit.
    assert!(!actions_for(ActiveBlock::Containers).contains(&ResourceAction::Edit));
    assert!(!actions_for(ActiveBlock::Troubleshoot).contains(&ResourceAction::Edit));
  }

  #[test]
  fn test_edit_is_hotkey_backed() {
    assert_eq!(
      ResourceAction::Edit.hotkey(ActiveBlock::Pods),
      Some(DEFAULT_KEYBINDING.edit_resource.key)
    );
  }

  fn scale_input(buffer: &str) -> InputModal {
    InputModal {
      title: "Scale".into(),
      prompt: "Replicas:".into(),
      buffer: buffer.into(),
      error: None,
      action: InputAction::Scale {
        block: ActiveBlock::Deployments,
        name: "web".into(),
        namespace: Some("default".into()),
        kind: "deployment".into(),
      },
    }
  }

  fn expect_confirm(submit: InputSubmit) -> Modal {
    match submit {
      InputSubmit::Confirm(modal) => modal,
      other => panic!("expected a confirm modal, got {other:?}"),
    }
  }

  #[test]
  fn test_scale_input_valid_count_builds_confirm_modal() {
    let modal = expect_confirm(scale_input("3").validate().expect("3 is valid"));
    assert!(modal.prompt.contains("to 3 replica(s)"));
    assert_eq!(
      modal.on_confirm,
      IoEvent::PatchResource {
        block: ActiveBlock::Deployments,
        name: "web".into(),
        namespace: Some("default".into()),
        patch: ResourcePatch::SetReplicas(3),
      }
    );
  }

  #[test]
  fn test_scale_input_zero_is_allowed() {
    let modal = expect_confirm(scale_input("0").validate().expect("0 is valid"));
    assert_eq!(
      modal.on_confirm,
      IoEvent::PatchResource {
        block: ActiveBlock::Deployments,
        name: "web".into(),
        namespace: Some("default".into()),
        patch: ResourcePatch::SetReplicas(0),
      }
    );
  }

  fn port_forward_input(buffer: &str) -> InputModal {
    InputModal {
      title: "Port-forward".into(),
      prompt: "Ports:".into(),
      buffer: buffer.into(),
      error: None,
      action: InputAction::PortForward {
        kind: "pods".into(),
        namespace: "default".into(),
        name: "web".into(),
      },
    }
  }

  #[test]
  fn test_port_forward_input_parses_local_remote() {
    let submit = port_forward_input("8080:80")
      .validate()
      .expect("valid mapping");
    assert_eq!(
      submit,
      InputSubmit::StartPortForward {
        kind: "pods".into(),
        namespace: "default".into(),
        name: "web".into(),
        local_port: 8080,
        remote_port: 80,
      }
    );
  }

  #[test]
  fn test_port_forward_input_single_port_maps_both() {
    let submit = port_forward_input(" 9000 ")
      .validate()
      .expect("valid single port");
    assert_eq!(
      submit,
      InputSubmit::StartPortForward {
        kind: "pods".into(),
        namespace: "default".into(),
        name: "web".into(),
        local_port: 9000,
        remote_port: 9000,
      }
    );
  }

  #[test]
  fn test_port_forward_input_rejects_bad_and_zero_ports() {
    assert!(port_forward_input("").validate().is_err());
    assert!(port_forward_input("abc").validate().is_err());
    assert!(port_forward_input("0:80").validate().is_err());
    assert!(port_forward_input("8080:0").validate().is_err());
    assert!(port_forward_input("99999:80").validate().is_err());
  }

  #[test]
  fn test_actions_for_port_forwardable_blocks() {
    assert!(actions_for(ActiveBlock::Pods).contains(&ResourceAction::PortForward));
    assert!(actions_for(ActiveBlock::Services).contains(&ResourceAction::PortForward));
    // Not offered for resources without a forwardable port.
    assert!(!actions_for(ActiveBlock::ConfigMaps).contains(&ResourceAction::PortForward));
    assert!(!actions_for(ActiveBlock::Nodes).contains(&ResourceAction::PortForward));
  }

  #[test]
  fn test_port_forward_is_hotkey_backed() {
    assert_eq!(
      ResourceAction::PortForward.hotkey(ActiveBlock::Pods),
      Some(DEFAULT_KEYBINDING.port_forward.key)
    );
  }

  #[test]
  fn test_scale_input_rejects_negative_empty_and_non_numeric() {
    assert!(scale_input("-1").validate().is_err());
    assert!(scale_input("").validate().is_err());
    assert!(scale_input("two").validate().is_err());
    // Surrounding whitespace is tolerated.
    assert!(scale_input("  2 ").validate().is_ok());
  }

  #[test]
  fn test_actions_for_non_resource_blocks_is_empty() {
    assert!(actions_for(ActiveBlock::Help).is_empty());
    assert!(actions_for(ActiveBlock::Logs).is_empty());
    assert!(actions_for(ActiveBlock::More).is_empty());
  }

  #[test]
  fn test_resource_action_hotkey_matches_bindings() {
    assert_eq!(
      ResourceAction::Describe.hotkey(ActiveBlock::Pods),
      Some(DEFAULT_KEYBINDING.describe_resource.key)
    );
    assert_eq!(
      ResourceAction::DecodeSecret.hotkey(ActiveBlock::Secrets),
      Some(DEFAULT_KEYBINDING.decode_secret.key)
    );
    // Menu-only actions have no hotkey.
    assert_eq!(ResourceAction::Cordon.hotkey(ActiveBlock::Nodes), None);
  }

  #[test]
  fn test_logs_hotkey_is_context_aware() {
    // Containers open logs with Enter; pods/workloads use the aggregate-logs key.
    assert_eq!(
      ResourceAction::Logs.hotkey(ActiveBlock::Containers),
      Some(DEFAULT_KEYBINDING.submit.key)
    );
    assert_eq!(
      ResourceAction::Logs.hotkey(ActiveBlock::Pods),
      Some(DEFAULT_KEYBINDING.aggregate_logs.key)
    );
  }
}
