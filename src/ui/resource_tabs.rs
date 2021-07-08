use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  style::Style,
  text::{Span, Spans, Text},
  widgets::{Cell, List, ListItem, Paragraph, Row, Table, Tabs, Wrap},
  Frame,
};

use super::{
  utils::{
    centered_rect, layout_block_default, layout_block_top_border, loading, style_default,
    style_failure, style_highlight, style_primary, style_secondary, style_success,
    table_header_style, title_with_dual_style, vertical_chunks_with_margin,
  },
  HIGHLIGHT,
};
use crate::app::{models::StatefulTable, ActiveBlock, App};

static DESCRIBE_AND_YAML_HINT: &str = "| describe <d> | yaml <y>";
static DESCRIBE_YAML_AND_ESC_HINT: &str = "| describe <d> | yaml <y> | back to menu <esc>";
static COPY_HINT: &str = "| copy <c>";
static NODES_TITLE: &str = "Nodes";
static PODS_TITLE: &str = "Pods";
static SERVICES_TITLE: &str = "Services";
static CONFIG_MAPS_TITLE: &str = "ConfigMaps";
static STFS_TITLE: &str = "StatefulSets";
static REPLICA_SETS_TITLE: &str = "ReplicaSets";
static DEPLOYMENTS_TITLE: &str = "Deployments";
static JOBS_TITLE: &str = "Jobs";
static DAEMON_SETS_TITLE: &str = "DaemonSets";
static CRON_JOBS_TITLE: &str = "Cron Jobs";
static SECRETS_TITLE: &str = "Secrets";
static DESCRIBE_ACTIVE: &str = "-> Describe ";
static YAML_ACTIVE: &str = "-> YAML ";

pub fn draw_resource_tabs_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let chunks =
    vertical_chunks_with_margin(vec![Constraint::Length(2), Constraint::Min(0)], area, 1);

  let mut block = layout_block_default("Resources");
  if app.get_current_route().active_block != ActiveBlock::Namespaces {
    block = block.style(style_secondary())
  }

  let titles = app
    .context_tabs
    .items
    .iter()
    .map(|t| Spans::from(Span::styled(&t.title, style_default(app.light_theme))))
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
    7 => draw_jobs_tab(app.get_current_route().active_block, f, app, chunks[1]),
    8 => draw_daemon_sets_tab(app.get_current_route().active_block, f, app, chunks[1]),
    9 => draw_more(app.get_current_route().active_block, f, app, chunks[1]),
    _ => {}
  };
}

/// more resources tab
fn draw_more<B: Backend>(block: ActiveBlock, f: &mut Frame<B>, app: &mut App, area: Rect) {
  match block {
    // ActiveBlock::More => draw_menu(f, app, area),
    ActiveBlock::CronJobs => draw_cronjobs_tab(block, f, app, area),
    ActiveBlock::Secrets => draw_secrets_tab(block, f, app, area),
    ActiveBlock::Describe | ActiveBlock::Yaml => {
      let mut prev_route = app.get_prev_route();
      if prev_route.active_block == block {
        prev_route = app.get_nth_route_from_last(2);
      }
      match prev_route.active_block {
        ActiveBlock::CronJobs => draw_cronjobs_tab(block, f, app, area),
        ActiveBlock::Secrets => draw_secrets_tab(block, f, app, area),
        _ => { /* do nothing */ }
      }
    }
    ActiveBlock::Namespaces => draw_more(app.get_prev_route().active_block, f, app, area),
    _ => draw_menu(f, app, area),
  }
}

/// more resources menu
fn draw_menu<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let area = centered_rect(50, 15, area);

  let items: Vec<ListItem> = app
    .more_resources_menu
    .items
    .iter()
    .map(|it| ListItem::new(it.0.clone()))
    .collect();
  f.render_stateful_widget(
    List::new(items)
      .block(layout_block_default("Select Resource"))
      .highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT),
    area,
    &mut app.more_resources_menu.state,
  );
}

macro_rules! draw_resource_tab {
  ($title:expr, $block:expr, $f:expr, $app:expr, $area:expr, $fn1:expr, $fn2:expr, $res:expr) => {
    match $block {
      ActiveBlock::Describe | ActiveBlock::Yaml => draw_describe_block(
        $f,
        $app,
        $area,
        title_with_dual_style(
          get_resource_title($app, $title, get_describe_active($block), $res.items.len()),
          format!("{} | {} <esc>", COPY_HINT, $title),
          $app.light_theme,
        ),
      ),
      ActiveBlock::Namespaces => $fn1($app.get_prev_route().active_block, $f, $app, $area),
      _ => $fn2($f, $app, $area),
    };
  };
}

fn draw_pods_tab<B: Backend>(block: ActiveBlock, f: &mut Frame<B>, app: &mut App, area: Rect) {
  match block {
    ActiveBlock::Containers => draw_containers_block(f, app, area),
    ActiveBlock::Describe | ActiveBlock::Yaml => draw_describe_block(
      f,
      app,
      area,
      title_with_dual_style(
        get_resource_title(
          app,
          PODS_TITLE,
          get_describe_active(block),
          app.data.pods.items.len(),
        ),
        format!("{} | {} <esc>", COPY_HINT, PODS_TITLE),
        app.light_theme,
      ),
    ),
    ActiveBlock::Logs => draw_logs_block(f, app, area),
    ActiveBlock::Namespaces => draw_pods_tab(app.get_prev_route().active_block, f, app, area),
    _ => draw_pods_block(f, app, area),
  };
}

fn draw_pods_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, PODS_TITLE, "", app.data.pods.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: format!("| Containers <enter> {}", DESCRIBE_AND_YAML_HINT),
      resource: &mut app.data.pods,
      table_headers: vec!["Namespace", "Name", "Ready", "Status", "Restarts", "Age"],
      column_widths: vec![
        Constraint::Percentage(25),
        // workaround for TUI-RS issue : https://github.com/fdehau/tui-rs/issues/470#issuecomment-852562848
        Constraint::Percentage(34),
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

fn draw_containers_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_container_title(app, app.data.containers.items.len(), "");

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: format!("| Logs <enter> | {} <esc>", PODS_TITLE),
      resource: &mut app.data.containers,
      table_headers: vec![
        "Name",
        "Image",
        "Init",
        "Ready",
        "State",
        "Restarts",
        "Probes(L/R)",
        "Ports",
        "Age",
      ],
      column_widths: vec![
        Constraint::Percentage(20),
        // workaround for TUI-RS issue : https://github.com/fdehau/tui-rs/issues/470#issuecomment-852562848
        Constraint::Percentage(24),
        Constraint::Percentage(5),
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
        Cell::from(c.init.to_string()),
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

fn draw_logs_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
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

fn draw_nodes_tab<B: Backend>(block: ActiveBlock, f: &mut Frame<B>, app: &mut App, area: Rect) {
  match block {
    ActiveBlock::Describe | ActiveBlock::Yaml => draw_describe_block(
      f,
      app,
      area,
      title_with_dual_style(
        get_node_title(app, get_describe_active(block)),
        format!("{} | {} <esc>", COPY_HINT, NODES_TITLE),
        app.light_theme,
      ),
    ),
    ActiveBlock::Namespaces => draw_nodes_tab(app.get_prev_route().active_block, f, app, area),
    _ => draw_nodes_block(f, app, area),
  };
}

fn draw_nodes_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_node_title(app, "");

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_AND_YAML_HINT.into(),
      resource: &mut app.data.nodes,
      table_headers: vec![
        "Name", "Status", "Roles", "Version", PODS_TITLE, "CPU", "Mem", "CPU %", "Mem %", "CPU/A",
        "Mem/A", "Age",
      ],
      column_widths: vec![
        // workaround for TUI-RS issue : https://github.com/fdehau/tui-rs/issues/470#issuecomment-852562848
        Constraint::Percentage(24),
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

fn draw_services_tab<B: Backend>(block: ActiveBlock, f: &mut Frame<B>, app: &mut App, area: Rect) {
  draw_resource_tab!(
    SERVICES_TITLE,
    block,
    f,
    app,
    area,
    draw_services_tab,
    draw_services_block,
    app.data.services
  );
}

fn draw_services_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, SERVICES_TITLE, "", app.data.services.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_AND_YAML_HINT.into(),
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
        // workaround for TUI-RS issue : https://github.com/fdehau/tui-rs/issues/470#issuecomment-852562848
        Constraint::Percentage(10),
        Constraint::Percentage(24),
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

fn draw_config_maps_tab<B: Backend>(
  block: ActiveBlock,
  f: &mut Frame<B>,
  app: &mut App,
  area: Rect,
) {
  draw_resource_tab!(
    CONFIG_MAPS_TITLE,
    block,
    f,
    app,
    area,
    draw_config_maps_tab,
    draw_config_maps_block,
    app.data.config_maps
  );
}

fn draw_config_maps_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, CONFIG_MAPS_TITLE, "", app.data.config_maps.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_AND_YAML_HINT.into(),
      resource: &mut app.data.config_maps,
      table_headers: vec!["Namespace", "Name", "Data", "Age"],
      column_widths: vec![
        Constraint::Percentage(30),
        // workaround for TUI-RS issue : https://github.com/fdehau/tui-rs/issues/470#issuecomment-852562848
        Constraint::Percentage(39),
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

fn draw_stateful_sets_tab<B: Backend>(
  block: ActiveBlock,
  f: &mut Frame<B>,
  app: &mut App,
  area: Rect,
) {
  draw_resource_tab!(
    STFS_TITLE,
    block,
    f,
    app,
    area,
    draw_stateful_sets_tab,
    draw_stateful_sets_block,
    app.data.stateful_sets
  );
}

fn draw_stateful_sets_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, STFS_TITLE, "", app.data.stateful_sets.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_AND_YAML_HINT.into(),
      resource: &mut app.data.stateful_sets,
      table_headers: vec!["Namespace", "Name", "Ready", "Service", "Age"],
      column_widths: vec![
        Constraint::Percentage(25),
        // workaround for TUI-RS issue : https://github.com/fdehau/tui-rs/issues/470#issuecomment-852562848
        Constraint::Percentage(29),
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

fn draw_replica_sets_tab<B: Backend>(
  block: ActiveBlock,
  f: &mut Frame<B>,
  app: &mut App,
  area: Rect,
) {
  draw_resource_tab!(
    REPLICA_SETS_TITLE,
    block,
    f,
    app,
    area,
    draw_replica_sets_tab,
    draw_replica_sets_block,
    app.data.replica_sets
  );
}

fn draw_replica_sets_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_resource_title(
    app,
    REPLICA_SETS_TITLE,
    "",
    app.data.replica_sets.items.len(),
  );

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_AND_YAML_HINT.into(),
      resource: &mut app.data.replica_sets,
      table_headers: vec!["Namespace", "Name", "Desired", "Current", "Ready", "Age"],
      column_widths: vec![
        Constraint::Percentage(25),
        // workaround for TUI-RS issue : https://github.com/fdehau/tui-rs/issues/470#issuecomment-852562848
        Constraint::Percentage(34),
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

fn draw_deployments_tab<B: Backend>(
  block: ActiveBlock,
  f: &mut Frame<B>,
  app: &mut App,
  area: Rect,
) {
  draw_resource_tab!(
    DEPLOYMENTS_TITLE,
    block,
    f,
    app,
    area,
    draw_deployments_tab,
    draw_deployments_block,
    app.data.deployments
  );
}

fn draw_deployments_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, DEPLOYMENTS_TITLE, "", app.data.deployments.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_AND_YAML_HINT.into(),
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
        // workaround for TUI-RS issue : https://github.com/fdehau/tui-rs/issues/470#issuecomment-852562848
        Constraint::Percentage(34),
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

fn draw_jobs_tab<B: Backend>(block: ActiveBlock, f: &mut Frame<B>, app: &mut App, area: Rect) {
  draw_resource_tab!(
    JOBS_TITLE,
    block,
    f,
    app,
    area,
    draw_jobs_tab,
    draw_jobs_block,
    app.data.jobs
  );
}

fn draw_jobs_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, JOBS_TITLE, "", app.data.jobs.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_AND_YAML_HINT.into(),
      resource: &mut app.data.jobs,
      table_headers: vec!["Namespace", "Name", "Completions", "Duration", "Age"],
      column_widths: vec![
        Constraint::Percentage(25),
        // workaround for TUI-RS issue : https://github.com/fdehau/tui-rs/issues/470#issuecomment-852562848
        Constraint::Percentage(39),
        Constraint::Percentage(15),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.completions.to_owned()),
        Cell::from(c.duration.to_string()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary())
    },
    app.light_theme,
    app.is_loading,
  );
}

fn draw_daemon_sets_tab<B: Backend>(
  block: ActiveBlock,
  f: &mut Frame<B>,
  app: &mut App,
  area: Rect,
) {
  draw_resource_tab!(
    DAEMON_SETS_TITLE,
    block,
    f,
    app,
    area,
    draw_daemon_sets_tab,
    draw_daemon_sets_block,
    app.data.daemon_sets
  );
}

fn draw_daemon_sets_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, DAEMON_SETS_TITLE, "", app.data.daemon_sets.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_AND_YAML_HINT.into(),
      resource: &mut app.data.daemon_sets,
      table_headers: vec![
        "Namespace",
        "Name",
        "Desired",
        "Current",
        "Ready",
        "Up-to-date",
        "Available",
        "Age",
      ],
      column_widths: vec![
        // workaround for TUI-RS issue : https://github.com/fdehau/tui-rs/issues/470#issuecomment-852562848
        Constraint::Percentage(19),
        Constraint::Percentage(20),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
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
        Cell::from(c.up_to_date.to_string()),
        Cell::from(c.available.to_string()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary())
    },
    app.light_theme,
    app.is_loading,
  );
}

fn draw_cronjobs_tab<B: Backend>(block: ActiveBlock, f: &mut Frame<B>, app: &mut App, area: Rect) {
  draw_resource_tab!(
    CRON_JOBS_TITLE,
    block,
    f,
    app,
    area,
    draw_cronjobs_tab,
    draw_cronjobs_block,
    app.data.cronjobs
  );
}

fn draw_cronjobs_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, CRON_JOBS_TITLE, "", app.data.cronjobs.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_YAML_AND_ESC_HINT.into(),
      resource: &mut app.data.cronjobs,
      table_headers: vec![
        "Namespace",
        "Name",
        "Schedule",
        "Last Scheduled",
        "Suspend",
        "Active",
        "Age",
      ],
      column_widths: vec![
        Constraint::Percentage(20),
        // workaround for TUI-RS issue : https://github.com/fdehau/tui-rs/issues/470#issuecomment-852562848
        Constraint::Percentage(24),
        Constraint::Percentage(15),
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
        Cell::from(c.schedule.to_owned()),
        Cell::from(c.last_schedule.to_string()),
        Cell::from(c.suspend.to_string()),
        Cell::from(c.active.to_string()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary())
    },
    app.light_theme,
    app.is_loading,
  );
}

fn draw_secrets_tab<B: Backend>(block: ActiveBlock, f: &mut Frame<B>, app: &mut App, area: Rect) {
  draw_resource_tab!(
    SECRETS_TITLE,
    block,
    f,
    app,
    area,
    draw_secrets_tab,
    draw_secrets_block,
    app.data.secrets
  );
}

fn draw_secrets_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, SECRETS_TITLE, "", app.data.secrets.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_YAML_AND_ESC_HINT.into(),
      resource: &mut app.data.secrets,
      table_headers: vec!["Namespace", "Name", "Type", "Data", "Age"],
      column_widths: vec![
        Constraint::Percentage(25),
        // workaround for TUI-RS issue : https://github.com/fdehau/tui-rs/issues/470#issuecomment-852562848
        Constraint::Percentage(29),
        Constraint::Percentage(25),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.type_.to_owned()),
        Cell::from(c.data.len().to_string()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary())
    },
    app.light_theme,
    app.is_loading,
  );
}

/// common for all resources
fn draw_describe_block<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect, title: Spans) {
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

struct ResourceTableProps<'a, T> {
  title: String,
  inline_help: String,
  resource: &'a mut StatefulTable<T>,
  table_headers: Vec<&'a str>,
  column_widths: Vec<Constraint>,
}

/// Draw a kubernetes resource i overview tab
fn draw_resource_block<'a, B, T, F>(
  f: &mut Frame<B>,
  area: Rect,
  table_props: ResourceTableProps<'a, T>,
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

fn get_resource_row_style(status: &str) -> Style {
  if status == "Running" {
    style_primary()
  } else if status == "Completed" {
    style_success()
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
  format!(
    "{} [{}] {}",
    NODES_TITLE,
    app.data.nodes.items.len(),
    suffix.as_ref()
  )
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

fn get_describe_active<'a>(block: ActiveBlock) -> &'a str {
  match block {
    ActiveBlock::Describe => DESCRIBE_ACTIVE,
    _ => YAML_ACTIVE,
  }
}

#[cfg(test)]
mod tests {
  use tui::{
    backend::TestBackend,
    buffer::Buffer,
    style::{Color, Modifier},
    Terminal,
  };

  use super::*;
  use crate::app::pods::KubePod;

  #[test]
  fn test_draw_resource_tabs_block() {
    let backend = TestBackend::new(100, 7);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
      .draw(|f| {
        let size = f.size();
        let mut app = App::default();
        let mut pod = KubePod::default();
        pod.name = "pod name test".into();
        pod.namespace = "pod namespace test".into();
        pod.ready = "0/2".into();
        pod.status = "Failed".into();
        pod.age = "6h52m".into();
        app.data.pods.set_items(vec![pod]);
        draw_resource_tabs_block(f, &mut app, size);
      })
      .unwrap();

    let mut expected = Buffer::with_lines(vec![
        "┌Resources─────────────────────────────────────────────────────────────────────────────────────────┐",
        "│ Pods <1> │ Services <2> │ Nodes <3> │ ConfigMaps <4> │ StatefulSets <5> │ ReplicaSets <6> │ Deplo│",
        "│                                                                                                  │",
        "│Pods (ns: all) [1] | Containers <enter> | describe <d> | yaml <y>─────────────────────────────────│",
        "│   Namespace               Name                            Ready     Status    Restarts  Age      │",
        "│=> pod namespace test      pod name test                   0/2       Failed    0         6h52m    │",
        "└──────────────────────────────────────────────────────────────────────────────────────────────────┘",
      ]);
    // set row styles
    // First row heading style
    for col in 0..=99 {
      match col {
        0 | 10..=99 => {
          expected
            .get_mut(col, 0)
            .set_style(Style::default().fg(Color::Yellow));
        }
        _ => {
          expected.get_mut(col, 0).set_style(
            Style::default()
              .fg(Color::Yellow)
              .add_modifier(Modifier::BOLD),
          );
        }
      }
    }
    // second row tab headings
    for col in 0..=99 {
      match col {
        0..=12 | 25..=27 | 37..=39 | 54..=56 | 73..=75 | 91..=93 | 99 => {
          expected
            .get_mut(col, 1)
            .set_style(Style::default().fg(Color::Yellow));
        }
        _ => {
          expected
            .get_mut(col, 1)
            .set_style(Style::default().fg(Color::White));
        }
      }
    }
    // third empty row
    for col in 0..=99 {
      expected
        .get_mut(col, 2)
        .set_style(Style::default().fg(Color::Yellow));
    }

    // fourth row tab header style
    for col in 0..=99 {
      match col {
        0 | 66..=99 => {
          expected
            .get_mut(col, 3)
            .set_style(Style::default().fg(Color::Yellow));
        }
        1..=19 => {
          expected.get_mut(col, 3).set_style(
            Style::default()
              .fg(Color::Yellow)
              .add_modifier(Modifier::BOLD),
          );
        }
        _ => {
          expected.get_mut(col, 3).set_style(
            Style::default()
              .fg(Color::White)
              .add_modifier(Modifier::BOLD),
          );
        }
      }
    }
    // table header row
    for col in 0..=99 {
      match col {
        1..=98 => {
          expected
            .get_mut(col, 4)
            .set_style(Style::default().fg(Color::White));
        }
        _ => {
          expected
            .get_mut(col, 4)
            .set_style(Style::default().fg(Color::Yellow));
        }
      }
    }
    // first table data row style
    for col in 0..=99 {
      match col {
        1..=98 => {
          expected.get_mut(col, 5).set_style(
            Style::default()
              .fg(Color::Red)
              .add_modifier(Modifier::REVERSED),
          );
        }
        _ => {
          expected
            .get_mut(col, 5)
            .set_style(Style::default().fg(Color::Yellow));
        }
      }
    }

    // last row
    for col in 0..=99 {
      expected
        .get_mut(col, 6)
        .set_style(Style::default().fg(Color::Yellow));
    }

    terminal.backend().assert_buffer(&expected);
  }

  #[test]
  fn test_draw_resource_block() {
    let backend = TestBackend::new(100, 6);
    let mut terminal = Terminal::new(backend).unwrap();

    struct RenderTest {
      pub name: String,
      pub namespace: String,
      pub data: i32,
      pub age: String,
    }
    terminal
      .draw(|f| {
        let size = f.size();
        let mut resource: StatefulTable<RenderTest> = StatefulTable::new();
        resource.set_items(vec![
          RenderTest {
            name: "Test 1".into(),
            namespace: "Test ns".into(),
            age: "65h3m".into(),
            data: 5,
          },
          RenderTest {
            name: "Test long name that should be truncated from view".into(),
            namespace: "Test ns".into(),
            age: "65h3m".into(),
            data: 3,
          },
          RenderTest {
            name: "test_long_name_that_should_be_truncated_from_view".into(),
            namespace: "Test ns long value check that should be truncated".into(),
            age: "65h3m".into(),
            data: 6,
          },
        ]);
        draw_resource_block(
          f,
          size,
          ResourceTableProps {
            title: "Test".into(),
            inline_help: "-> yaml <y>".into(),
            resource: &mut resource,
            table_headers: vec!["Namespace", "Name", "Data", "Age"],
            column_widths: vec![
              Constraint::Percentage(30),
              // workaround for TUI-RS issue : https://github.com/fdehau/tui-rs/issues/470#issuecomment-852562848
              Constraint::Percentage(39),
              Constraint::Percentage(15),
              Constraint::Percentage(15),
            ],
          },
          |c| {
            Row::new(vec![
              Cell::from(c.namespace.to_owned()),
              Cell::from(c.name.to_owned()),
              Cell::from(c.data.to_string()),
              Cell::from(c.age.to_owned()),
            ])
            .style(style_primary())
          },
          false,
          false,
        );
      })
      .unwrap();

    let mut expected = Buffer::with_lines(vec![
        "Test-> yaml <y>─────────────────────────────────────────────────────────────────────────────────────",
        "   Namespace                    Name                                  Data           Age            ",
        "=> Test ns                      Test 1                                5              65h3m          ",
        "   Test ns                      Test long name that should be truncat 3              65h3m          ",
        "   Test ns long value check tha test_long_name_that_should_be_truncat 6              65h3m          ",
        "                                                                                                    ",
      ]);
    // set row styles
    // First row heading style
    for col in 0..=99 {
      match col {
        0..=3 => {
          expected.get_mut(col, 0).set_style(
            Style::default()
              .fg(Color::Yellow)
              .add_modifier(Modifier::BOLD),
          );
        }
        4..=14 => {
          expected.get_mut(col, 0).set_style(
            Style::default()
              .fg(Color::White)
              .add_modifier(Modifier::BOLD),
          );
        }
        _ => {}
      }
    }

    // Second row table header style
    for col in 0..=99 {
      expected
        .get_mut(col, 1)
        .set_style(Style::default().fg(Color::White));
    }
    // first table data row style
    for col in 0..=99 {
      expected.get_mut(col, 2).set_style(
        Style::default()
          .fg(Color::Cyan)
          .add_modifier(Modifier::REVERSED),
      );
    }
    // remaining table data row style
    for row in 3..=4 {
      for col in 0..=99 {
        expected
          .get_mut(col, row)
          .set_style(Style::default().fg(Color::Cyan));
      }
    }

    terminal.backend().assert_buffer(&expected);
  }

  #[test]
  fn test_get_node_title() {
    let app = App::default();
    assert_eq!(get_node_title(&app, "-> hello"), "Nodes [0] -> hello");
  }

  #[test]
  fn test_get_resource_title() {
    let app = App::default();
    assert_eq!(
      get_resource_title(&app, "Title", "-> hello", 5),
      "Title (ns: all) [5] -> hello"
    );
  }

  #[test]
  fn test_get_container_title() {
    let app = App::default();
    assert_eq!(
      get_container_title(&app, 3, "hello"),
      "Pods (ns: all) [0] -> Containers [3] hello"
    );
  }

  #[test]
  fn test_title_with_ns() {
    assert_eq!(title_with_ns("Title", "hello", 3), "Title (ns: hello) [3]");
  }
}
