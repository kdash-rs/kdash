use kubectl_view_allocations::{qty::Qty, tree::provide_prefix};
use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  widgets::{Cell, Row, Table},
  Frame,
};

use super::utils::{
  layout_block_active, loading, style_highlight, style_primary, style_success, style_warning,
  table_header_style,
};
use crate::app::App;

pub fn draw_utilization<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let title = format!(
    " Resource Utilization (ns: [{}], group by <g>: {:?}) ",
    app
      .data
      .selected
      .ns
      .as_ref()
      .unwrap_or(&String::from("all")),
    app.utilization_group_by
  );
  let block = layout_block_active(title.as_str(), app.light_theme);

  if !app.data.metrics.items.is_empty() {
    let data = &app.data.metrics.items;

    let prefixes = provide_prefix(data, |parent, item| parent.0.len() + 1 == item.0.len());

    // Create the table
    let mut rows: Vec<Row<'_>> = vec![];
    for ((k, oqtys), prefix) in data.iter().zip(prefixes.iter()) {
      let column0 = format!(
        "{} {}",
        prefix,
        k.last().map(|x| x.as_str()).unwrap_or("???")
      );
      if let Some(qtys) = oqtys {
        let style = if qtys.requested > qtys.limit || qtys.utilization > qtys.limit {
          style_warning(app.light_theme)
        } else if is_empty(&qtys.requested) || is_empty(&qtys.limit) {
          style_primary(app.light_theme)
        } else {
          style_success(app.light_theme)
        };

        let row = Row::new(vec![
          Cell::from(column0),
          make_table_cell(&qtys.utilization, &qtys.allocatable),
          make_table_cell(&qtys.requested, &qtys.allocatable),
          make_table_cell(&qtys.limit, &qtys.allocatable),
          make_table_cell(&qtys.allocatable, &None),
          make_table_cell(&qtys.calc_free(), &None),
        ])
        .style(style);
        rows.push(row);
      }
    }

    let table = Table::new(rows)
      .header(table_header_style(
        vec![
          "Resource",
          "Utilization",
          "Requested",
          "Limit",
          "Allocatable",
          "Free",
        ],
        app.light_theme,
      ))
      .block(block)
      .widths(&[
        Constraint::Percentage(50),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
      ])
      .highlight_style(style_highlight());

    f.render_stateful_widget(table, area, &mut app.data.metrics.state);
  } else {
    loading(f, block, area, app.is_loading, app.light_theme);
  }
}

fn make_table_cell<'a>(oqty: &Option<Qty>, o100: &Option<Qty>) -> Cell<'a> {
  let txt = match oqty {
    None => "__".into(),
    Some(ref qty) => match o100 {
      None => format!("{}", qty.adjust_scale()),
      Some(q100) => format!("{} ({:.0}%)", qty.adjust_scale(), qty.calc_percentage(q100)),
    },
  };
  Cell::from(txt)
}

fn is_empty(oqty: &Option<Qty>) -> bool {
  match oqty {
    Some(qty) => qty.is_zero(),
    None => true,
  }
}
