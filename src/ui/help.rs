use crate::app::DEFAULT_KEYBINDING;
use crate::event::Key;
use std::fmt;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
enum HContext {
  General,
  Overview,
  Contexts,
}

impl fmt::Display for HContext {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match *self {
      _ => write!(f, "{:?}", self),
    }
  }
}

fn help_row(key: Key, desc: &str, context: HContext) -> Vec<String> {
  vec![key.to_string(), String::from(desc), context.to_string()]
}

pub fn get_help_docs() -> Vec<Vec<String>> {
  vec![
    help_row(
      DEFAULT_KEYBINDING.esc,
      "Close popup page",
      HContext::General,
    ),
    vec![
      format!(
        "{} | {}",
        DEFAULT_KEYBINDING.quit.to_string(),
        Key::Ctrl('c')
      ),
      String::from("Quit"),
      HContext::General.to_string(),
    ],
    help_row(DEFAULT_KEYBINDING.help, "Help page", HContext::General),
    help_row(
      DEFAULT_KEYBINDING.submit,
      "Select table row",
      HContext::General,
    ),
    help_row(
      DEFAULT_KEYBINDING.refresh,
      "Refresh data",
      HContext::General,
    ),
    help_row(
      DEFAULT_KEYBINDING.toggle_theme,
      "Toggle theme",
      HContext::General,
    ),
    help_row(
      DEFAULT_KEYBINDING.jump_to_all_context,
      "Switch to all contexts view",
      HContext::General,
    ),
    help_row(
      DEFAULT_KEYBINDING.jump_to_current_context,
      "Switch to active context view",
      HContext::General,
    ),
    help_row(DEFAULT_KEYBINDING.down, "Next table row", HContext::General),
    help_row(
      DEFAULT_KEYBINDING.up,
      "Previous table row",
      HContext::General,
    ),
    help_row(
      DEFAULT_KEYBINDING.right,
      "Next resource tab",
      HContext::Overview,
    ),
    help_row(
      DEFAULT_KEYBINDING.left,
      "Previous resource tab",
      HContext::Overview,
    ),
    help_row(
      DEFAULT_KEYBINDING.toggle_info,
      "Show/Hide info bar",
      HContext::Overview,
    ),
    help_row(
      DEFAULT_KEYBINDING.jump_to_namespace,
      "Select namespace block",
      HContext::Overview,
    ),
    help_row(
      DEFAULT_KEYBINDING.jump_to_pods,
      "Select pods tab",
      HContext::Overview,
    ),
    help_row(
      DEFAULT_KEYBINDING.jump_to_services,
      "Select services tab",
      HContext::Overview,
    ),
    help_row(
      DEFAULT_KEYBINDING.jump_to_nodes,
      "Select nodes tab",
      HContext::Overview,
    ),
  ]
}
