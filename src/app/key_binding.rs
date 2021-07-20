use std::fmt;

use crate::event::Key;

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
  quit,
  esc,
  help,
  submit,
  refresh,
  toggle_theme,
  cycle_main_views,
  jump_to_current_context,
  jump_to_all_context,
  jump_to_utilization,
  copy_to_clipboard,
  down,
  up,
  left,
  right,
  pg_up,
  pg_down,
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
  jump_to_jobs,
  jump_to_daemonsets,
  jump_to_more_resources,
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
  pub alt: Option<Key>,
  pub desc: &'static str,
  pub context: HContext,
}

pub const DEFAULT_KEYBINDING: KeyBindings = KeyBindings {
  quit: KeyBinding {
    key: Key::Ctrl('c'),
    alt: Some(Key::Char('q')),
    desc: "Quit",
    context: HContext::General,
  },
  esc: KeyBinding {
    key: Key::Esc,
    alt: None,
    desc: "Close child page/Go back",
    context: HContext::General,
  },
  help: KeyBinding {
    key: Key::Char('?'),
    alt: None,
    desc: "Help page",
    context: HContext::General,
  },
  submit: KeyBinding {
    key: Key::Enter,
    alt: None,
    desc: "Select table row",
    context: HContext::General,
  },
  refresh: KeyBinding {
    key: Key::Ctrl('r'),
    alt: None,
    desc: "Refresh data",
    context: HContext::General,
  },
  toggle_theme: KeyBinding {
    key: Key::Char('t'),
    alt: None,
    desc: "Toggle theme",
    context: HContext::General,
  },
  jump_to_current_context: KeyBinding {
    key: Key::Char('A'),
    alt: None,
    desc: "Switch to active context view",
    context: HContext::General,
  },
  jump_to_all_context: KeyBinding {
    key: Key::Char('C'),
    alt: None,
    desc: "Switch to all contexts view",
    context: HContext::General,
  },
  jump_to_utilization: KeyBinding {
    key: Key::Char('U'),
    alt: None,
    desc: "Switch to resource utilization view",
    context: HContext::General,
  },
  cycle_main_views: KeyBinding {
    key: Key::Tab,
    alt: None,
    desc: "Cycle through main views",
    context: HContext::General,
  },
  copy_to_clipboard: KeyBinding {
    key: Key::Char('c'),
    alt: None,
    desc: "Copy log/output to clipboard",
    context: HContext::General,
  },
  down: KeyBinding {
    key: Key::Down,
    alt: Some(Key::Char('j')),
    desc: "Next row/Scroll down",
    context: HContext::General,
  },
  up: KeyBinding {
    key: Key::Up,
    alt: Some(Key::Char('k')),
    desc: "Previous row/Scroll up",
    context: HContext::General,
  },
  pg_up: KeyBinding {
    key: Key::PageUp,
    alt: None,
    desc: "Scroll page up",
    context: HContext::General,
  },
  pg_down: KeyBinding {
    key: Key::PageDown,
    alt: None,
    desc: "Scroll page down",
    context: HContext::General,
  },
  left: KeyBinding {
    key: Key::Left,
    alt: Some(Key::Char('h')),
    desc: "Next resource tab",
    context: HContext::Overview,
  },
  right: KeyBinding {
    key: Key::Right,
    alt: Some(Key::Char('l')),
    desc: "Previous resource tab",
    context: HContext::Overview,
  },
  toggle_info: KeyBinding {
    key: Key::Char('i'),
    alt: None,
    desc: "Show/Hide info bar",
    context: HContext::Overview,
  },
  log_auto_scroll: KeyBinding {
    key: Key::Char('s'),
    alt: None,
    desc: "Toggle log auto scroll",
    context: HContext::Overview,
  },
  jump_to_namespace: KeyBinding {
    key: Key::Char('n'),
    alt: None,
    desc: "Select namespace block",
    context: HContext::Overview,
  },
  select_all_namespace: KeyBinding {
    key: Key::Char('a'),
    alt: None,
    desc: "Select all namespaces",
    context: HContext::Overview,
  },
  describe_resource: KeyBinding {
    key: Key::Char('d'),
    alt: None,
    desc: "Describe resource",
    context: HContext::Overview,
  },
  resource_yaml: KeyBinding {
    key: Key::Char('y'),
    alt: None,
    desc: "Get Resource YAML",
    context: HContext::Overview,
  },
  jump_to_pods: KeyBinding {
    key: Key::Char('1'),
    alt: None,
    desc: "Select pods tab",
    context: HContext::Overview,
  },
  jump_to_services: KeyBinding {
    key: Key::Char('2'),
    alt: None,
    desc: "Select services tab",
    context: HContext::Overview,
  },
  jump_to_nodes: KeyBinding {
    key: Key::Char('3'),
    alt: None,
    desc: "Select nodes tab",
    context: HContext::Overview,
  },
  jump_to_configmaps: KeyBinding {
    key: Key::Char('4'),
    alt: None,
    desc: "Select configmaps tab",
    context: HContext::Overview,
  },
  jump_to_statefulsets: KeyBinding {
    key: Key::Char('5'),
    alt: None,
    desc: "Select replicasets tab",
    context: HContext::Overview,
  },
  jump_to_replicasets: KeyBinding {
    key: Key::Char('6'),
    alt: None,
    desc: "Select statefulsets tab",
    context: HContext::Overview,
  },
  jump_to_deployments: KeyBinding {
    key: Key::Char('7'),
    alt: None,
    desc: "Select deployments tab",
    context: HContext::Overview,
  },
  jump_to_jobs: KeyBinding {
    key: Key::Char('8'),
    alt: None,
    desc: "Select jobs tab",
    context: HContext::Overview,
  },
  jump_to_daemonsets: KeyBinding {
    key: Key::Char('9'),
    alt: None,
    desc: "Select daemon sets tab",
    context: HContext::Overview,
  },
  jump_to_more_resources: KeyBinding {
    key: Key::Char('0'),
    alt: None,
    desc: "Select more resources",
    context: HContext::Overview,
  },
  cycle_group_by: KeyBinding {
    key: Key::Char('g'),
    alt: None,
    desc: "Cycle through grouping",
    context: HContext::Utilization,
  },
};

pub fn get_help_docs() -> Vec<Vec<String>> {
  let items = DEFAULT_KEYBINDING.as_iter();

  items.iter().map(|it| help_row(it)).collect()
}

fn help_row(item: &KeyBinding) -> Vec<String> {
  vec![
    if item.alt.is_some() {
      format!("{} | {}", item.key, item.alt.unwrap())
    } else {
      item.key.to_string()
    },
    String::from(item.desc),
    item.context.to_string(),
  ]
}

#[cfg(test)]
mod tests {
  use super::DEFAULT_KEYBINDING;

  #[test]
  fn test_as_iter() {
    assert!(DEFAULT_KEYBINDING.as_iter().len() >= 28);
  }
}
