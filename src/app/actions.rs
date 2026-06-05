//! Central registry of resource actions and the overlay modal state that backs
//! the `m` action menu and the confirmation / input dialogs.
//!
//! `actions_for` is the single source of truth for "what can I do to the
//! selected item" so the menu, hotkey hints, and handlers can never drift.
use crate::app::key_binding::DEFAULT_KEYBINDING;
use crate::app::ActiveBlock;
use crate::event::Key;

/// An action that can be performed on the selected resource. Surfaced both as a
/// hotkey (with a hint in the hint area) and as an entry in the `m` action menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceAction {
  Describe,
  Yaml,
  Shell,
  DecodeSecret,
}

impl ResourceAction {
  /// Human-readable label shown in the action menu.
  pub fn label(self) -> &'static str {
    match self {
      ResourceAction::Describe => "Describe",
      ResourceAction::Yaml => "YAML",
      ResourceAction::Shell => "Shell",
      ResourceAction::DecodeSecret => "Decode secret",
    }
  }

  /// The hotkey that triggers this action. Selecting the action from the menu
  /// replays this key through the normal handler so there is one dispatch path.
  pub fn hotkey(self) -> Key {
    match self {
      ResourceAction::Describe => DEFAULT_KEYBINDING.describe_resource.key,
      ResourceAction::Yaml => DEFAULT_KEYBINDING.resource_yaml.key,
      ResourceAction::Shell => DEFAULT_KEYBINDING.shell_exec.key,
      ResourceAction::DecodeSecret => DEFAULT_KEYBINDING.decode_secret.key,
    }
  }
}

/// Returns the ordered list of actions available for the selected item in the
/// given block. Empty for blocks that have no item-level actions (menus, help,
/// logs, etc.).
pub fn actions_for(block: ActiveBlock) -> Vec<ResourceAction> {
  use ResourceAction::*;
  match block {
    ActiveBlock::Containers => vec![Shell],
    ActiveBlock::Secrets => vec![Describe, Yaml, DecodeSecret],
    ActiveBlock::Pods
    | ActiveBlock::Services
    | ActiveBlock::Nodes
    | ActiveBlock::ConfigMaps
    | ActiveBlock::StatefulSets
    | ActiveBlock::ReplicaSets
    | ActiveBlock::Deployments
    | ActiveBlock::Jobs
    | ActiveBlock::DaemonSets
    | ActiveBlock::CronJobs
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
    | ActiveBlock::DynamicResource => vec![Describe, Yaml],
    _ => vec![],
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_actions_for_containers_offers_shell() {
    assert_eq!(actions_for(ActiveBlock::Containers), vec![ResourceAction::Shell]);
  }

  #[test]
  fn test_actions_for_secrets_offers_decode() {
    assert_eq!(
      actions_for(ActiveBlock::Secrets),
      vec![
        ResourceAction::Describe,
        ResourceAction::Yaml,
        ResourceAction::DecodeSecret
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
      DEFAULT_KEYBINDING.describe_resource.key
    );
    assert_eq!(
      ResourceAction::DecodeSecret.hotkey(),
      DEFAULT_KEYBINDING.decode_secret.key
    );
  }
}
