use chrono::Utc;
use ratatui::{
  layout::{Constraint, Rect},
  text::{Line, Span, Text},
  widgets::{Block, Borders, Cell, Paragraph, Row, Table},
  Frame,
};

use super::{
  resource_tabs::draw_resource_tabs_block,
  utils::{
    action_hint, gauge_line, help_part, horizontal_chunks, layout_block_default,
    layout_block_default_line, loading, mixed_bold_line, style_caution, style_failure, style_label,
    style_logo, style_primary, style_text, title_with_dual_style, vertical_chunks,
    vertical_chunks_with_margin,
  },
};
use crate::{
  app::{
    key_binding::DEFAULT_KEYBINDING,
    metrics::KubeNodeMetrics,
    models::{AppResource, KubeResource},
    nodes::KubeNode,
    ns::NamespaceResource,
    pods::KubePod,
    utils::to_age,
    ActiveBlock, App,
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
  let hide_logo = app.config.hide_logo;
  let mut constraints = vec![
    Constraint::Length(45),
    Constraint::Min(10),
    Constraint::Length(30),
  ];
  if !hide_logo {
    constraints.push(Constraint::Length(15));
  }
  let chunks = horizontal_chunks(constraints, area);

  NamespaceResource::render(ActiveBlock::Namespaces, f, app, chunks[0]);
  draw_context_info_block(f, app, chunks[1]);
  draw_cli_version_block(f, app, chunks[2]);
  if !hide_logo {
    draw_logo_block(f, app, chunks[3]);
  }
}

fn draw_logo_block(f: &mut Frame<'_>, app: &App, area: Rect) {
  let palette = app.palette;
  // Banner text with correct styling
  let text = Text::from(BANNER);
  let text = text.patch_style(style_logo(palette));
  let block = Block::default()
    .borders(Borders::ALL)
    .border_style(style_primary(palette));
  // Contains the banner
  let paragraph = Paragraph::new(text).block(block);
  f.render_widget(paragraph, area);
}

fn draw_cli_version_block(f: &mut Frame<'_>, app: &App, area: Rect) {
  let block = layout_block_default(" CLI Info ", app.palette);
  if !app.data.clis.is_empty() {
    let rows = app.data.clis.iter().map(|s| {
      let version_style = if s.status {
        style_text(app.palette)
      } else {
        style_failure(app.palette)
      };
      Row::new(vec![
        Cell::from(s.name.to_owned()).style(style_label(app.palette)),
        Cell::from(s.version.to_owned()).style(version_style),
      ])
    });

    let table = Table::new(
      rows,
      [Constraint::Percentage(50), Constraint::Percentage(50)],
    )
    .block(block);
    f.render_widget(table, area);
  } else {
    loading(f, block, area, app.is_loading(), app.palette);
  }
}

fn draw_context_info_block(f: &mut Frame<'_>, app: &App, area: Rect) {
  let chunks = vertical_chunks_with_margin(
    vec![
      Constraint::Length(3),
      Constraint::Length(2),
      Constraint::Min(2),
    ],
    area,
    1,
  );

  let block = layout_block_default_line(
    title_with_dual_style(
      " Context Info ".to_string(),
      mixed_bold_line(
        [help_part(format!(
          "{} ",
          action_hint("toggle", DEFAULT_KEYBINDING.toggle_info.key)
        ))],
        app.palette,
      ),
      app.palette,
    ),
    app.palette,
  );

  f.render_widget(block, area);

  let text = match &app.data.active_context {
    Some(active_context) => {
      if let Some(user) = &active_context.user {
        vec![
          Line::from(vec![
            Span::styled("Context: ", style_label(app.palette)),
            Span::styled(&active_context.name, style_text(app.palette)),
          ]),
          Line::from(vec![
            Span::styled("Cluster: ", style_label(app.palette)),
            Span::styled(&active_context.cluster, style_text(app.palette)),
          ]),
          Line::from(vec![
            Span::styled("User:    ", style_label(app.palette)),
            Span::styled(user, style_text(app.palette)),
          ]),
        ]
      } else {
        vec![
          Line::from(vec![
            Span::styled("Context: ", style_label(app.palette)),
            Span::styled(&active_context.name, style_text(app.palette)),
          ]),
          Line::from(vec![
            Span::styled("Cluster: ", style_label(app.palette)),
            Span::styled(&active_context.cluster, style_text(app.palette)),
          ]),
          Line::from(vec![
            Span::styled("User:    ", style_label(app.palette)),
            Span::styled("<none>", style_text(app.palette)),
          ]),
        ]
      }
    }
    None => {
      vec![Line::from(Span::styled(
        "Context information not found",
        style_failure(app.palette),
      ))]
    }
  };

  let paragraph = Paragraph::new(text).block(Block::default());
  f.render_widget(paragraph, chunks[0]);

  let cpu_pct = get_nm_ratio(app.data.node_metrics.as_ref(), |nm| nm.cpu_percent) * 100.0;
  let mem_pct = get_nm_ratio(app.data.node_metrics.as_ref(), |nm| nm.mem_percent) * 100.0;
  // both gauges share one paragraph so they sit on consecutive lines (no gap),
  // and the percentage is right-padded to 3 digits so the bar end stays put as
  // the value grows from single to triple digits.
  let gauges = vec![
    gauge_line(
      "CPU:     ".into(),
      cpu_pct,
      format!("{cpu_pct:>3.0}%"),
      chunks[1].width,
      app.palette,
      app.enhanced_graphics,
    ),
    gauge_line(
      "Memory:  ".into(),
      mem_pct,
      format!("{mem_pct:>3.0}%"),
      chunks[1].width,
      app.palette,
      app.enhanced_graphics,
    ),
  ];
  f.render_widget(Paragraph::new(gauges), chunks[1]);

  draw_cluster_facts(f, app, chunks[2]);
}

/// Two plain-text fact lines below the gauges: node readiness with cluster
/// uptime, and a pod state breakdown. Values tint amber/red when something is
/// off (NotReady nodes, pending/failed pods) so problems stand out at a glance.
fn draw_cluster_facts(f: &mut Frame<'_>, app: &App, area: Rect) {
  let palette = app.palette;
  let nodes = &app.data.nodes.items;
  let pods = &app.data.pods.items;

  let total_nodes = nodes.len();
  let ready_nodes = nodes.iter().filter(|n| n.status == "Ready").count();
  let node_style = if total_nodes > 0 && ready_nodes < total_nodes {
    style_failure(palette)
  } else {
    style_text(palette)
  };

  let (running, pending, failed) = pod_counts(pods);
  let pod_style = if failed > 0 {
    style_failure(palette)
  } else if pending > 0 {
    style_caution(palette)
  } else {
    style_text(palette)
  };

  let lines = vec![
    Line::from(vec![
      Span::styled("Nodes:   ", style_label(palette)),
      Span::styled(
        node_summary(ready_nodes, total_nodes, oldest_node_age(nodes)),
        node_style,
      ),
    ]),
    Line::from(vec![
      Span::styled("Pods:    ", style_label(palette)),
      Span::styled(pod_summary(running, pending, failed), pod_style),
    ]),
  ];
  f.render_widget(Paragraph::new(lines), area);
}

/// Cluster uptime as the age of the oldest node (earliest creation timestamp).
/// `None` when no node carries a timestamp (e.g. before nodes have loaded).
fn oldest_node_age(nodes: &[KubeNode]) -> Option<String> {
  nodes
    .iter()
    .filter_map(|n| n.get_k8s_obj().metadata.creation_timestamp.as_ref())
    .min_by_key(|t| t.0)
    .map(|t| to_age(Some(t), Utc::now()))
}

/// `(running, pending, failed)` pod counts. "failed" is anything that is not
/// Running, Pending, or a finished job (Completed/Succeeded).
fn pod_counts(pods: &[KubePod]) -> (usize, usize, usize) {
  let mut running = 0;
  let mut pending = 0;
  let mut failed = 0;
  for pod in pods {
    match pod.status.as_str() {
      "Running" => running += 1,
      "Pending" => pending += 1,
      "Completed" | "Succeeded" => {}
      _ => failed += 1,
    }
  }
  (running, pending, failed)
}

/// `16 running · 2 pending · 1 failed` — pending/failed segments appear only
/// when non-zero, so a healthy cluster reads simply `16 running`.
fn pod_summary(running: usize, pending: usize, failed: usize) -> String {
  let mut parts = vec![format!("{running} running")];
  if pending > 0 {
    parts.push(format!("{pending} pending"));
  }
  if failed > 0 {
    parts.push(format!("{failed} failed"));
  }
  parts.join(" · ")
}

/// `1 Ready · 5d3h up` — switches to `R/N Ready` when some nodes are NotReady;
/// appends uptime when known.
fn node_summary(ready: usize, total: usize, uptime: Option<String>) -> String {
  let readiness = if ready == total {
    format!("{total} Ready")
  } else {
    format!("{ready}/{total} Ready")
  };
  match uptime {
    Some(up) => format!("{readiness} · {up} up"),
    None => readiness,
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

  fn pod_with_status(status: &str) -> KubePod {
    let mut pod = KubePod::default();
    pod.status = status.into();
    pod
  }

  #[test]
  fn test_pod_counts_categorises_states() {
    let pods = vec![
      pod_with_status("Running"),
      pod_with_status("Running"),
      pod_with_status("Pending"),
      pod_with_status("Completed"),
      pod_with_status("Succeeded"),
      pod_with_status("ImagePullBackOff"),
      pod_with_status("CrashLoopBackOff"),
    ];
    // 2 running, 1 pending, finished jobs ignored, 2 problem pods counted failed
    assert_eq!(pod_counts(&pods), (2, 1, 2));
    assert_eq!(pod_counts(&[]), (0, 0, 0));
  }

  #[test]
  fn test_pod_summary_hides_zero_segments() {
    assert_eq!(pod_summary(16, 0, 0), "16 running");
    assert_eq!(pod_summary(16, 2, 0), "16 running · 2 pending");
    assert_eq!(pod_summary(16, 2, 1), "16 running · 2 pending · 1 failed");
    assert_eq!(pod_summary(16, 0, 1), "16 running · 1 failed");
  }

  #[test]
  fn test_node_summary_readiness_and_uptime() {
    assert_eq!(node_summary(1, 1, None), "1 Ready");
    assert_eq!(node_summary(3, 3, Some("5d3h".into())), "3 Ready · 5d3h up");
    // a NotReady node switches to the R/N form
    assert_eq!(
      node_summary(2, 3, Some("5d3h".into())),
      "2/3 Ready · 5d3h up"
    );
    assert_eq!(node_summary(0, 1, None), "0/1 Ready");
  }
}
