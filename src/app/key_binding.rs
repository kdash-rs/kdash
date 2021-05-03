use std::fmt;

use super::super::event::Key;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub enum HContext {
  General,
  Overview,
  Utilization,
}

impl fmt::Display for HContext {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}
#[derive(Clone)]
pub struct KeyBinding {
  pub key: Key,
  pub desc: &'static str,
  pub context: HContext,
}

#[derive(Clone)]
pub struct KeyBindings {
  pub ctr_c: KeyBinding,
  pub quit: KeyBinding,
  pub esc: KeyBinding,
  pub help: KeyBinding,
  pub submit: KeyBinding,
  pub refresh: KeyBinding,
  pub toggle_theme: KeyBinding,
  pub jump_to_all_context: KeyBinding,
  pub jump_to_current_context: KeyBinding,
  pub jump_to_utilization: KeyBinding,
  pub up: KeyBinding,
  pub down: KeyBinding,
  pub left: KeyBinding,
  pub right: KeyBinding,
  pub toggle_info: KeyBinding,
  pub log_auto_scroll: KeyBinding,
  pub select_all_namespace: KeyBinding,
  pub jump_to_namespace: KeyBinding,
  pub jump_to_pods: KeyBinding,
  pub jump_to_services: KeyBinding,
  pub jump_to_nodes: KeyBinding,
  pub jump_to_configmaps: KeyBinding,
  pub jump_to_deployments: KeyBinding,
  pub jump_to_statefulsets: KeyBinding,
  pub jump_to_replicasets: KeyBinding,
  pub describe_resource: KeyBinding,
  pub resource_yaml: KeyBinding,
  pub cycle_group_by: KeyBinding,
  pub copy_to_clipboard: KeyBinding,
}

// update the as_vec method below with field as well
pub const DEFAULT_KEYBINDING: KeyBindings = KeyBindings {
  ctr_c: KeyBinding {
    key: Key::Ctrl('c'),
    desc: "Quit",
    context: HContext::General,
  },
  quit: KeyBinding {
    key: Key::Char('q'),
    desc: "Quit",
    context: HContext::General,
  },
  esc: KeyBinding {
    key: Key::Esc,
    desc: "Close popup page",
    context: HContext::General,
  },
  help: KeyBinding {
    key: Key::Char('?'),
    desc: "Help page",
    context: HContext::General,
  },
  submit: KeyBinding {
    key: Key::Enter,
    desc: "Select table row",
    context: HContext::General,
  },
  refresh: KeyBinding {
    key: Key::Ctrl('r'),
    desc: "Refresh data",
    context: HContext::General,
  },
  toggle_theme: KeyBinding {
    key: Key::Char('t'),
    desc: "Toggle theme",
    context: HContext::General,
  },
  jump_to_current_context: KeyBinding {
    key: Key::Char('A'),
    desc: "Switch to active context view",
    context: HContext::General,
  },
  jump_to_all_context: KeyBinding {
    key: Key::Char('C'),
    desc: "Switch to all contexts view",
    context: HContext::General,
  },
  jump_to_utilization: KeyBinding {
    key: Key::Char('U'),
    desc: "Switch to resource utilization view",
    context: HContext::General,
  },
  copy_to_clipboard: KeyBinding {
    key: Key::Char('c'),
    desc: "Copy log/output to clipboard",
    context: HContext::General,
  },
  up: KeyBinding {
    key: Key::Down,
    desc: "Next table row",
    context: HContext::General,
  },
  down: KeyBinding {
    key: Key::Up,
    desc: "Previous table row",
    context: HContext::General,
  },
  left: KeyBinding {
    key: Key::Left,
    desc: "Next resource tab",
    context: HContext::Overview,
  },
  right: KeyBinding {
    key: Key::Right,
    desc: "Previous resource tab",
    context: HContext::Overview,
  },
  toggle_info: KeyBinding {
    key: Key::Char('i'),
    desc: "Show/Hide info bar",
    context: HContext::Overview,
  },
  log_auto_scroll: KeyBinding {
    key: Key::Char('s'),
    desc: "Toggle log auto scroll",
    context: HContext::Overview,
  },
  jump_to_namespace: KeyBinding {
    key: Key::Char('n'),
    desc: "Select namespace block",
    context: HContext::Overview,
  },
  select_all_namespace: KeyBinding {
    key: Key::Char('a'),
    desc: "Select all namespaces",
    context: HContext::Overview,
  },
  describe_resource: KeyBinding {
    key: Key::Char('d'),
    desc: "Describe resource",
    context: HContext::Overview,
  },
  resource_yaml: KeyBinding {
    key: Key::Char('y'),
    desc: "Get Resource YAML",
    context: HContext::Overview,
  },
  jump_to_pods: KeyBinding {
    key: Key::Char('1'),
    desc: "Select pods tab",
    context: HContext::Overview,
  },
  jump_to_services: KeyBinding {
    key: Key::Char('2'),
    desc: "Select services tab",
    context: HContext::Overview,
  },
  jump_to_nodes: KeyBinding {
    key: Key::Char('3'),
    desc: "Select nodes tab",
    context: HContext::Overview,
  },
  jump_to_configmaps: KeyBinding {
    key: Key::Char('4'),
    desc: "Select configmaps tab",
    context: HContext::Overview,
  },
  jump_to_statefulsets: KeyBinding {
    key: Key::Char('5'),
    desc: "Select replicasets tab",
    context: HContext::Overview,
  },
  jump_to_replicasets: KeyBinding {
    key: Key::Char('6'),
    desc: "Select statefulsets tab",
    context: HContext::Overview,
  },
  jump_to_deployments: KeyBinding {
    key: Key::Char('7'),
    desc: "Select deployments tab",
    context: HContext::Overview,
  },
  cycle_group_by: KeyBinding {
    key: Key::Char('g'),
    desc: "Cycle through grouping",
    context: HContext::Utilization,
  },
};

impl KeyBindings {
  // mainly used for showing the help screen
  pub fn as_vec(&self) -> Vec<&KeyBinding> {
    vec![
      // order here is shown as is in Help
      &self.ctr_c,
      &self.quit,
      &self.esc,
      &self.help,
      &self.submit,
      &self.refresh,
      &self.toggle_theme,
      &self.jump_to_current_context,
      &self.jump_to_all_context,
      &self.jump_to_utilization,
      &self.copy_to_clipboard,
      &self.down,
      &self.up,
      &self.left,
      &self.right,
      &self.toggle_info,
      &self.log_auto_scroll,
      &self.select_all_namespace,
      &self.jump_to_namespace,
      &self.describe_resource,
      &self.resource_yaml,
      &self.jump_to_pods,
      &self.jump_to_services,
      &self.jump_to_nodes,
      &self.jump_to_configmaps,
      &self.jump_to_statefulsets,
      &self.jump_to_replicasets,
      &self.jump_to_deployments,
      &self.cycle_group_by,
    ]
  }
}
