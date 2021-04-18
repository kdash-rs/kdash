use super::super::app::{ActiveBlock, App, NodeMetrics, DEFAULT_KEYBINDING};
use super::super::banner::BANNER;
use super::utils::{
  draw_placeholder, get_gauge_style, horizontal_chunks, layout_block_default,
  layout_block_top_border, loading, style_default, style_failure, style_highlight, style_logo,
  style_primary, style_secondary, table_header_style, vertical_chunks, vertical_chunks_with_margin,
};
use super::HIGHLIGHT;

use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  text::{Span, Spans, Text},
  widgets::{Block, Borders, Cell, LineGauge, Paragraph, Row, Table, Tabs},
  Frame,
};

pub fn draw_overview<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  if app.show_info_bar {
    let chunks = vertical_chunks(vec![Constraint::Length(9), Constraint::Min(10)], area);
    draw_status(f, app, chunks[0]);
    draw_active_context_tabs(f, app, chunks[1]);
  } else {
    draw_active_context_tabs(f, app, area);
  }
}

fn draw_status<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let chunks = horizontal_chunks(
    vec![
      Constraint::Length(30),
      Constraint::Min(10),
      Constraint::Length(40),
      Constraint::Length(30),
    ],
    area,
  );

  draw_cli_status(f, app, chunks[0]);
  draw_context_info(f, app, chunks[1]);
  draw_namespaces(f, app, chunks[2]);
  draw_logo(f, app, chunks[3])
}

fn draw_logo<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  // Banner text with correct styling
  let text = format!(
    "{}\nv{} with ♥ in Rust {}",
    BANNER,
    env!("CARGO_PKG_VERSION"),
    nw_loading_indicator(app.is_loading)
  );
  let mut text = Text::from(text);
  text.patch_style(style_logo());

  // Contains the banner
  let paragraph = Paragraph::new(text).block(Block::default().borders(Borders::ALL));
  f.render_widget(paragraph, area);
}

fn nw_loading_indicator<'a>(loading: bool) -> &'a str {
  if loading {
    "..."
  } else {
    ""
  }
}

fn draw_cli_status<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let block = layout_block_default("CLI Info");
  if !app.data.clis.is_empty() {
    let rows = app.data.clis.iter().map(|s| {
      let style = if s.status {
        style_primary()
      } else {
        style_failure()
      };
      Row::new(vec![
        Cell::from(s.name.as_ref()),
        Cell::from(s.version.as_ref()),
      ])
      .style(style)
    });

    let table = Table::new(rows)
      .block(block)
      .widths(&[Constraint::Percentage(50), Constraint::Percentage(50)]);
    f.render_widget(table, area);
  } else {
    loading(f, block, area, app.is_loading);
  }
}

fn draw_active_context_tabs<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let chunks =
    vertical_chunks_with_margin(vec![Constraint::Length(2), Constraint::Min(0)], area, 1);

  let mut block = layout_block_default("Resources");
  if app.get_current_route().active_block != ActiveBlock::Namespaces {
    block = block.style(style_secondary())
  }

  let titles = app
    .context_tabs
    .titles
    .iter()
    .map(|t| Spans::from(Span::styled(t, style_default(app.light_theme))))
    .collect();
  let tabs = Tabs::new(titles)
    .block(block)
    .highlight_style(style_secondary())
    .select(app.context_tabs.index);

  f.render_widget(tabs, area);
  // render tab content
  match app.context_tabs.index {
    0 => draw_pods(f, app, chunks[1]),
    1 => draw_services(f, app, chunks[1]),
    2 => draw_nodes(f, app, chunks[1]),
    3..=7 => draw_placeholder(f, chunks[1]),
    _ => {}
  };
}

fn draw_context_info<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let chunks = vertical_chunks_with_margin(
    vec![
      Constraint::Length(3),
      Constraint::Min(2),
      Constraint::Min(2),
    ],
    area,
    1,
  );

  let block = layout_block_default("Context Info");

  f.render_widget(block, area);

  let text;
  match &app.data.active_context {
    Some(active_context) => {
      text = vec![
        Spans::from(vec![
          Span::styled("Context: ", style_default(app.light_theme)),
          Span::styled(&active_context.name, style_primary()),
        ]),
        Spans::from(vec![
          Span::styled("Cluster: ", style_default(app.light_theme)),
          Span::styled(&active_context.cluster, style_primary()),
        ]),
        Spans::from(vec![
          Span::styled("User: ", style_default(app.light_theme)),
          Span::styled(&active_context.user, style_primary()),
        ]),
      ];
    }
    None => {
      text = vec![Spans::from(Span::styled(
        "Context information not found",
        style_failure(),
      ))]
    }
  }

  let paragraph = Paragraph::new(text).block(Block::default());
  f.render_widget(paragraph, chunks[0]);

  let cpu_gauge = LineGauge::default()
    .block(Block::default().title("CPU:"))
    .gauge_style(style_primary())
    .line_set(get_gauge_style(app.enhanced_graphics))
    .ratio(get_nm_ratio(app.data.node_metrics.as_ref(), |acc, nm| {
      acc + nm.cpu_percent
    }));
  f.render_widget(cpu_gauge, chunks[1]);

  let mem_gauge = LineGauge::default()
    .block(Block::default().title("Memory:"))
    .gauge_style(style_primary())
    .line_set(get_gauge_style(app.enhanced_graphics))
    .ratio(get_nm_ratio(app.data.node_metrics.as_ref(), |acc, nm| {
      acc + nm.mem_percent
    }));
  f.render_widget(mem_gauge, chunks[2]);
}

fn draw_namespaces<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = format!(
    "Namespaces {} (all: {})",
    DEFAULT_KEYBINDING.jump_to_namespace, DEFAULT_KEYBINDING.select_all_namespace
  );
  let mut block = layout_block_default(title.as_str());

  if app.get_current_route().active_block == ActiveBlock::Namespaces {
    block = block.style(style_secondary())
  }

  if !app.data.namespaces.items.is_empty() {
    let rows = app.data.namespaces.items.iter().map(|s| {
      let style = if Some(s.name.clone()) == app.data.selected_ns {
        style_secondary()
      } else {
        style_primary()
      };
      Row::new(vec![
        Cell::from(s.name.as_ref()),
        Cell::from(s.status.as_ref()),
      ])
      .style(style)
    });

    let table = Table::new(rows)
      .header(table_header_style(vec!["Name", "Status"], app.light_theme))
      .block(block)
      .highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT)
      .widths(&[Constraint::Percentage(80), Constraint::Percentage(20)]);

    f.render_stateful_widget(table, area, &mut app.data.namespaces.state);
  } else {
    loading(f, block, area, app.is_loading);
  }
}

fn draw_pods<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = format!(
    "Pods ({}) [{}]",
    app
      .data
      .selected_ns
      .as_ref()
      .unwrap_or(&String::from("all")),
    app.data.pods.items.len()
  );
  let block = layout_block_top_border(title.as_str());

  if !app.data.pods.items.is_empty() {
    let rows = app.data.pods.items.iter().map(|c| {
      let style = if ["Running", "Completed"].contains(&c.status.as_str()) {
        style_primary()
      } else if [
        "ContainerCreating",
        "PodInitializing",
        "Pending",
        "Initialized",
      ]
      .contains(&c.status.as_str())
      {
        style_secondary()
      } else {
        style_failure()
      };
      Row::new(vec![
        Cell::from(c.namespace.as_ref()),
        Cell::from(c.name.as_ref()),
        Cell::from(c.ready.as_ref()),
        Cell::from(c.status.as_ref()),
        Cell::from(c.restarts.to_string()),
        Cell::from(c.age.as_ref()),
      ])
      .style(style)
    });

    let table = Table::new(rows)
      .header(table_header_style(
        vec!["Namespace", "Name", "Ready", "Status", "Restarts", "Age"],
        app.light_theme,
      ))
      .block(block)
      .highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT)
      .widths(&[
        Constraint::Percentage(25),
        Constraint::Percentage(35),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
      ]);

    f.render_stateful_widget(table, area, &mut app.data.pods.state);
  } else {
    loading(f, block, area, app.is_loading);
  }
}

fn draw_nodes<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = format!("Nodes [{}]", app.data.nodes.items.len());
  let block = layout_block_top_border(title.as_str());

  if !app.data.nodes.items.is_empty() {
    let rows = app.data.nodes.items.iter().map(|c| {
      let style = if c.status != "Ready" {
        style_failure()
      } else {
        style_primary()
      };
      let pods = c.pods.to_string();
      Row::new(vec![
        Cell::from(c.name.as_ref()),
        Cell::from(c.status.as_ref()),
        Cell::from(c.role.as_ref()),
        Cell::from(c.version.as_ref()),
        Cell::from(pods),
        Cell::from(c.cpu.as_ref()),
        Cell::from(c.mem.as_ref()),
        Cell::from(c.cpu_percent.as_ref()),
        Cell::from(c.mem_percent.as_ref()),
        Cell::from(c.cpu_a.as_ref()),
        Cell::from(c.mem_a.as_ref()),
        Cell::from(c.age.as_ref()),
      ])
      .style(style)
    });

    let table = Table::new(rows)
      .header(table_header_style(
        vec![
          "Name", "Status", "Roles", "Version", "Pods", "CPU", "Mem", "CPU %", "Mem %", "CPU/A",
          "Mem/A", "Age",
        ],
        app.light_theme,
      ))
      .block(block)
      .highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT)
      .widths(&[
        Constraint::Percentage(25),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(5),
        Constraint::Percentage(5),
        Constraint::Percentage(5),
        Constraint::Percentage(5),
        Constraint::Percentage(5),
        Constraint::Percentage(5),
        Constraint::Percentage(5),
        Constraint::Percentage(10),
      ]);

    f.render_stateful_widget(table, area, &mut app.data.nodes.state);
  } else {
    loading(f, block, area, app.is_loading);
  }
}

fn draw_services<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = format!(
    "Services ({}) [{}]",
    app
      .data
      .selected_ns
      .as_ref()
      .unwrap_or(&String::from("all")),
    app.data.services.items.len()
  );
  let block = layout_block_top_border(title.as_str());

  if !app.data.services.items.is_empty() {
    let rows = app.data.services.items.iter().map(|c| {
      Row::new(vec![
        Cell::from(c.namespace.as_ref()),
        Cell::from(c.name.as_ref()),
        Cell::from(c.type_.as_ref()),
        Cell::from(c.cluster_ip.as_ref()),
        Cell::from(c.external_ip.as_ref()),
        Cell::from(c.ports.as_ref()),
        Cell::from(c.age.as_ref()),
      ])
      .style(style_primary())
    });

    let table = Table::new(rows)
      .header(table_header_style(
        vec![
          "Namespace",
          "Name",
          "Type",
          "Cluster IP",
          "External IP",
          "Ports",
          "Age",
        ],
        app.light_theme,
      ))
      .block(block)
      .highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT)
      .widths(&[
        Constraint::Percentage(10),
        Constraint::Percentage(25),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(10),
      ]);

    f.render_stateful_widget(table, area, &mut app.data.services.state);
  } else {
    loading(f, block, area, app.is_loading);
  }
}

/// covert percent value from metrics to ratio that gauge can understand
fn get_nm_ratio(node_metrics: &[NodeMetrics], f: fn(a: f64, b: &NodeMetrics) -> f64) -> f64 {
  if !node_metrics.is_empty() {
    let sum = node_metrics.iter().fold(0f64, f);
    (sum / node_metrics.len() as f64) / 100f64
  } else {
    0f64
  }
}
