use ratatui::{
  layout::{Constraint, Rect},
  text::{Line, Span},
  widgets::{List, ListItem, Tabs},
  Frame,
};

use super::{
  utils::{
    centered_rect, layout_block_default, style_default, style_highlight, style_secondary,
    vertical_chunks_with_margin,
  },
  HIGHLIGHT,
};
use crate::app::{
  configmaps::ConfigMapResource,
  cronjobs::CronJobResource,
  daemonsets::DaemonSetResource,
  deployments::DeploymentResource,
  dynamic::DynamicResource,
  ingress::IngressResource,
  jobs::JobResource,
  models::{AppResource, StatefulList},
  network_policies::NetworkPolicyResource,
  nodes::NodeResource,
  pods::PodResource,
  pvcs::PvcResource,
  pvs::PvResource,
  replicasets::ReplicaSetResource,
  replication_controllers::ReplicationControllerResource,
  roles::{ClusterRoleBindingResource, ClusterRoleResource, RoleBindingResource, RoleResource},
  secrets::SecretResource,
  serviceaccounts::SvcAcctResource,
  statefulsets::StatefulSetResource,
  storageclass::StorageClassResource,
  svcs::SvcResource,
  ActiveBlock, App,
};

pub fn draw_resource_tabs_block(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let chunks =
    vertical_chunks_with_margin(vec![Constraint::Length(2), Constraint::Min(0)], area, 1);

  let mut block = layout_block_default(" Resources ");
  if app.get_current_route().active_block != ActiveBlock::Namespaces {
    block = block.style(style_secondary(app.light_theme))
  }

  let titles: Vec<_> = app
    .context_tabs
    .items
    .iter()
    .map(|t| Line::from(Span::styled(&t.title, style_default(app.light_theme))))
    .collect();
  let tabs = Tabs::new(titles)
    .block(block)
    .highlight_style(style_secondary(app.light_theme))
    .select(app.context_tabs.index);

  f.render_widget(tabs, area);

  // render tab content
  match app.context_tabs.index {
    0 => PodResource::render(app.get_current_route().active_block, f, app, chunks[1]),
    1 => SvcResource::render(app.get_current_route().active_block, f, app, chunks[1]),
    2 => NodeResource::render(app.get_current_route().active_block, f, app, chunks[1]),
    3 => ConfigMapResource::render(app.get_current_route().active_block, f, app, chunks[1]),
    4 => StatefulSetResource::render(app.get_current_route().active_block, f, app, chunks[1]),
    5 => ReplicaSetResource::render(app.get_current_route().active_block, f, app, chunks[1]),
    6 => DeploymentResource::render(app.get_current_route().active_block, f, app, chunks[1]),
    7 => JobResource::render(app.get_current_route().active_block, f, app, chunks[1]),
    8 => DaemonSetResource::render(app.get_current_route().active_block, f, app, chunks[1]),
    9 | 10 => draw_more(app.get_current_route().active_block, f, app, chunks[1]),
    _ => {}
  };
}

/// more resources tab
fn draw_more(block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect) {
  match block {
    ActiveBlock::More => draw_menu(f, &mut app.more_resources_menu, area),
    ActiveBlock::DynamicView => draw_menu(f, &mut app.dynamic_resources_menu, area),
    ActiveBlock::CronJobs => CronJobResource::render(block, f, app, area),
    ActiveBlock::Secrets => SecretResource::render(block, f, app, area),
    ActiveBlock::RplCtrl => ReplicationControllerResource::render(block, f, app, area),
    ActiveBlock::StorageClasses => StorageClassResource::render(block, f, app, area),
    ActiveBlock::Roles => RoleResource::render(block, f, app, area),
    ActiveBlock::RoleBindings => RoleBindingResource::render(block, f, app, area),
    ActiveBlock::ClusterRoles => ClusterRoleResource::render(block, f, app, area),
    ActiveBlock::ClusterRoleBinding => ClusterRoleBindingResource::render(block, f, app, area),
    ActiveBlock::Ingress => IngressResource::render(block, f, app, area),
    ActiveBlock::Pvc => PvcResource::render(block, f, app, area),
    ActiveBlock::Pv => PvResource::render(block, f, app, area),
    ActiveBlock::ServiceAccounts => SvcAcctResource::render(block, f, app, area),
    ActiveBlock::NetworkPolicies => NetworkPolicyResource::render(block, f, app, area),
    ActiveBlock::DynamicResource => DynamicResource::render(block, f, app, area),
    ActiveBlock::Describe | ActiveBlock::Yaml => {
      let mut prev_route = app.get_prev_route();
      if prev_route.active_block == block {
        prev_route = app.get_nth_route_from_last(2);
      }
      match prev_route.active_block {
        ActiveBlock::CronJobs => CronJobResource::render(block, f, app, area),
        ActiveBlock::Secrets => SecretResource::render(block, f, app, area),
        ActiveBlock::RplCtrl => ReplicationControllerResource::render(block, f, app, area),
        ActiveBlock::StorageClasses => StorageClassResource::render(block, f, app, area),
        ActiveBlock::Roles => RoleResource::render(block, f, app, area),
        ActiveBlock::RoleBindings => RoleBindingResource::render(block, f, app, area),
        ActiveBlock::ClusterRoles => ClusterRoleResource::render(block, f, app, area),
        ActiveBlock::ClusterRoleBinding => ClusterRoleBindingResource::render(block, f, app, area),
        ActiveBlock::Ingress => IngressResource::render(block, f, app, area),
        ActiveBlock::Pvc => PvcResource::render(block, f, app, area),
        ActiveBlock::Pv => PvResource::render(block, f, app, area),
        ActiveBlock::ServiceAccounts => SvcAcctResource::render(block, f, app, area),
        ActiveBlock::NetworkPolicies => NetworkPolicyResource::render(block, f, app, area),
        ActiveBlock::DynamicResource => DynamicResource::render(block, f, app, area),
        _ => { /* do nothing */ }
      }
    }
    ActiveBlock::Namespaces => draw_more(app.get_prev_route().active_block, f, app, area),
    _ => { /* do nothing */ }
  }
}

/// more resources menu
fn draw_menu(
  f: &mut Frame<'_>,
  more_resources_menu: &mut StatefulList<(String, ActiveBlock)>,
  area: Rect,
) {
  let area = centered_rect(50, 15, area);

  let items: Vec<ListItem<'_>> = more_resources_menu
    .items
    .iter()
    .map(|it| ListItem::new(it.0.clone()))
    .collect();
  f.render_stateful_widget(
    List::new(items)
      .block(layout_block_default(" Select Resource "))
      .highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT),
    area,
    &mut more_resources_menu.state,
  );
}

#[cfg(test)]
mod tests {
  use ratatui::{
    backend::TestBackend,
    buffer::Buffer,
    layout::Position,
    style::{Modifier, Style},
    Terminal,
  };

  use super::*;
  use crate::{
    app::pods::KubePod,
    ui::utils::{COLOR_RED, COLOR_WHITE, COLOR_YELLOW},
  };

  #[test]
  fn test_draw_resource_tabs_block() {
    let backend = TestBackend::new(100, 7);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
      .draw(|f| {
        let size = f.area();
        let mut app = App::default();
        let mut pod = KubePod::default();
        pod.name = "pod name test".into();
        pod.namespace = "pod namespace test".into();
        pod.ready = (0, 2);
        pod.status = "Failed".into();
        pod.age = "6h52m".into();
        app.data.pods.set_items(vec![pod]);
        draw_resource_tabs_block(f, &mut app, size);
      })
      .unwrap();

    let mut expected = Buffer::with_lines(vec![
        "┌ Resources ───────────────────────────────────────────────────────────────────────────────────────┐",
        "│ Pods <1> │ Services <2> │ Nodes <3> │ ConfigMaps <4> │ StatefulSets <5> │ ReplicaSets <6> │ Deplo│",
        "│                                                                                                  │",
        "│ Pods (ns: all) [1] | Containers <enter> | describe <d> | yaml <y> ───────────────────────────────│",
        "│   Namespace                Name                         Ready      Status    Restarts   Age      │",
        "│=> pod namespace test       pod name test                0/2        Failed    0          6h52m    │",
        "└──────────────────────────────────────────────────────────────────────────────────────────────────┘",
      ]);
    // set row styles
    // First row heading style
    for col in 0..=99 {
      match col {
        0 | 12..=99 => {
          expected
            .cell_mut(Position::new(col, 0))
            .unwrap()
            .set_style(Style::default().fg(COLOR_YELLOW));
        }
        _ => {
          expected.cell_mut(Position::new(col, 0)).unwrap().set_style(
            Style::default()
              .fg(COLOR_YELLOW)
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
            .cell_mut(Position::new(col, 1))
            .unwrap()
            .set_style(Style::default().fg(COLOR_YELLOW));
        }
        _ => {
          expected
            .cell_mut(Position::new(col, 1))
            .unwrap()
            .set_style(Style::default().fg(COLOR_WHITE));
        }
      }
    }
    // third empty row
    for col in 0..=99 {
      expected
        .cell_mut(Position::new(col, 2))
        .unwrap()
        .set_style(Style::default().fg(COLOR_YELLOW));
    }

    // fourth row tab header style
    for col in 0..=99 {
      match col {
        0 | 68..=99 => {
          expected
            .cell_mut(Position::new(col, 3))
            .unwrap()
            .set_style(Style::default().fg(COLOR_YELLOW));
        }
        1..=20 => {
          expected.cell_mut(Position::new(col, 3)).unwrap().set_style(
            Style::default()
              .fg(COLOR_YELLOW)
              .add_modifier(Modifier::BOLD),
          );
        }
        _ => {
          expected.cell_mut(Position::new(col, 3)).unwrap().set_style(
            Style::default()
              .fg(COLOR_WHITE)
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
            .cell_mut(Position::new(col, 4))
            .unwrap()
            .set_style(Style::default().fg(COLOR_WHITE));
        }
        _ => {
          expected
            .cell_mut(Position::new(col, 4))
            .unwrap()
            .set_style(Style::default().fg(COLOR_YELLOW));
        }
      }
    }
    // first table data row style
    for col in 0..=99 {
      match col {
        1..=98 => {
          expected.cell_mut(Position::new(col, 5)).unwrap().set_style(
            Style::default()
              .fg(COLOR_RED)
              .add_modifier(Modifier::REVERSED),
          );
        }
        _ => {
          expected
            .cell_mut(Position::new(col, 5))
            .unwrap()
            .set_style(Style::default().fg(COLOR_YELLOW));
        }
      }
    }

    // last row
    for col in 0..=99 {
      expected
        .cell_mut(Position::new(col, 6))
        .unwrap()
        .set_style(Style::default().fg(COLOR_YELLOW));
    }

    terminal.backend().assert_buffer(&expected);
  }
}
