//! Central registry of resource actions and the overlay modal state that backs
//! the `m` action menu and the confirmation / input dialogs.
//!
//! `actions_for` is the single source of truth for "what can I do to the
//! selected item" so the menu, hotkey hints, and handlers can never drift.
use crate::app::key_binding::DEFAULT_KEYBINDING;
use crate::app::ActiveBlock;
use crate::event::Key;
use crate::network::IoEvent;

/// An action that can be performed on the selected resource. Surfaced both as a
/// hotkey (with a hint in the hint area) and as an entry in the `m` action menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceAction {
  Describe,
  Yaml,
  Shell,
  PreviousLogs,
  Restart,
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
      ResourceAction::Shell => "Shell",
      ResourceAction::PreviousLogs => "Previous logs",
      ResourceAction::Restart => "Rollout restart",
      ResourceAction::Cordon => "Cordon / Uncordon",
      ResourceAction::Suspend => "Suspend / Resume",
      ResourceAction::Trigger => "Trigger now",
      ResourceAction::DecodeSecret => "Decode secret",
      ResourceAction::Delete => "Delete",
    }
  }

  /// The hotkey that triggers this action, if it has one. Hotkey-backed actions
  /// are replayed through the normal handler when selected from the menu so the
  /// menu and hotkeys share one dispatch path; menu-only actions return `None`
  /// and are dispatched directly by the menu handler.
  pub fn hotkey(self) -> Option<Key> {
    match self {
      ResourceAction::Describe => Some(DEFAULT_KEYBINDING.describe_resource.key),
      ResourceAction::Yaml => Some(DEFAULT_KEYBINDING.resource_yaml.key),
      ResourceAction::Shell => Some(DEFAULT_KEYBINDING.shell_exec.key),
      ResourceAction::PreviousLogs => Some(DEFAULT_KEYBINDING.previous_logs.key),
      ResourceAction::Restart => Some(DEFAULT_KEYBINDING.restart_resource.key),
      ResourceAction::DecodeSecret => Some(DEFAULT_KEYBINDING.decode_secret.key),
      ResourceAction::Delete => Some(DEFAULT_KEYBINDING.delete_resource.key),
      ResourceAction::Cordon | ResourceAction::Suspend | ResourceAction::Trigger => None,
    }
  }
}

/// Returns the ordered list of actions available for the selected item in the
/// given block. Empty for blocks that have no item-level actions (menus, help,
/// logs, etc.).
pub fn actions_for(block: ActiveBlock) -> Vec<ResourceAction> {
  use ResourceAction::*;
  match block {
    ActiveBlock::Containers => vec![Shell, PreviousLogs],
    ActiveBlock::Pods => vec![Describe, Yaml, PreviousLogs, Delete],
    ActiveBlock::Secrets => vec![Describe, Yaml, DecodeSecret, Delete],
    ActiveBlock::Deployments | ActiveBlock::StatefulSets | ActiveBlock::DaemonSets => {
      vec![Describe, Yaml, Restart, Delete]
    }
    ActiveBlock::Nodes => vec![Describe, Yaml, Cordon, Delete],
    ActiveBlock::CronJobs => vec![Describe, Yaml, Suspend, Trigger, Delete],
    ActiveBlock::Services
    | ActiveBlock::ConfigMaps
    | ActiveBlock::ReplicaSets
    | ActiveBlock::Jobs
    | ActiveBlock::ReplicationControllers
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
    | ActiveBlock::DynamicResource => vec![Describe, Yaml, Delete],
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_actions_for_containers_offers_shell() {
    assert_eq!(
      actions_for(ActiveBlock::Containers),
      vec![ResourceAction::Shell, ResourceAction::PreviousLogs]
    );
  }

  #[test]
  fn test_actions_for_secrets_offers_decode() {
    assert_eq!(
      actions_for(ActiveBlock::Secrets),
      vec![
        ResourceAction::Describe,
        ResourceAction::Yaml,
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
  fn test_actions_for_non_resource_blocks_is_empty() {
    assert!(actions_for(ActiveBlock::Help).is_empty());
    assert!(actions_for(ActiveBlock::Logs).is_empty());
    assert!(actions_for(ActiveBlock::More).is_empty());
  }

  #[test]
  fn test_resource_action_hotkey_matches_bindings() {
    assert_eq!(
      ResourceAction::Describe.hotkey(),
      Some(DEFAULT_KEYBINDING.describe_resource.key)
    );
    assert_eq!(
      ResourceAction::DecodeSecret.hotkey(),
      Some(DEFAULT_KEYBINDING.decode_secret.key)
    );
    // Menu-only actions have no hotkey.
    assert_eq!(ResourceAction::Cordon.hotkey(), None);
  }
}
