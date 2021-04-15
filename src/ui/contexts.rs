use crate::app::App;
use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  widgets::{Row, Table},
  Frame,
};

use super::utils::{
  layout_block_default, style_highlight, style_primary, style_success, table_header_style,
};

static HIGHLIGHT: &'static str = "=> ";

pub fn draw_contexts<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = format!("Contexts [{}]", app.contexts.items.len());
  let block = layout_block_default(title.as_str());

  let rows = app.contexts.items.iter().map(|c| {
    let style = if c.is_active == true {
      style_success()
    } else {
      style_primary()
    };
    Row::new(vec![c.name.as_ref(), c.cluster.as_ref(), c.user.as_ref()]).style(style)
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
}
