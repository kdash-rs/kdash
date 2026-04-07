use ratatui::{
  layout::{Constraint, Rect},
  text::{Line, Span, Text},
  widgets::{Block, Borders, Cell, LineGauge, Paragraph, Row, Table},
  Frame,
};

use super::{
  resource_tabs::draw_resource_tabs_block,
  utils::{
    action_hint, default_part, get_gauge_symbol, help_part, horizontal_chunks,
    layout_block_default, layout_block_default_line, loading, mixed_bold_line, style_failure,
    style_logo, style_primary, style_text, vertical_chunks, vertical_chunks_with_margin,
  },
};
use crate::{
  app::{
    key_binding::DEFAULT_KEYBINDING, metrics::KubeNodeMetrics, models::AppResource,
    ns::NamespaceResource, ActiveBlock, App,
  },
  banner::BANNER,
};

pub fn draw_overview(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  if app.show_info_bar {
    let chunks = vertical_chunks(vec![Constraint::Length(9), Constraint::Min(10)], area);
    draw_status_block(f, app, chunks[0]);
    draw_resource_tabs_block(f, app, chunks[1]);
  } else {
    draw_resource_tabs_block(f, app, area);
  }
}

fn draw_status_block(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let chunks = horizontal_chunks(
    vec![
      Constraint::Length(45),
      Constraint::Min(10),
      Constraint::Length(30),
      Constraint::Length(15),
    ],
    area,
  );

  NamespaceResource::render(ActiveBlock::Namespaces, f, app, chunks[0]);
  draw_context_info_block(f, app, chunks[1]);
  draw_cli_version_block(f, app, chunks[2]);
  draw_logo_block(f, app, chunks[3]);
}

fn draw_logo_block(f: &mut Frame<'_>, app: &App, area: Rect) {
  // Banner text with correct styling
  let text = Text::from(BANNER);
  let text = text.patch_style(style_logo(app.light_theme));
  let block = Block::default()
    .borders(Borders::ALL)
    .title(mixed_bold_line(
      [help_part(format!(
        " {} ",
        action_hint("theme", DEFAULT_KEYBINDING.toggle_theme.key)
      ))],
      app.light_theme,
    ));
  // Contains the banner
  let paragraph = Paragraph::new(text).block(block);
  f.render_widget(paragraph, area);
}

fn draw_cli_version_block(f: &mut Frame<'_>, app: &App, area: Rect) {
  let block = layout_block_default(" CLI Info ");
  if !app.data.clis.is_empty() {
    let rows = app.data.clis.iter().map(|s| {
      let style = if s.status {
        style_primary(app.light_theme)
      } else {
        style_failure(app.light_theme)
      };
      Row::new(vec![
        Cell::from(s.name.to_owned()),
        Cell::from(s.version.to_owned()),
      ])
      .style(style)
    });

    let table = Table::new(
      rows,
      [Constraint::Percentage(50), Constraint::Percentage(50)],
    )
    .block(block);
    f.render_widget(table, area);
  } else {
    loading(f, block, area, app.is_loading(), app.light_theme);
  }
}

fn draw_context_info_block(f: &mut Frame<'_>, app: &App, area: Rect) {
  let chunks = vertical_chunks_with_margin(
    vec![
      Constraint::Length(3),
      Constraint::Min(2),
      Constraint::Min(2),
    ],
    area,
    1,
  );

  let block = layout_block_default_line(mixed_bold_line(
    [
      default_part(" Context Info "),
      help_part(format!(
        "{} ",
        action_hint("toggle", DEFAULT_KEYBINDING.toggle_info.key)
      )),
    ],
    app.light_theme,
  ));

  f.render_widget(block, area);

  let text = match &app.data.active_context {
    Some(active_context) => {
      if let Some(user) = &active_context.user {
        vec![
          Line::from(vec![
            Span::styled("Context: ", style_text(app.light_theme)),
            Span::styled(&active_context.name, style_primary(app.light_theme)),
          ]),
          Line::from(vec![
            Span::styled("Cluster: ", style_text(app.light_theme)),
            Span::styled(&active_context.cluster, style_primary(app.light_theme)),
          ]),
          Line::from(vec![
            Span::styled("User: ", style_text(app.light_theme)),
            Span::styled(user, style_primary(app.light_theme)),
          ]),
        ]
      } else {
        vec![
          Line::from(vec![
            Span::styled("Context: ", style_text(app.light_theme)),
            Span::styled(&active_context.name, style_primary(app.light_theme)),
          ]),
          Line::from(vec![
            Span::styled("Cluster: ", style_text(app.light_theme)),
            Span::styled(&active_context.cluster, style_primary(app.light_theme)),
          ]),
          Line::from(vec![
            Span::styled("User: ", style_text(app.light_theme)),
            Span::styled("<none>", style_primary(app.light_theme)),
          ]),
        ]
      }
    }
    None => {
      vec![Line::from(Span::styled(
        "Context information not found",
        style_failure(app.light_theme),
      ))]
    }
  };

  let paragraph = Paragraph::new(text).block(Block::default());
  f.render_widget(paragraph, chunks[0]);

  let ratio = get_nm_ratio(app.data.node_metrics.as_ref(), |nm| nm.cpu_percent);
  let limited_ratio = if ratio > 1f64 { 1f64 } else { ratio };

  let cpu_gauge = LineGauge::default()
    .block(Block::default().title("CPU:"))
    .filled_style(style_primary(app.light_theme))
    .filled_symbol(get_gauge_symbol(app.enhanced_graphics))
    .unfilled_symbol(get_gauge_symbol(app.enhanced_graphics))
    .ratio(limited_ratio)
    .label(Line::from(format!("{:.0}%", ratio * 100.0)));
  f.render_widget(cpu_gauge, chunks[1]);

  let ratio = get_nm_ratio(app.data.node_metrics.as_ref(), |nm| nm.mem_percent);
  let limited_ratio = if ratio > 1f64 { 1f64 } else { ratio };

  let mem_gauge = LineGauge::default()
    .block(Block::default().title("Memory:"))
    .filled_style(style_primary(app.light_theme))
    .filled_symbol(get_gauge_symbol(app.enhanced_graphics))
    .unfilled_symbol(get_gauge_symbol(app.enhanced_graphics))
    .ratio(limited_ratio)
    .label(Line::from(format!("{:.0}%", ratio * 100.0)));
  f.render_widget(mem_gauge, chunks[2]);
}

/// covert percent value from metrics to ratio that gauge can understand
fn get_nm_ratio(node_metrics: &[KubeNodeMetrics], f: fn(b: &KubeNodeMetrics) -> f64) -> f64 {
  if !node_metrics.is_empty() {
    let sum = node_metrics.iter().map(f).sum::<f64>();
    (sum / node_metrics.len() as f64) / 100f64
  } else {
    0f64
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  #[allow(clippy::float_cmp)]
  fn test_get_nm_ratio() {
    let mut app = App::default();
    assert_eq!(
      get_nm_ratio(app.data.node_metrics.as_ref(), |nm| nm.cpu_percent),
      0.0f64
    );
    app.data.node_metrics = vec![
      KubeNodeMetrics {
        cpu_percent: 80f64,
        ..KubeNodeMetrics::default()
      },
      KubeNodeMetrics {
        cpu_percent: 60f64,
        ..KubeNodeMetrics::default()
      },
    ];
    assert_eq!(
      get_nm_ratio(app.data.node_metrics.as_ref(), |nm| nm.cpu_percent),
      0.7f64
    );
  }
}
