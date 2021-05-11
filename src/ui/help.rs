use super::super::app::App;
use super::utils::{
  layout_block_active_span, style_highlight, style_primary, style_secondary, title_with_dual_style,
  vertical_chunks,
};
use super::HIGHLIGHT;

use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  widgets::{Row, Table},
  Frame,
};

pub fn draw_help<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let chunks = vertical_chunks(vec![Constraint::Percentage(100)], area);

  // Create a one-column table to avoid flickering due to non-determinism when
  // resolving constraints on widths of table columns.
  let format_row =
    |r: &Vec<String>| -> Vec<String> { vec![format!("{:50}{:40}{:20}", r[0], r[1], r[2])] };

  let header = ["Key", "Action", "Context"];
  let header = format_row(&header.iter().map(|s| s.to_string()).collect());

  let help_docs = app
    .help_docs
    .items
    .iter()
    .map(format_row)
    .collect::<Vec<Vec<String>>>();
  let help_docs = &help_docs[0_usize..];

  let rows = help_docs
    .iter()
    .map(|item| Row::new(item.clone()).style(style_primary()));

  let title = title_with_dual_style("Help ".into(), "| close <esc>".into(), app.light_theme);

  let help_menu = Table::new(rows)
    .header(Row::new(header).style(style_secondary()).bottom_margin(0))
    .block(layout_block_active_span(title))
    .highlight_style(style_highlight())
    .highlight_symbol(HIGHLIGHT)
    .widths(&[Constraint::Max(110)]);
  f.render_stateful_widget(help_menu, chunks[0], &mut app.help_docs.state);
}
