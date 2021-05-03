use super::super::app::{key_binding::DEFAULT_KEYBINDING, nodes::NodeMetrics, ActiveBlock, App};
use super::super::banner::BANNER;
use super::utils::{
  get_gauge_style, horizontal_chunks, layout_block_default, layout_block_top_border,
  layout_block_top_border_span, loading, style_default, style_failure, style_highlight, style_logo,
  style_primary, style_secondary, table_header_style, title_with_dual_style, vertical_chunks,
  vertical_chunks_with_margin,
};
use super::HIGHLIGHT;

use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  style::Style,
  text::{Span, Spans, Text},
  widgets::{Block, Borders, Cell, LineGauge, Paragraph, Row, Table, Tabs, Wrap},
  Frame,
};

static DESCRIBE_YAML: &str = "| describe <d> | yaml <y>";
static COPY: &str = "| copy <c>";

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
    0 => draw_pods_tab(app.get_current_route().active_block, f, app, chunks[1]),
    1 => draw_services(f, app, chunks[1]),
    2 => draw_nodes_tab(app.get_current_route().active_block, f, app, chunks[1]),
    3 => draw_config_maps(f, app, chunks[1]),
    4 => draw_stateful_sets(f, app, chunks[1]),
    5 => draw_replica_sets(f, app, chunks[1]),
    6 => draw_deployments(f, app, chunks[1]),
    _ => {}
  };
}

fn draw_pods_tab<B: Backend>(block: ActiveBlock, f: &mut Frame<B>, app: &mut App, area: Rect) {
  match block {
    ActiveBlock::Containers => draw_containers(f, app, area),
    ActiveBlock::Describe => draw_describe(
      f,
      app,
      area,
      title_with_dual_style(
        get_pod_title(app, "-> Describe "),
        format!("{} | Pods <esc>", COPY),
        app.light_theme,
      ),
    ),
    ActiveBlock::Yaml => draw_describe(
      f,
      app,
      area,
      title_with_dual_style(
        get_pod_title(app, "-> YAML "),
        format!("{} | Pods <esc>", COPY),
        app.light_theme,
      ),
    ),
    ActiveBlock::Logs => draw_logs(f, app, area),
    ActiveBlock::Namespaces => {
      draw_pods_tab(app.get_prev_route().active_block, f, app, area);
    }
    _ => draw_pods(f, app, area),
  };
}

fn draw_nodes_tab<B: Backend>(block: ActiveBlock, f: &mut Frame<B>, app: &mut App, area: Rect) {
  match block {
    ActiveBlock::Describe => draw_describe(
      f,
      app,
      area,
      title_with_dual_style(
        get_node_title(app, "-> Describe "),
        format!("{} | Nodes <esc>", COPY),
        app.light_theme,
      ),
    ),
    ActiveBlock::Yaml => draw_describe(
      f,
      app,
      area,
      title_with_dual_style(
        get_pod_title(app, "-> YAML "),
        format!("{} | Nodes <esc>", COPY),
        app.light_theme,
      ),
    ),
    ActiveBlock::Namespaces => {
      draw_nodes_tab(app.get_prev_route().active_block, f, app, area);
    }
    _ => draw_nodes(f, app, area),
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

  let block = layout_block_default("Context Info (toggle <i>)");

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
    DEFAULT_KEYBINDING.jump_to_namespace.key, DEFAULT_KEYBINDING.select_all_namespace.key
  );
  let mut block = layout_block_default(title.as_str());

  if app.get_current_route().active_block == ActiveBlock::Namespaces {
    block = block.style(style_secondary())
  }

  if !app.data.namespaces.items.is_empty() {
    let rows = app.data.namespaces.items.iter().map(|s| {
      let style = if Some(s.name.clone()) == app.data.selected.ns {
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
  let title = title_with_dual_style(
    get_pod_title(app, ""),
    format!("| Containers <enter> {}", DESCRIBE_YAML),
    app.light_theme,
  );
  let block = layout_block_top_border_span(title);

  if !app.data.pods.items.is_empty() {
    let rows = app.data.pods.items.iter().map(|c| {
      let style = get_resource_row_style(&c.status.as_str());
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

fn draw_containers<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = title_with_dual_style(
    get_container_title(app, app.data.containers.items.len(), ""),
    "| Logs <enter> | Pods <esc>".into(),
    app.light_theme,
  );

  let block = layout_block_top_border_span(title);

  if !app.data.containers.items.is_empty() {
    let rows = app.data.containers.items.iter().map(|c| {
      let style = get_resource_row_style(&c.status.as_str());
      Row::new(vec![
        Cell::from(c.name.as_ref()),
        Cell::from(c.image.as_ref()),
        Cell::from(c.ready.as_ref()),
        Cell::from(c.status.as_ref()),
        Cell::from(c.restarts.to_string()),
        Cell::from(format!(
          "{}/{}",
          c.liveliness_probe.to_string(),
          c.readiness_probe.to_string()
        )),
        Cell::from(c.ports.as_ref()),
        Cell::from(c.age.as_ref()),
      ])
      .style(style)
    });

    let table = Table::new(rows)
      .header(table_header_style(
        vec![
          "Name",
          "Image",
          "Ready",
          "State",
          "Restarts",
          "Probes(L/R)",
          "Ports",
          "Age",
        ],
        app.light_theme,
      ))
      .block(block)
      .highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT)
      .widths(&[
        Constraint::Percentage(20),
        Constraint::Percentage(30),
        Constraint::Percentage(5),
        Constraint::Percentage(10),
        Constraint::Percentage(5),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
      ]);
    f.render_stateful_widget(table, area, &mut app.data.containers.state);
  } else {
    loading(f, block, area, app.is_loading);
  }
}

fn draw_nodes<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = title_with_dual_style(
    get_node_title(app, ""),
    DESCRIBE_YAML.into(),
    app.light_theme,
  );
  let block = layout_block_top_border_span(title);

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
  let title = title_with_ns(
    "Services",
    app
      .data
      .selected
      .ns
      .as_ref()
      .unwrap_or(&String::from("all")),
    app.data.services.items.len(),
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

fn draw_config_maps<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = title_with_ns(
    "Config Maps",
    app
      .data
      .selected
      .ns
      .as_ref()
      .unwrap_or(&String::from("all")),
    app.data.config_maps.items.len(),
  );
  let block = layout_block_top_border(title.as_str());

  if !app.data.config_maps.items.is_empty() {
    let rows = app.data.config_maps.items.iter().map(|c| {
      Row::new(vec![
        Cell::from(c.namespace.as_ref()),
        Cell::from(c.name.as_ref()),
        Cell::from(c.data.len().to_string()),
        Cell::from(c.age.as_ref()),
      ])
      .style(style_primary())
    });

    let table = Table::new(rows)
      .header(table_header_style(
        vec!["Namespace", "Name", "Data", "Age"],
        app.light_theme,
      ))
      .block(block)
      .highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT)
      .widths(&[
        Constraint::Percentage(30),
        Constraint::Percentage(40),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
      ]);

    f.render_stateful_widget(table, area, &mut app.data.config_maps.state);
  } else {
    loading(f, block, area, app.is_loading);
  }
}

fn draw_stateful_sets<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = title_with_ns(
    "StatefulSets",
    app
      .data
      .selected
      .ns
      .as_ref()
      .unwrap_or(&String::from("all")),
    app.data.stateful_sets.items.len(),
  );
  let block = layout_block_top_border(title.as_str());

  if !app.data.stateful_sets.items.is_empty() {
    let rows = app.data.stateful_sets.items.iter().map(|c| {
      Row::new(vec![
        Cell::from(c.namespace.as_ref()),
        Cell::from(c.name.as_ref()),
        Cell::from(c.ready.as_ref()),
        Cell::from(c.service.as_ref()),
        Cell::from(c.age.as_ref()),
      ])
      .style(style_primary())
    });

    let table = Table::new(rows)
      .header(table_header_style(
        vec!["Namespace", "Name", "Ready", "Service", "Age"],
        app.light_theme,
      ))
      .block(block)
      .highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT)
      .widths(&[
        Constraint::Percentage(25),
        Constraint::Percentage(30),
        Constraint::Percentage(10),
        Constraint::Percentage(25),
        Constraint::Percentage(10),
      ]);

    f.render_stateful_widget(table, area, &mut app.data.stateful_sets.state);
  } else {
    loading(f, block, area, app.is_loading);
  }
}

fn draw_replica_sets<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = title_with_ns(
    "ReplicaSets",
    app
      .data
      .selected
      .ns
      .as_ref()
      .unwrap_or(&String::from("all")),
    app.data.replica_sets.items.len(),
  );
  let block = layout_block_top_border(title.as_str());

  if !app.data.replica_sets.items.is_empty() {
    let rows = app.data.replica_sets.items.iter().map(|c| {
      Row::new(vec![
        Cell::from(c.namespace.as_ref()),
        Cell::from(c.name.as_ref()),
        Cell::from(c.desired.to_string()),
        Cell::from(c.current.to_string()),
        Cell::from(c.ready.to_string()),
        Cell::from(c.age.as_ref()),
      ])
      .style(style_primary())
    });

    let table = Table::new(rows)
      .header(table_header_style(
        vec!["Namespace", "Name", "Desired", "Current", "Ready", "Age"],
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

    f.render_stateful_widget(table, area, &mut app.data.replica_sets.state);
  } else {
    loading(f, block, area, app.is_loading);
  }
}

fn draw_deployments<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = title_with_ns(
    "Deployments",
    app
      .data
      .selected
      .ns
      .as_ref()
      .unwrap_or(&String::from("all")),
    app.data.deployments.items.len(),
  );
  let block = layout_block_top_border(title.as_str());

  if !app.data.deployments.items.is_empty() {
    let rows = app.data.deployments.items.iter().map(|c| {
      Row::new(vec![
        Cell::from(c.namespace.as_ref()),
        Cell::from(c.name.as_ref()),
        Cell::from(c.ready.as_ref()),
        Cell::from(c.updated.to_string()),
        Cell::from(c.available.to_string()),
        Cell::from(c.age.as_ref()),
      ])
      .style(style_primary())
    });

    let table = Table::new(rows)
      .header(table_header_style(
        vec![
          "Namespace",
          "Name",
          "Ready",
          "Up-to-date",
          "Available",
          "Age",
        ],
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

    f.render_stateful_widget(table, area, &mut app.data.deployments.state);
  } else {
    loading(f, block, area, app.is_loading);
  }
}

fn draw_logs<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let selected_container = app.data.selected.container.clone();
  let container_name = selected_container.unwrap_or_default();

  let title = title_with_dual_style(
    get_container_title(
      app,
      app.data.containers.items.len(),
      format!("-> Logs ({}) ", container_name),
    ),
    "| copy <c> | Containers <esc>".into(),
    app.light_theme,
  );

  let block = layout_block_top_border_span(title);

  if container_name == app.data.logs.id {
    app
      .data
      .logs
      .render_list(f, area, block, style_primary(), app.log_auto_scroll);
  } else {
    loading(f, block, area, app.is_loading);
  }
}

fn draw_describe<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect, title: Spans) {
  let block = layout_block_top_border_span(title);

  let txt = &app.data.describe_out.get_txt();
  if !txt.is_empty() {
    let mut txt = Text::from(txt.clone());
    txt.patch_style(style_primary());

    let paragraph = Paragraph::new(txt)
      .block(block)
      .wrap(Wrap { trim: false })
      .scroll((app.data.describe_out.offset, 0));
    f.render_widget(paragraph, area);
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

fn get_resource_row_style(status: &str) -> Style {
  if ["Running", "Completed"].contains(&status) {
    style_primary()
  } else if [
    "ContainerCreating",
    "PodInitializing",
    "Pending",
    "Initialized",
  ]
  .contains(&status)
  {
    style_secondary()
  } else {
    style_failure()
  }
}

fn get_node_title<S: AsRef<str>>(app: &App, suffix: S) -> String {
  format!("Nodes [{}] {}", app.data.nodes.items.len(), suffix.as_ref())
}

fn get_pod_title<S: AsRef<str>>(app: &App, suffix: S) -> String {
  format!(
    "{} {}",
    title_with_ns(
      "Pods",
      app
        .data
        .selected
        .ns
        .as_ref()
        .unwrap_or(&String::from("all")),
      app.data.pods.items.len()
    ),
    suffix.as_ref(),
  )
}

fn get_container_title<S: AsRef<str>>(app: &App, container_len: usize, suffix: S) -> String {
  let title = get_pod_title(
    app,
    format!("-> Containers [{}] {}", container_len, suffix.as_ref()),
  );
  title
}

fn title_with_ns(title: &str, ns: &str, length: usize) -> String {
  format!("{} (ns: {}) [{}]", title, ns, length)
}
