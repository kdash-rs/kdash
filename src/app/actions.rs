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
  Logs,
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
      ResourceAction::Logs => "Logs",
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
      // In the Containers view Enter opens the selected container's logs; for
      // pods and workloads logs come from the aggregate-logs key.
      ResourceAction::Logs => Some(match block {
        ActiveBlock::Containers => DEFAULT_KEYBINDING.submit.key,
        _ => DEFAULT_KEYBINDING.aggregate_logs.key,
      }),
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
    ActiveBlock::Containers => vec![Logs, PreviousLogs, Shell],
    ActiveBlock::Pods => vec![Describe, Yaml, Logs, PreviousLogs, Delete],
    ActiveBlock::Secrets => vec![Describe, Yaml, DecodeSecret, Delete],
    ActiveBlock::Deployments | ActiveBlock::StatefulSets | ActiveBlock::DaemonSets => {
      vec![Describe, Yaml, Logs, Restart, Delete]
    }
    ActiveBlock::ReplicaSets | ActiveBlock::Jobs | ActiveBlock::ReplicationControllers => {
      vec![Describe, Yaml, Logs, Delete]
    }
    ActiveBlock::Nodes => vec![Describe, Yaml, Cordon, Delete],
    ActiveBlock::CronJobs => vec![Describe, Yaml, Logs, Suspend, Trigger, Delete],
    // Troubleshoot findings support describe/yaml (handled by the troubleshoot
    // route), so the `m` hint shown on that pane is honest.
    ActiveBlock::Troubleshoot => vec![Describe, Yaml],
    ActiveBlock::Services
    | ActiveBlock::ConfigMaps
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
