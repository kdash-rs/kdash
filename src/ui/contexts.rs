use super::super::app::App;
use super::utils::{
  layout_block_default, loading, style_highlight, style_primary, style_secondary,
  table_header_style,
};
use super::HIGHLIGHT;

use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  widgets::{Cell, Row, Table},
  Frame,
};

pub fn draw_contexts<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = format!("Contexts [{}]", app.data.contexts.items.len());
  let block = layout_block_default(title.as_str());

  if !app.data.contexts.items.is_empty() {
    let rows = app.data.contexts.items.iter().map(|c| {
      let style = if c.is_active {
        style_secondary()
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
      .header(table_header_style(
        vec!["Context", "Cluster", "User"],
        app.light_theme,
      ))
      .block(block)
      .widths(&[
        Constraint::Percentage(34),
        Constraint::Percentage(33),
        Constraint::Percentage(33),
      ])
      .highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT);

    f.render_stateful_widget(table, area, &mut app.data.contexts.state);
  } else {
    loading(f, block, area, app.is_loading);
  }
}
