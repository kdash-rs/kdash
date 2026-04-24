//! Troubleshoot UI rendering.

use ratatui::{
  layout::Rect,
  widgets::{Cell, Row},
  Frame,
};

use super::types::Severity;
use crate::app::key_binding::DEFAULT_KEYBINDING;
use crate::app::models::FilterableTable;
use crate::app::App;
use crate::ui::utils::{
  action_hint, describe_and_yaml_hint, draw_route_resource_block, filter_cursor_position,
  filter_status_parts, help_part, mixed_bold_line, responsive_columns, style_caution,
  style_failure, style_primary, ColumnDef, ResourceTableProps, ViewTier,
};

const FINDING_COLUMNS: [ColumnDef; 6] = [
  ColumnDef::all("Severity", 7, 7, 7),
  ColumnDef::all("Type", 6, 6, 6),
  ColumnDef::all("Reason", 13, 13, 13),
  ColumnDef::all("Resource", 18, 18, 18),
  ColumnDef::all("Message", 44, 44, 44),
  ColumnDef::all("Age", 12, 12, 12),
];

pub fn render_troubleshoot(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let light_theme = app.light_theme;
  let is_loading = app.is_loading();
  let title = format!(
    " Troubleshoot (ns: {}) [{}] ",
    app
      .data
      .selected
      .ns
      .as_ref()
      .unwrap_or(&String::from("all")),
    app.data.troubleshoot_findings.count_label(),
  );
  let title_width = title.chars().count();
  let findings = &mut app.data.troubleshoot_findings;
  let filter = findings.filter.clone();
  let filter_active = findings.filter_active;

  let mut inline_help = vec![];
  inline_help.extend(filter_status_parts(&filter, filter_active));
  if !filter_active {
    inline_help.extend([
      help_part(format!(
        " | {} | ",
        action_hint("resource", DEFAULT_KEYBINDING.submit.key)
      )),
      help_part(describe_and_yaml_hint()),
    ]);
  }

  let (headers, widths) = responsive_columns(&FINDING_COLUMNS, ViewTier::Compact);

  draw_route_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: mixed_bold_line(inline_help, app.light_theme),
      resource: findings,
      table_headers: headers,
      column_widths: widths,
    },
    |c| {
      let style = match c.severity {
        Severity::Error => style_failure(light_theme),
        Severity::Warn => style_caution(light_theme),
        Severity::Info => style_primary(light_theme),
      };

      Row::new(vec![
        Cell::from(c.severity.to_string()),
        Cell::from(c.resource_kind.to_string()),
        Cell::from(c.reason.clone()),
        Cell::from(c.resource_ref()),
        Cell::from(c.message.clone()),
        Cell::from(c.age.clone()),
      ])
      .style(style)
    },
    light_theme,
    is_loading,
  );

  if filter_active {
    f.set_cursor_position(filter_cursor_position(area, title_width, &filter));
  }
}
