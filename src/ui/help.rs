use crate::app::models::DEFAULT_KEYBINDING;
use crate::app::App;
use crate::event::Key;
use std::fmt;
use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  widgets::{Row, Table},
  Frame,
};

use super::utils::{layout_block_default, style_primary, style_secondary, vertical_chunks};

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
enum HContext {
  General,
  Overview,
}

impl fmt::Display for HContext {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}

pub fn draw_help_menu<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let chunks = vertical_chunks(vec![Constraint::Percentage(100)], area);

  // Create a one-column table to avoid flickering due to non-determinism when
  // resolving constraints on widths of table columns.
  let format_row =
    |r: Vec<String>| -> Vec<String> { vec![format!("{:50}{:40}{:20}", r[0], r[1], r[2])] };

  let header = ["Key", "Action", "Context"];
  let header = format_row(header.iter().map(|s| s.to_string()).collect());

  let help_docs = get_help_docs();
  let help_docs = help_docs
    .into_iter()
    .map(format_row)
    .collect::<Vec<Vec<String>>>();
  let help_docs = &help_docs[app.help_menu_offset as usize..];

  let rows = help_docs
    .iter()
    .map(|item| Row::new(item.clone()).style(style_primary()));

  let help_menu = Table::new(rows)
    .header(Row::new(header).style(style_secondary()).bottom_margin(0))
    .block(layout_block_default("Help (press <Esc> to go back)"))
    .widths(&[Constraint::Max(110)]);
  f.render_widget(help_menu, chunks[0]);
}

fn get_help_docs() -> Vec<Vec<String>> {
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

fn help_row(key: Key, desc: &str, context: HContext) -> Vec<String> {
  vec![key.to_string(), String::from(desc), context.to_string()]
}
