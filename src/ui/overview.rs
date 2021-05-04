use super::super::app::{
  key_binding::DEFAULT_KEYBINDING, models::StatefulTable, nodes::NodeMetrics, ActiveBlock, App,
};
use super::super::banner::BANNER;
use super::utils::{
  get_gauge_style, horizontal_chunks, layout_block_default, layout_block_top_border, loading,
  style_default, style_failure, style_highlight, style_logo, style_primary, style_secondary,
  table_header_style, title_with_dual_style, vertical_chunks, vertical_chunks_with_margin,
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

static DESCRIBE_AND_YAML_HINT: &str = "| describe <d> | yaml <y>";
static YAML_HINT: &str = "| yaml <y>";
static COPY_HINT: &str = "| copy <c>";
static PODS_HINT: &str = "Pods <esc>";
static NODES_HINT: &str = "Nodes <esc>";
static PODS_TITLE: &str = "Pods";
static SERVICES_TITLE: &str = "Services";
static CONFIG_MAPS_TITLE: &str = "ConfigMaps";
static STFS_TITLE: &str = "StatefulSets";
static REPLICA_SETS_TITLE: &str = "ReplicaSets";
static DEPLOYMENTS_TITLE: &str = "Deployments";
static DESCRIBE_ACTIVE: &str = "-> Describe ";
static YAML_ACTIVE: &str = "-> YAML ";

pub fn draw_overview<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  if app.show_info_bar {
    let chunks = vertical_chunks(vec![Constraint::Length(9), Constraint::Min(10)], area);
    draw_status_block(f, app, chunks[0]);
    draw_resource_tabs(f, app, chunks[1]);
  } else {
    draw_resource_tabs(f, app, area);
  }
}

fn draw_status_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let chunks = horizontal_chunks(
    vec![
      Constraint::Length(30),
      Constraint::Min(10),
      Constraint::Length(40),
      Constraint::Length(30),
    ],
    area,
  );

  draw_cli_version_block(f, app, chunks[0]);
  draw_context_info(f, app, chunks[1]);
  draw_namespaces(f, app, chunks[2]);
  draw_logo_block(f, app, chunks[3])
}

fn draw_logo_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
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

fn draw_cli_version_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
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

fn draw_resource_tabs<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
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
    1 => draw_services_tab(app.get_current_route().active_block, f, app, chunks[1]),
    2 => draw_nodes_tab(app.get_current_route().active_block, f, app, chunks[1]),
    3 => draw_config_maps_tab(app.get_current_route().active_block, f, app, chunks[1]),
    4 => draw_stateful_sets_tab(app.get_current_route().active_block, f, app, chunks[1]),
    5 => draw_replica_sets_tab(app.get_current_route().active_block, f, app, chunks[1]),
    6 => draw_deployments_tab(app.get_current_route().active_block, f, app, chunks[1]),
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
        get_resource_title(app, PODS_TITLE, DESCRIBE_ACTIVE, app.data.pods.items.len()),
        format!("{} | {}", COPY_HINT, PODS_HINT),
        app.light_theme,
      ),
    ),
    ActiveBlock::Yaml => draw_describe(
      f,
      app,
      area,
      title_with_dual_style(
        get_resource_title(app, PODS_TITLE, YAML_ACTIVE, app.data.pods.items.len()),
        format!("{} | {}", COPY_HINT, PODS_HINT),
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
        get_node_title(app, DESCRIBE_ACTIVE),
        format!("{} | {}", COPY_HINT, NODES_HINT),
        app.light_theme,
      ),
    ),
    ActiveBlock::Yaml => draw_describe(
      f,
      app,
      area,
      title_with_dual_style(
        get_node_title(app, YAML_ACTIVE),
        format!("{} | {}", COPY_HINT, NODES_HINT),
        app.light_theme,
      ),
    ),
    ActiveBlock::Namespaces => {
      draw_nodes_tab(app.get_prev_route().active_block, f, app, area);
    }
    _ => draw_nodes(f, app, area),
  };
}

fn draw_services_tab<B: Backend>(block: ActiveBlock, f: &mut Frame<B>, app: &mut App, area: Rect) {
  match block {
    ActiveBlock::Yaml => draw_describe(
      f,
      app,
      area,
      title_with_dual_style(
        get_resource_title(
          app,
          SERVICES_TITLE,
          YAML_ACTIVE,
          app.data.services.items.len(),
        ),
        format!("{} | Services <esc>", COPY_HINT),
        app.light_theme,
      ),
    ),
    ActiveBlock::Namespaces => {
      draw_services_tab(app.get_prev_route().active_block, f, app, area);
    }
    _ => draw_services(f, app, area),
  };
}

fn draw_config_maps_tab<B: Backend>(
  block: ActiveBlock,
  f: &mut Frame<B>,
  app: &mut App,
  area: Rect,
) {
  match block {
    ActiveBlock::Yaml => draw_describe(
      f,
      app,
      area,
      title_with_dual_style(
        get_resource_title(
          app,
          CONFIG_MAPS_TITLE,
          YAML_ACTIVE,
          app.data.config_maps.items.len(),
        ),
        format!("{} | ConfigMaps <esc>", COPY_HINT),
        app.light_theme,
      ),
    ),
    ActiveBlock::Namespaces => {
      draw_config_maps_tab(app.get_prev_route().active_block, f, app, area);
    }
    _ => draw_config_maps(f, app, area),
  };
}

fn draw_stateful_sets_tab<B: Backend>(
  block: ActiveBlock,
  f: &mut Frame<B>,
  app: &mut App,
  area: Rect,
) {
  match block {
    ActiveBlock::Yaml => draw_describe(
      f,
      app,
      area,
      title_with_dual_style(
        get_resource_title(
          app,
          STFS_TITLE,
          YAML_ACTIVE,
          app.data.stateful_sets.items.len(),
        ),
        format!("{} | StatefulSets <esc>", COPY_HINT),
        app.light_theme,
      ),
    ),
    ActiveBlock::Namespaces => {
      draw_stateful_sets_tab(app.get_prev_route().active_block, f, app, area);
    }
    _ => draw_stateful_sets(f, app, area),
  };
}

fn draw_replica_sets_tab<B: Backend>(
  block: ActiveBlock,
  f: &mut Frame<B>,
  app: &mut App,
  area: Rect,
) {
  match block {
    ActiveBlock::Yaml => draw_describe(
      f,
      app,
      area,
      title_with_dual_style(
        get_resource_title(
          app,
          REPLICA_SETS_TITLE,
          YAML_ACTIVE,
          app.data.replica_sets.items.len(),
        ),
        format!("{} | ReplicaSets <esc>", COPY_HINT),
        app.light_theme,
      ),
    ),
    ActiveBlock::Namespaces => {
      draw_replica_sets_tab(app.get_prev_route().active_block, f, app, area);
    }
    _ => draw_replica_sets(f, app, area),
  };
}

fn draw_deployments_tab<B: Backend>(
  block: ActiveBlock,
  f: &mut Frame<B>,
  app: &mut App,
  area: Rect,
) {
  match block {
    ActiveBlock::Yaml => draw_describe(
      f,
      app,
      area,
      title_with_dual_style(
        get_resource_title(
          app,
          DEPLOYMENTS_TITLE,
          YAML_ACTIVE,
          app.data.deployments.items.len(),
        ),
        format!("{} | Deployments <esc>", COPY_HINT),
        app.light_theme,
      ),
    ),
    ActiveBlock::Namespaces => {
      draw_config_maps_tab(app.get_prev_route().active_block, f, app, area);
    }
    _ => draw_deployments(f, app, area),
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
  let title = get_resource_title(app, PODS_TITLE, "", app.data.pods.items.len());

  draw_resource(
    f,
    area,
    ResourceTable {
      title,
      inline_help: format!("| Containers <enter> {}", DESCRIBE_AND_YAML_HINT),
      resource: &mut app.data.pods,
      table_headers: vec!["Namespace", "Name", "Ready", "Status", "Restarts", "Age"],
      column_widths: vec![
        Constraint::Percentage(25),
        Constraint::Percentage(35),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      let style = get_resource_row_style(&c.status.as_str());
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.ready.to_owned()),
        Cell::from(c.status.to_owned()),
        Cell::from(c.restarts.to_string()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style)
    },
    app.light_theme,
    app.is_loading,
  );
}

fn draw_containers<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_container_title(app, app.data.containers.items.len(), "");

  draw_resource(
    f,
    area,
    ResourceTable {
      title,
      inline_help: format!("| Logs <enter> | {}", PODS_HINT),
      resource: &mut app.data.containers,
      table_headers: vec![
        "Name",
        "Image",
        "Ready",
        "State",
        "Restarts",
        "Probes(L/R)",
        "Ports",
        "Age",
      ],
      column_widths: vec![
        Constraint::Percentage(20),
        Constraint::Percentage(30),
        Constraint::Percentage(5),
        Constraint::Percentage(10),
        Constraint::Percentage(5),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      let style = get_resource_row_style(&c.status.as_str());
      Row::new(vec![
        Cell::from(c.name.to_owned()),
        Cell::from(c.image.to_owned()),
        Cell::from(c.ready.to_owned()),
        Cell::from(c.status.to_owned()),
        Cell::from(c.restarts.to_string()),
        Cell::from(format!(
          "{}/{}",
          c.liveliness_probe.to_string(),
          c.readiness_probe.to_string()
        )),
        Cell::from(c.ports.to_owned()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style)
    },
    app.light_theme,
    app.is_loading,
  );
}

fn draw_nodes<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_node_title(app, "");

  draw_resource(
    f,
    area,
    ResourceTable {
      title,
      inline_help: DESCRIBE_AND_YAML_HINT.into(),
      resource: &mut app.data.nodes,
      table_headers: vec![
        "Name", "Status", "Roles", "Version", PODS_TITLE, "CPU", "Mem", "CPU %", "Mem %", "CPU/A",
        "Mem/A", "Age",
      ],
      column_widths: vec![
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
      ],
    },
    |c| {
      let style = if c.status != "Ready" {
        style_failure()
      } else {
        style_primary()
      };
      Row::new(vec![
        Cell::from(c.name.to_owned()),
        Cell::from(c.status.to_owned()),
        Cell::from(c.role.to_owned()),
        Cell::from(c.version.to_owned()),
        Cell::from(c.pods.to_string()),
        Cell::from(c.cpu.to_owned()),
        Cell::from(c.mem.to_owned()),
        Cell::from(c.cpu_percent.to_owned()),
        Cell::from(c.mem_percent.to_owned()),
        Cell::from(c.cpu_a.to_owned()),
        Cell::from(c.mem_a.to_owned()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style)
    },
    app.light_theme,
    app.is_loading,
  );
}

fn draw_services<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, SERVICES_TITLE, "", app.data.services.items.len());

  draw_resource(
    f,
    area,
    ResourceTable {
      title,
      inline_help: YAML_HINT.into(),
      resource: &mut app.data.services,
      table_headers: vec![
        "Namespace",
        "Name",
        "Type",
        "Cluster IP",
        "External IP",
        "Ports",
        "Age",
      ],
      column_widths: vec![
        Constraint::Percentage(10),
        Constraint::Percentage(25),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.type_.to_owned()),
        Cell::from(c.cluster_ip.to_owned()),
        Cell::from(c.external_ip.to_owned()),
        Cell::from(c.ports.to_owned()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary())
    },
    app.light_theme,
    app.is_loading,
  );
}

fn draw_config_maps<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, CONFIG_MAPS_TITLE, "", app.data.config_maps.items.len());

  draw_resource(
    f,
    area,
    ResourceTable {
      title,
      inline_help: YAML_HINT.into(),
      resource: &mut app.data.config_maps,
      table_headers: vec!["Namespace", "Name", "Data", "Age"],
      column_widths: vec![
        Constraint::Percentage(30),
        Constraint::Percentage(40),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.data.len().to_string()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary())
    },
    app.light_theme,
    app.is_loading,
  );
}

fn draw_stateful_sets<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, STFS_TITLE, "", app.data.stateful_sets.items.len());

  draw_resource(
    f,
    area,
    ResourceTable {
      title,
      inline_help: YAML_HINT.into(),
      resource: &mut app.data.stateful_sets,
      table_headers: vec!["Namespace", "Name", "Ready", "Service", "Age"],
      column_widths: vec![
        Constraint::Percentage(25),
        Constraint::Percentage(30),
        Constraint::Percentage(10),
        Constraint::Percentage(25),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.ready.to_owned()),
        Cell::from(c.service.to_owned()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary())
    },
    app.light_theme,
    app.is_loading,
  );
}

fn draw_replica_sets<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_resource_title(
    app,
    REPLICA_SETS_TITLE,
    "",
    app.data.replica_sets.items.len(),
  );

  draw_resource(
    f,
    area,
    ResourceTable {
      title,
      inline_help: YAML_HINT.into(),
      resource: &mut app.data.replica_sets,
      table_headers: vec!["Namespace", "Name", "Desired", "Current", "Ready", "Age"],
      column_widths: vec![
        Constraint::Percentage(25),
        Constraint::Percentage(35),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.desired.to_string()),
        Cell::from(c.current.to_string()),
        Cell::from(c.ready.to_string()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary())
    },
    app.light_theme,
    app.is_loading,
  );
}

fn draw_deployments<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, DEPLOYMENTS_TITLE, "", app.data.deployments.items.len());

  draw_resource(
    f,
    area,
    ResourceTable {
      title,
      inline_help: YAML_HINT.into(),
      resource: &mut app.data.deployments,
      table_headers: vec![
        "Namespace",
        "Name",
        "Ready",
        "Up-to-date",
        "Available",
        "Age",
      ],
      column_widths: vec![
        Constraint::Percentage(25),
        Constraint::Percentage(35),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.ready.to_owned()),
        Cell::from(c.updated.to_string()),
        Cell::from(c.available.to_string()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary())
    },
    app.light_theme,
    app.is_loading,
  );
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

  let block = layout_block_top_border(title);

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
  let block = layout_block_top_border(title);

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

// Utility methods

struct ResourceTable<'a, T> {
  title: String,
  inline_help: String,
  resource: &'a mut StatefulTable<T>,
  table_headers: Vec<&'a str>,
  column_widths: Vec<Constraint>,
}

/// Draw a kubernetes resource i overview tab
fn draw_resource<'a, B, T, F>(
  f: &mut Frame<B>,
  area: Rect,
  table_props: ResourceTable<'a, T>,
  row_cell_mapper: F,
  light_theme: bool,
  is_loading: bool,
) where
  B: Backend,
  F: Fn(&T) -> Row<'a>,
{
  let title = title_with_dual_style(table_props.title, table_props.inline_help, light_theme);
  let block = layout_block_top_border(title);

  if !table_props.resource.items.is_empty() {
    let rows = table_props
      .resource
      .items
      .iter()
      //   .map(|c| { Row::new(row_cell_mapper(c)) }.style(style_primary()));
      .map(row_cell_mapper);

    let table = Table::new(rows)
      .header(table_header_style(table_props.table_headers, light_theme))
      .block(block)
      .highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT)
      .widths(&table_props.column_widths);

    f.render_stateful_widget(table, area, &mut table_props.resource.state);
  } else {
    loading(f, block, area, is_loading);
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

fn get_resource_title<S: AsRef<str>>(app: &App, title: S, suffix: S, items_len: usize) -> String {
  format!(
    "{} {}",
    title_with_ns(
      title.as_ref(),
      app
        .data
        .selected
        .ns
        .as_ref()
        .unwrap_or(&String::from("all")),
      items_len
    ),
    suffix.as_ref(),
  )
}

fn get_container_title<S: AsRef<str>>(app: &App, container_len: usize, suffix: S) -> String {
  let title = get_resource_title(
    app,
    PODS_TITLE,
    format!("-> Containers [{}] {}", container_len, suffix.as_ref()).as_str(),
    app.data.pods.items.len(),
  );
  title
}

fn title_with_ns(title: &str, ns: &str, length: usize) -> String {
  format!("{} (ns: {}) [{}]", title, ns, length)
}

fn nw_loading_indicator<'a>(loading: bool) -> &'a str {
  if loading {
    "..."
  } else {
    ""
  }
}
