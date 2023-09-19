use ratatui::{
  backend::Backend,
  layout::{Constraint, Rect},
  style::Style,
  text::{Line, Span, Text},
  widgets::{Block, Borders, Cell, LineGauge, Paragraph, Row, Table},
  Frame,
};

use super::{
  resource_tabs::draw_resource_tabs_block,
  utils::{
    get_gauge_style, horizontal_chunks, layout_block_default, loading, style_default,
    style_failure, style_logo, style_primary, style_secondary, vertical_chunks,
    vertical_chunks_with_margin,
  },
};
use crate::{
  app::{
    metrics::KubeNodeMetrics, models::AppResource, ns::NamespaceResource, ActiveBlock, App,
    InputMode,
  },
  banner::BANNER,
};

pub fn draw_overview<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  if app.show_info_bar {
    let chunks = vertical_chunks(vec![Constraint::Length(9), Constraint::Min(10)], area);
    draw_status_block(f, app, chunks[0]);
    draw_resource_tabs_block(f, app, chunks[1]);
  } else {
    draw_resource_tabs_block(f, app, area);
  }
}

fn draw_status_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let chunks = horizontal_chunks(
    vec![
      Constraint::Length(35),
      Constraint::Min(10),
      Constraint::Length(30),
      Constraint::Length(32),
    ],
    area,
  );

  NamespaceResource::render(ActiveBlock::Namespaces, f, app, chunks[0]);
  draw_context_info_block(f, app, chunks[1]);
  if app.show_filter_bar {
    draw_filter_block(f, app, chunks[2]);
  } else {
    draw_cli_version_block(f, app, chunks[2]);
  }
  draw_logo_block(f, app, chunks[3]);
}

fn draw_filter_block<B: Backend>(f: &mut Frame<'_, B>, app: &App, area: Rect) {
  let block = layout_block_default(" Global Filter (toggle <f>) ");

  f.render_widget(block, area);

  let mut text = Text::from(vec![Line::from(vec![Span::styled(
    match app.app_input.input_mode {
      InputMode::Normal => "Press <e> to start editing",
      InputMode::Editing => "Press <esc> to stop editing",
    },
    style_default(app.light_theme),
  )])]);

  text.patch_style(style_default(app.light_theme));

  let paragraph = Paragraph::new(text).block(Block::default());

  let chunks = vertical_chunks_with_margin(vec![Constraint::Min(2), Constraint::Min(2)], area, 1);
  f.render_widget(paragraph, chunks[0]);

  let width = chunks[1].width.max(3) - 3; // keep 2 for borders and 1 for cursor
  let scroll = app.app_input.input.visual_scroll(width as usize);
  let input = Paragraph::new(app.app_input.input.value())
    .style(get_input_style(app))
    .scroll((0, scroll as u16))
    .block(
      Block::default()
        .borders(Borders::ALL)
        .style(get_input_style(app)),
    );

  f.render_widget(input, chunks[1]);

  match app.app_input.input_mode {
    InputMode::Normal => {
      // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
    }

    InputMode::Editing => {
      // Make the cursor visible and ask tui-rs to put it at the specified coordinates after rendering
      f.set_cursor(
        // Put cursor past the end of the input text
        chunks[1].x + ((app.app_input.input.visual_cursor()).max(scroll) - scroll) as u16 + 1,
        // Move one line down, from the border to the input line
        chunks[1].y + 1,
      )
    }
  }
}

fn draw_logo_block<B: Backend>(f: &mut Frame<'_, B>, app: &App, area: Rect) {
  // Banner text with correct styling
  let text = format!(
    "{}\n v{} with ♥ in Rust {}",
    BANNER,
    env!("CARGO_PKG_VERSION"),
    nw_loading_indicator(app.is_loading)
  );
  let mut text = Text::from(text);
  text.patch_style(style_logo(app.light_theme));

  // Contains the banner
  let paragraph = Paragraph::new(text).block(Block::default().borders(Borders::ALL));
  f.render_widget(paragraph, area);
}

fn draw_cli_version_block<B: Backend>(f: &mut Frame<'_, B>, app: &App, area: Rect) {
  let block = layout_block_default(" CLI Info (filter <f>)");
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

    let table = Table::new(rows)
      .block(block)
      .widths(&[Constraint::Percentage(50), Constraint::Percentage(50)]);
    f.render_widget(table, area);
  } else {
    loading(f, block, area, app.is_loading, app.light_theme);
  }
}

fn draw_context_info_block<B: Backend>(f: &mut Frame<'_, B>, app: &App, area: Rect) {
  let chunks = vertical_chunks_with_margin(
    vec![
      Constraint::Length(3),
      Constraint::Min(2),
      Constraint::Min(2),
    ],
    area,
    1,
  );

  let block = layout_block_default(" Context Info (toggle <i>) ");

  f.render_widget(block, area);

  let text = match &app.data.active_context {
    Some(active_context) => {
      vec![
        Line::from(vec![
          Span::styled("Context: ", style_default(app.light_theme)),
          Span::styled(&active_context.name, style_primary(app.light_theme)),
        ]),
        Line::from(vec![
          Span::styled("Cluster: ", style_default(app.light_theme)),
          Span::styled(&active_context.cluster, style_primary(app.light_theme)),
        ]),
        Line::from(vec![
          Span::styled("User: ", style_default(app.light_theme)),
          Span::styled(&active_context.user, style_primary(app.light_theme)),
        ]),
      ]
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
    .gauge_style(style_primary(app.light_theme))
    .line_set(get_gauge_style(app.enhanced_graphics))
    .ratio(limited_ratio)
    .label(Line::from(format!("{:.0}%", ratio * 100.0)));
  f.render_widget(cpu_gauge, chunks[1]);

  let ratio = get_nm_ratio(app.data.node_metrics.as_ref(), |nm| nm.mem_percent);
  let limited_ratio = if ratio > 1f64 { 1f64 } else { ratio };

  let mem_gauge = LineGauge::default()
    .block(Block::default().title("Memory:"))
    .gauge_style(style_primary(app.light_theme))
    .line_set(get_gauge_style(app.enhanced_graphics))
    .ratio(limited_ratio)
    .label(Line::from(format!("{:.0}%", ratio * 100.0)));
  f.render_widget(mem_gauge, chunks[2]);
}

// Utility methods

fn get_input_style(app: &App) -> Style {
  match app.app_input.input_mode {
    InputMode::Normal => style_default(app.light_theme),
    InputMode::Editing => style_secondary(app.light_theme),
  }
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

fn nw_loading_indicator<'a>(loading: bool) -> &'a str {
  if loading {
    "..."
  } else {
    ""
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
