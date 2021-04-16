use super::HIGHLIGHT;
use crate::app::App;
use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  widgets::{Cell, Row, Table},
  Frame,
};

use super::utils::{
  layout_block_default, loading, style_highlight, style_primary, style_success, table_header_style,
};

pub fn draw_contexts<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = format!("Contexts [{}]", app.contexts.items.len());
  let block = layout_block_default(title.as_str());

  if !app.contexts.items.is_empty() {
    let rows = app.contexts.items.iter().map(|c| {
      let style = if c.is_active {
        style_success()
      } else {
        style_primary()
      };
      Row::new(vec![
        Cell::from(c.name.as_ref()),
        Cell::from(c.cluster.as_ref()),
        Cell::from(c.user.as_ref()),
      ])
      .style(style)
    });

    let table = Table::new(rows)
      .header(table_header_style(vec!["Context", "Cluster", "User"]))
      .block(block)
      .widths(&[
        Constraint::Percentage(34),
        Constraint::Percentage(33),
        Constraint::Percentage(33),
      ])
      .highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT);

    f.render_stateful_widget(table, area, &mut app.contexts.state);
  } else {
    loading(f, block, area, app.is_loading);
  }
}
