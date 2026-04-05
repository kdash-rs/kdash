use ratatui::{
  layout::{Constraint, Rect},
  text::{Line, Span},
  widgets::{List, ListItem, ListState, Tabs},
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

  let tab_counts = [
    app.data.pods.items.len(),
    app.data.services.items.len(),
    app.data.nodes.items.len(),
    app.data.config_maps.items.len(),
    app.data.stateful_sets.items.len(),
    app.data.replica_sets.items.len(),
    app.data.deployments.items.len(),
    app.data.jobs.items.len(),
    app.data.daemon_sets.items.len(),
    0, // More
    0, // Dynamic
  ];
  let titles: Vec<_> = app
    .context_tabs
    .items
    .iter()
    .enumerate()
    .map(|(i, t)| {
      let count = tab_counts.get(i).copied().unwrap_or(0);
      let label = if count > 0 {
        // Insert count before the shortcut key hint, e.g. "Pods [5] <1>"
        if let Some(pos) = t.title.find('<') {
          let (name, hint) = t.title.split_at(pos);
          format!("{}[{}] {}", name, count, hint)
        } else {
          format!("{} [{}]", t.title, count)
        }
      } else {
        t.title.clone()
      };
      Line::from(Span::styled(label, style_default(app.light_theme)))
    })
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
  // Collect counts before borrowing menu mutably
  let counts: Vec<(ActiveBlock, usize)> = vec![
    (ActiveBlock::CronJobs, app.data.cronjobs.items.len()),
    (ActiveBlock::Secrets, app.data.secrets.items.len()),
    (
      ActiveBlock::ReplicationControllers,
      app.data.replication_controllers.items.len(),
    ),
    (
      ActiveBlock::StorageClasses,
      app.data.storage_classes.items.len(),
    ),
    (ActiveBlock::Roles, app.data.roles.items.len()),
    (
      ActiveBlock::RoleBindings,
      app.data.role_bindings.items.len(),
    ),
    (
      ActiveBlock::ClusterRoles,
      app.data.cluster_roles.items.len(),
    ),
    (
      ActiveBlock::ClusterRoleBindings,
      app.data.cluster_role_bindings.items.len(),
    ),
    (ActiveBlock::Ingresses, app.data.ingress.items.len()),
    (
      ActiveBlock::PersistentVolumeClaims,
      app.data.persistent_volume_claims.items.len(),
    ),
    (
      ActiveBlock::PersistentVolumes,
      app.data.persistent_volumes.items.len(),
    ),
    (
      ActiveBlock::ServiceAccounts,
      app.data.service_accounts.items.len(),
    ),
    (
      ActiveBlock::NetworkPolicies,
      app.data.network_policies.items.len(),
    ),
  ];
  match block {
    ActiveBlock::More => draw_menu(
      f,
      &mut app.more_resources_menu,
      &app.menu_filter,
      app.menu_filter_active,
      &counts,
      area,
    ),
    ActiveBlock::DynamicView => draw_menu(
      f,
      &mut app.dynamic_resources_menu,
      &app.menu_filter,
      app.menu_filter_active,
      &counts,
      area,
    ),
    ActiveBlock::CronJobs => CronJobResource::render(block, f, app, area),
    ActiveBlock::Secrets => SecretResource::render(block, f, app, area),
    ActiveBlock::ReplicationControllers => {
      ReplicationControllerResource::render(block, f, app, area)
    }
    ActiveBlock::StorageClasses => StorageClassResource::render(block, f, app, area),
    ActiveBlock::Roles => RoleResource::render(block, f, app, area),
    ActiveBlock::RoleBindings => RoleBindingResource::render(block, f, app, area),
    ActiveBlock::ClusterRoles => ClusterRoleResource::render(block, f, app, area),
    ActiveBlock::ClusterRoleBindings => ClusterRoleBindingResource::render(block, f, app, area),
    ActiveBlock::Ingresses => IngressResource::render(block, f, app, area),
    ActiveBlock::PersistentVolumeClaims => PvcResource::render(block, f, app, area),
    ActiveBlock::PersistentVolumes => PvResource::render(block, f, app, area),
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
        ActiveBlock::ReplicationControllers => {
          ReplicationControllerResource::render(block, f, app, area)
        }
        ActiveBlock::StorageClasses => StorageClassResource::render(block, f, app, area),
        ActiveBlock::Roles => RoleResource::render(block, f, app, area),
        ActiveBlock::RoleBindings => RoleBindingResource::render(block, f, app, area),
        ActiveBlock::ClusterRoles => ClusterRoleResource::render(block, f, app, area),
        ActiveBlock::ClusterRoleBindings => ClusterRoleBindingResource::render(block, f, app, area),
        ActiveBlock::Ingresses => IngressResource::render(block, f, app, area),
        ActiveBlock::PersistentVolumeClaims => PvcResource::render(block, f, app, area),
        ActiveBlock::PersistentVolumes => PvResource::render(block, f, app, area),
        ActiveBlock::ServiceAccounts => SvcAcctResource::render(block, f, app, area),
        ActiveBlock::NetworkPolicies => NetworkPolicyResource::render(block, f, app, area),
        ActiveBlock::DynamicResource => DynamicResource::render(block, f, app, area),
        _ => { /* do nothing */ }
      }
    }
    ActiveBlock::Pods => crate::app::pods::draw_block_as_sub(f, app, area),
    ActiveBlock::Containers => crate::app::pods::draw_containers_block(f, app, area),
    ActiveBlock::Logs => crate::app::pods::draw_logs_block(f, app, area),
    ActiveBlock::Namespaces => draw_more(app.get_prev_route().active_block, f, app, area),
    _ => { /* do nothing */ }
  }
}

/// more resources menu
fn draw_menu(
  f: &mut Frame<'_>,
  more_resources_menu: &mut StatefulList<(String, ActiveBlock)>,
  filter: &str,
  filter_active: bool,
  counts: &[(ActiveBlock, usize)],
  area: Rect,
) {
  use crate::handlers::filter_menu_items;

  let area = centered_rect(50, 15, area);

  let filtered = filter_menu_items(&more_resources_menu.items, filter);
  let items: Vec<ListItem<'_>> = filtered
    .iter()
    .map(|(_, (name, block))| {
      let count = counts
        .iter()
        .find(|(b, _)| b == block)
        .map(|(_, c)| *c)
        .unwrap_or(0);
      if count > 0 {
        ListItem::new(format!("{} [{}]", name, count))
      } else {
        ListItem::new(name.clone())
      }
    })
    .collect();

  let title = if filter_active && !filter.is_empty() {
    format!(" Select Resource [{}] ", filter)
  } else if filter_active {
    " Select Resource (type to filter) ".to_string()
  } else {
    " Select Resource (/ to filter) ".to_string()
  };

  // Use a local ListState so selection operates within filtered bounds
  let selected = more_resources_menu
    .state
    .selected()
    .map(|i| i.min(items.len().saturating_sub(1)));
  let mut local_state = ListState::default();
  local_state.select(selected);

  f.render_stateful_widget(
    List::new(items)
      .block(layout_block_default(&title))
      .highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT),
    area,
    &mut local_state,
  );

  // Sync the clamped selection back
  more_resources_menu.state.select(local_state.selected());
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
        "│ Pods [1] <1> │ Services <2> │ Nodes <3> │ ConfigMaps <4> │ StatefulSets <5> │ ReplicaSets <6> │ D│",
        "│                                                                                                  │",
        "│ Pods (ns: all) [1] | Containers <enter> | describe <d> | yaml <y> | logs <o> ────────────────────│",
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
        0..=16 | 29..=31 | 41..=43 | 58..=60 | 77..=79 | 95..=97 | 99 => {
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
        0 | 79..=99 => {
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
