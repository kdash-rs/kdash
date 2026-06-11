use anyhow::anyhow;
use async_trait::async_trait;
use k8s_openapi::api::core::v1::{Node, Pod};
use kube::{
  api::{ListParams, ObjectMeta},
  Api,
};
use kubectl_view_allocations::{
  extract_allocatable_from_nodes, extract_allocatable_from_pods,
  extract_utilizations_from_pod_metrics, make_qualifiers,
  metrics::{PodMetrics, Usage},
  qty::Qty,
  tree::provide_prefix,
  QtyByQualifier, Resource, UsedMode,
};
use ratatui::{
  layout::{Constraint, Rect},
  text::{Line, Span},
  widgets::{Cell, LineGauge, Paragraph, Row, Table},
  Frame,
};
use serde::{Deserialize, Serialize};
use tokio::sync::MutexGuard;

use super::{models::AppResource, utils, ActiveBlock, App};
use crate::app::{key_binding::DEFAULT_KEYBINDING, models::FilterableTable};
use crate::{
  network::Network,
  ui::utils::{
    action_hint, default_part, filter_cursor_position, filter_status_parts, gauge_fill_style,
    get_gauge_symbol, help_part, horizontal_chunks, layout_block_active_span, layout_block_default,
    loading, mixed_bold_line, style_caution, style_highlight, style_label, style_success,
    style_text, table_header_style, text_matches_filter, title_with_dual_style, vertical_chunks,
  },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetrics {
  metadata: kube::api::ObjectMeta,
  usage: Usage,
  timestamp: String,
  window: String,
}

// custom impl since metrics API doesn't exist on kube-rs
impl k8s_openapi::Resource for NodeMetrics {
  const GROUP: &'static str = "metrics.k8s.io";
  const KIND: &'static str = "node";
  const VERSION: &'static str = "v1beta1";
  const API_VERSION: &'static str = "metrics.k8s.io/v1beta1";
  const URL_PATH_SEGMENT: &'static str = "nodes";
  type Scope = k8s_openapi::ClusterResourceScope;
}

impl k8s_openapi::Metadata for NodeMetrics {
  type Ty = ObjectMeta;

  fn metadata(&self) -> &Self::Ty {
    &self.metadata
  }

  fn metadata_mut(&mut self) -> &mut Self::Ty {
    &mut self.metadata
  }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct KubeNodeMetrics {
  pub name: String,
  pub cpu: String,
  pub cpu_percent: f64,
  pub mem: String,
  pub mem_percent: f64,
}

impl KubeNodeMetrics {
  pub fn from_api(metric: &NodeMetrics, app: &MutexGuard<'_, App>) -> Self {
    let name = metric.metadata.name.clone().unwrap_or_default();

    let (cpu_percent, mem_percent) = match app.data.node_metrics.iter().find(|it| it.name == name) {
      Some(nm) => (nm.cpu_percent, nm.mem_percent),
      None => (0f64, 0f64),
    };

    KubeNodeMetrics {
      name,
      cpu: utils::cpu_to_milli(metric.usage.cpu.trim_matches('"').to_owned()),
      mem: utils::mem_to_mi(metric.usage.memory.trim_matches('"').to_owned()),
      cpu_percent,
      mem_percent,
    }
  }
}

pub struct UtilizationResource {}

#[async_trait]
impl AppResource for UtilizationResource {
  fn render(_block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let left_title = format!(
      " Resource Utilization (ns: [{}]) [{}] ",
      app
        .data
        .selected
        .ns
        .as_ref()
        .unwrap_or(&String::from("all")),
      app.data.metrics.count_label(),
    );
    let group_by_value = format!(": {:?}", app.utilization_group_by);
    let title = title_with_dual_style(
      left_title.clone(),
      {
        let mut parts =
          filter_status_parts(&app.data.metrics.filter, app.data.metrics.filter_active);
        if !app.data.metrics.filter_active {
          parts.push(help_part(" · ".to_string()));
          parts.push(help_part(action_hint(
            "group by",
            DEFAULT_KEYBINDING.cycle_group_by.key,
          )));
          parts.push(default_part(group_by_value.clone()));
          parts.push(default_part(" ".to_string()));
        }
        mixed_bold_line(parts, app.palette)
      },
      app.palette,
    );
    let block = layout_block_active_span(title, app.palette);

    // Carve out a summary pane above the table when there is data and room:
    // one gauge row per top-level resource kind plus the borders, keeping at
    // least a few lines for the table itself.
    let summary = summary_rows(&app.data.metrics.items);
    let summary_height = summary.len() as u16 + 2;
    let table_area = if !summary.is_empty() && area.height >= summary_height + 8 {
      let chunks = vertical_chunks(
        vec![Constraint::Length(summary_height), Constraint::Min(0)],
        area,
      );
      draw_utilization_summary(f, app, chunks[0], &summary);
      chunks[1]
    } else {
      area
    };

    if !app.data.metrics.items.is_empty() {
      let data = &app.data.metrics.items;
      let filter = app.data.metrics.filter.to_lowercase();
      let has_filter = !filter.is_empty();

      let prefixes = provide_prefix(data, |parent, item| parent.0.len() + 1 == item.0.len());

      // Create the table
      let mut filtered_indices: Vec<usize> = Vec::new();
      let mut rows: Vec<Row<'_>> = vec![];
      for (idx, ((k, oqtys), prefix)) in data.iter().zip(prefixes.iter()).enumerate() {
        if !utilization_matches_filter(&filter, k) {
          continue;
        }
        let column0 = format!(
          "{} {}",
          prefix,
          k.last().map(|x| x.as_str()).unwrap_or("???")
        );
        if let Some(qtys) = oqtys {
          let style = if qtys.requested > qtys.limit || qtys.utilization > qtys.limit {
            style_caution(app.palette)
          } else if is_empty(&qtys.requested) || is_empty(&qtys.limit) {
            style_text(app.palette)
          } else {
            style_success(app.palette)
          };

          let row = Row::new(vec![
            Cell::from(column0),
            make_table_cell(&qtys.utilization, &qtys.allocatable),
            make_table_cell(&qtys.requested, &qtys.allocatable),
            make_table_cell(&qtys.limit, &qtys.allocatable),
            make_table_cell(&qtys.allocatable, &None),
            make_table_cell(&qtys.calc_free(UsedMode::default()), &None),
          ])
          .style(style);
          rows.push(row);
          if has_filter {
            filtered_indices.push(idx);
          }
        }
      }

      if has_filter {
        let max = filtered_indices.len().saturating_sub(1);
        if let Some(sel) = app.data.metrics.state.selected() {
          if sel > max {
            app.data.metrics.state.select(Some(max));
          }
        }
      }
      app.data.metrics.filtered_indices = filtered_indices;

      let table = Table::new(
        rows,
        [
          Constraint::Percentage(50),
          Constraint::Percentage(10),
          Constraint::Percentage(10),
          Constraint::Percentage(10),
          Constraint::Percentage(10),
          Constraint::Percentage(10),
        ],
      )
      .header(table_header_style(
        vec![
          "Resource",
          "Utilization",
          "Requested",
          "Limit",
          "Allocatable",
          "Free",
        ],
        app.palette,
      ))
      .block(block)
      .row_highlight_style(style_highlight());

      f.render_stateful_widget(table, table_area, &mut app.data.metrics.state);
    } else {
      loading(f, block, table_area, app.is_loading(), app.palette);
    }

    if app.data.metrics.filter_active {
      f.set_cursor_position(filter_cursor_position(
        table_area,
        left_title.chars().count() + 1,
        &app.data.metrics.filter,
      ));
    }
  }

  async fn get_resource(nw: &Network<'_>) {
    let mut resources: Vec<Resource> = vec![];

    // extract_allocatable_from_pods skips pods whose node is not listed, so
    // collect every node name here — otherwise no requests/limits show up.
    let mut node_names: Vec<String> = vec![];

    let node_api: Api<Node> = Api::all(nw.client.clone());
    match node_api.list(&ListParams::default()).await {
      Ok(node_list) => {
        node_names = node_list
          .items
          .iter()
          .filter_map(|node| node.metadata.name.clone())
          .collect();
        if let Err(e) = extract_allocatable_from_nodes(node_list.items, &mut resources).await {
          nw.handle_error(anyhow!("Failed to extract node allocation metrics. {}", e))
            .await;
        }
      }
      Err(e) => {
        nw.handle_error(anyhow!("Failed to extract node allocation metrics. {}", e))
          .await
      }
    }

    let pod_api: Api<Pod> = nw.get_namespaced_api().await;
    match pod_api.list(&ListParams::default()).await {
      Ok(pod_list) => {
        if let Err(e) =
          extract_allocatable_from_pods(pod_list.items, &mut resources, &node_names).await
        {
          nw.handle_error(anyhow!("Failed to extract pod allocation metrics. {}", e))
            .await;
        }
      }
      Err(e) => {
        nw.handle_error(anyhow!("Failed to extract pod allocation metrics. {}", e))
          .await
      }
    }

    let api_pod_metrics: Api<PodMetrics> = Api::all(nw.client.clone());

    match api_pod_metrics
    .list(&ListParams::default())
    .await
    {
      Ok(pod_metrics) => {
        if let Err(e) = extract_utilizations_from_pod_metrics(pod_metrics, &mut resources).await {
          nw.handle_error(anyhow!("Failed to extract pod utilization metrics. {}", e)).await;
        }
      }
      Err(_e) => nw.handle_error(anyhow!("Failed to extract pod utilization metrics. Make sure you have a metrics-server deployed on your cluster.")).await,
    };

    let mut app = nw.app.lock().await;

    let data = make_qualifiers(&resources, &app.utilization_group_by, &[]);

    app.data.metrics.set_items(data);
  }
}

#[derive(Debug, PartialEq)]
struct UtilizationSummaryRow {
  name: String,
  utilization: f64,
  requested: f64,
  limit: f64,
}

const SUMMARY_MAX_ROWS: usize = 4;

/// Cluster-wide percentages (vs allocatable) for each top-level resource kind
/// (the first group-by level is always `resource`). cpu and memory come first,
/// the rest keep data order, capped at [`SUMMARY_MAX_ROWS`].
fn summary_rows(data: &[(Vec<String>, Option<QtyByQualifier>)]) -> Vec<UtilizationSummaryRow> {
  let pct = |oqty: &Option<Qty>, alloc: &Qty| -> f64 {
    oqty
      .as_ref()
      .map(|qty| qty.calc_percentage(alloc))
      .unwrap_or(0f64)
  };

  let mut rows: Vec<UtilizationSummaryRow> = data
    .iter()
    .filter(|(k, _)| k.len() == 1)
    .filter_map(|(k, oqtys)| {
      let qtys = oqtys.as_ref()?;
      let alloc = qtys.allocatable.as_ref().filter(|qty| !qty.is_zero())?;
      Some(UtilizationSummaryRow {
        name: k[0].clone(),
        utilization: pct(&qtys.utilization, alloc),
        requested: pct(&qtys.requested, alloc),
        limit: pct(&qtys.limit, alloc),
      })
    })
    .collect();

  rows.sort_by_key(|row| match row.name.as_str() {
    "cpu" => 0,
    "memory" => 1,
    _ => 2,
  });
  rows.truncate(SUMMARY_MAX_ROWS);
  rows
}

fn draw_utilization_summary(
  f: &mut Frame<'_>,
  app: &App,
  area: Rect,
  rows: &[UtilizationSummaryRow],
) {
  let block = layout_block_default(" Cluster Summary (% of allocatable) ", app.palette);
  let inner = block.inner(area);
  f.render_widget(block, area);

  let chunks = vertical_chunks(vec![Constraint::Length(1); rows.len()], inner);
  let name_width = rows
    .iter()
    .map(|row| row.name.chars().count())
    .max()
    .unwrap_or(0)
    .min(24) as u16
    + 2;

  for (row, rect) in rows.iter().zip(chunks.iter()) {
    let cols = horizontal_chunks(
      vec![
        Constraint::Length(name_width),
        Constraint::Fill(1),
        Constraint::Length(2),
        Constraint::Fill(1),
        Constraint::Length(2),
        Constraint::Fill(1),
        Constraint::Length(1),
      ],
      *rect,
    );

    f.render_widget(
      Paragraph::new(Span::styled(row.name.clone(), style_label(app.palette))),
      cols[0],
    );
    for (label, pct, col) in [
      ("Util", row.utilization, cols[1]),
      ("Req", row.requested, cols[3]),
      ("Lim", row.limit, cols[5]),
    ] {
      draw_summary_gauge(f, app, col, label, pct);
    }
  }
}

fn draw_summary_gauge(f: &mut Frame<'_>, app: &App, area: Rect, label: &str, pct: f64) {
  let pct = if pct.is_finite() { pct } else { 0f64 };
  let ratio = (pct / 100f64).clamp(0f64, 1f64);
  let gauge = LineGauge::default()
    .filled_style(gauge_fill_style(ratio, app.palette))
    .filled_symbol(get_gauge_symbol(app.enhanced_graphics))
    .unfilled_symbol(get_gauge_symbol(app.enhanced_graphics))
    .ratio(ratio)
    .label(Line::from(vec![
      Span::styled(format!("{label} "), style_label(app.palette)),
      Span::styled(format!("{pct:>4.0}% "), style_text(app.palette)),
    ]));
  f.render_widget(gauge, area);
}

fn utilization_matches_filter(filter: &str, qualifiers: &[String]) -> bool {
  filter.is_empty()
    || qualifiers
      .iter()
      .any(|part| text_matches_filter(filter, part))
    || text_matches_filter(filter, &qualifiers.join(" "))
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

#[cfg(test)]
mod tests {
  use tokio::sync::Mutex;

  use super::*;
  use crate::app::test_utils::load_resource_from_file;

  #[tokio::test]
  async fn test_kube_node_metrics_from_api() {
    let node_metrics = load_resource_from_file("node_metrics");
    assert_eq!(node_metrics.items.len(), 2);

    let mut app = App::default();
    app.data.node_metrics = vec![KubeNodeMetrics {
      name: "k3d-my-kdash-cluster-server-0".into(),
      cpu: "".into(),
      cpu_percent: 10f64,
      mem: "".into(),
      mem_percent: 20f64,
    }];
    let app = Mutex::new(app);
    let app = app.lock().await;

    let metrics = node_metrics
      .iter()
      .map(|it| KubeNodeMetrics::from_api(it, &app))
      .collect::<Vec<_>>();
    assert_eq!(metrics.len(), 2);
    assert_eq!(
      metrics[0],
      KubeNodeMetrics {
        name: "k3d-my-kdash-cluster-server-0".into(),
        cpu: "162m".into(),
        cpu_percent: 10f64,
        mem: "569Mi".into(),
        mem_percent: 20f64,
      }
    );
    assert_eq!(
      metrics[1],
      KubeNodeMetrics {
        name: "k3d-my-kdash-cluster-server-1".into(),
        cpu: "102m".into(),
        cpu_percent: 0f64,
        mem: "276Mi".into(),
        mem_percent: 0f64,
      }
    );
  }

  #[test]
  fn test_summary_rows() {
    use std::str::FromStr;

    fn qtys(
      utilization: &str,
      requested: &str,
      limit: &str,
      allocatable: Option<&str>,
    ) -> Option<QtyByQualifier> {
      Some(QtyByQualifier {
        utilization: Some(Qty::from_str(utilization).unwrap()),
        requested: Some(Qty::from_str(requested).unwrap()),
        limit: Some(Qty::from_str(limit).unwrap()),
        allocatable: allocatable.map(|qty| Qty::from_str(qty).unwrap()),
        ..QtyByQualifier::default()
      })
    }

    let data = vec![
      (
        vec!["memory".to_string()],
        qtys("2Gi", "4Gi", "8Gi", Some("16Gi")),
      ),
      // children are ignored, only top-level rows are summarised
      (
        vec!["memory".to_string(), "node-1".to_string()],
        qtys("2Gi", "4Gi", "8Gi", Some("16Gi")),
      ),
      (vec!["cpu".to_string()], qtys("500m", "1", "2", Some("4"))),
      // rows without allocatable are skipped
      (vec!["pods".to_string()], qtys("0", "10", "10", None)),
    ];

    let rows = summary_rows(&data);

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].name, "cpu");
    assert!((rows[0].utilization - 12.5).abs() < f64::EPSILON);
    assert!((rows[0].requested - 25.0).abs() < f64::EPSILON);
    assert!((rows[0].limit - 50.0).abs() < f64::EPSILON);
    assert_eq!(rows[1].name, "memory");
    assert!((rows[1].utilization - 12.5).abs() < f64::EPSILON);
  }

  #[test]
  fn test_utilization_matches_filter() {
    let qualifiers = vec![
      "namespace-a".to_string(),
      "pod-web-123".to_string(),
      "container-1".to_string(),
    ];

    assert!(utilization_matches_filter("", &qualifiers));
    assert!(utilization_matches_filter("pod-web", &qualifiers));
    assert!(utilization_matches_filter("namespace-*", &qualifiers));
    assert!(utilization_matches_filter(
      "namespace-a pod-web-123",
      &qualifiers
    ));
    assert!(!utilization_matches_filter("db", &qualifiers));
  }
}
