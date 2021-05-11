use std::fmt;

use super::super::event::Key;

// using a macro so that we can automatically generate an iterable vector for bindings. This beats reflection :)
macro_rules! generate_keybindings {
  ($($field:ident),+) => {
    pub struct KeyBindings { $(pub $field: KeyBinding),+ }
    impl KeyBindings {
      pub fn as_iter(&self) -> Vec<&KeyBinding> {
        vec![
            $(&self.$field),+
        ]
      }
    }
  };
}

generate_keybindings! {
  // order here is shown as is in Help
  ctr_c,
  quit,
  esc,
  help,
  submit,
  refresh,
  toggle_theme,
  jump_to_current_context,
  jump_to_all_context,
  jump_to_utilization,
  copy_to_clipboard,
  down,
  up,
  left,
  right,
  toggle_info,
  log_auto_scroll,
  select_all_namespace,
  jump_to_namespace,
  describe_resource,
  resource_yaml,
  jump_to_pods,
  jump_to_services,
  jump_to_nodes,
  jump_to_configmaps,
  jump_to_statefulsets,
  jump_to_replicasets,
  jump_to_deployments,
  cycle_group_by
}
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

#[cfg(test)]
mod tests {
  use super::DEFAULT_KEYBINDING;

  #[test]
  fn test_as_iter() {
    assert!(DEFAULT_KEYBINDING.as_iter().len() >= 29);
  }
}
