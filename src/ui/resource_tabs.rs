use ratatui::{
  layout::{Constraint, Rect},
  text::Line,
  widgets::{List, ListItem, ListState, Tabs},
  Frame,
};

use super::{
  utils::{
    centered_rect, default_part, filter_bar_title, filter_cursor_position, help_part,
    layout_block_default, layout_block_default_line, layout_block_top_border, mixed_bold_line,
    mixed_line, split_hint_suffix, style_highlight, style_secondary, vertical_chunks,
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
  events::EventResource,
  ingress::IngressResource,
  jobs::JobResource,
  key_binding::DEFAULT_KEYBINDING,
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
  let current_filter = app
    .current_or_selected_resource_table()
    .map(|table| (table.filter_text(), table.is_filter_active()));
  let chunks = vertical_chunks_with_margin(
    vec![
      Constraint::Length(2),
      Constraint::Length(2),
      Constraint::Min(0),
    ],
    area,
    1,
  );

  let mut block = layout_block_default(" Resources ");
  if app.get_current_route().active_block != ActiveBlock::Namespaces {
    block = block.style(style_secondary(app.light_theme))
  }

  let titles: Vec<Line<'_>> = app
    .context_tabs
    .items
    .iter()
    .enumerate()
    .map(|(i, t)| {
      let count = tab_count_label(app, i);
      let (name, hint) = split_hint_suffix(&t.title);
      if i == app.context_tabs.index {
        Line::from(format!("{} [{}]", name, count))
      } else if let Some(hint) = hint {
        mixed_line(
          [
            default_part(format!("{} [{}]", name, count)),
            help_part(format!(" {}", hint)),
          ],
          app.light_theme,
        )
      } else {
        Line::from(format!("{} [{}]", t.title, count))
      }
    })
    .collect();
  let tabs = Tabs::new(titles)
    .block(block)
    .highlight_style(style_secondary(app.light_theme))
    .select(app.context_tabs.index);

  f.render_widget(tabs, area);
  let filter_chunks = vertical_chunks(
    vec![Constraint::Length(1), Constraint::Length(1)],
    chunks[1],
  );
  draw_filter_bar(f, app, filter_chunks[0], current_filter);
  let content_chunk = chunks[2];

  // render tab content
  match app.context_tabs.index {
    0 => PodResource::render(app.get_current_route().active_block, f, app, content_chunk),
    1 => SvcResource::render(app.get_current_route().active_block, f, app, content_chunk),
    2 => NodeResource::render(app.get_current_route().active_block, f, app, content_chunk),
    3 => ConfigMapResource::render(app.get_current_route().active_block, f, app, content_chunk),
    4 => StatefulSetResource::render(app.get_current_route().active_block, f, app, content_chunk),
    5 => ReplicaSetResource::render(app.get_current_route().active_block, f, app, content_chunk),
    6 => DeploymentResource::render(app.get_current_route().active_block, f, app, content_chunk),
    7 => JobResource::render(app.get_current_route().active_block, f, app, content_chunk),
    8 => DaemonSetResource::render(app.get_current_route().active_block, f, app, content_chunk),
    9 | 10 => draw_more(app.get_current_route().active_block, f, app, content_chunk),
    _ => {}
  };
}

fn tab_count_label(app: &App, index: usize) -> String {
  app
    .context_tab_resource_table(index)
    .map_or_else(|| "0".to_string(), |table| table.count_label())
}

fn draw_filter_bar(f: &mut Frame<'_>, app: &App, area: Rect, current_filter: Option<(&str, bool)>) {
  let (filter, filter_active) = current_filter.unwrap_or(("", false));
  let title = filter_bar_title(filter, filter_active, app.light_theme);

  f.render_widget(layout_block_top_border(title), area);

  if filter_active {
    f.set_cursor_position(filter_cursor_position(area, 1, filter));
  }
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
    (ActiveBlock::Events, app.data.events.items.len()),
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
      app.light_theme,
      area,
    ),
    ActiveBlock::DynamicView => draw_menu(
      f,
      &mut app.dynamic_resources_menu,
      &app.menu_filter,
      app.menu_filter_active,
      &counts,
      app.light_theme,
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
    ActiveBlock::Events => EventResource::render(block, f, app, area),
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
        ActiveBlock::Events => EventResource::render(block, f, app, area),
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
  light_theme: bool,
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
    mixed_bold_line(
      [
        default_part(" Select Resource ".to_string()),
        default_part(format!("[{}] ", filter)),
      ],
      light_theme,
    )
  } else if filter_active {
    mixed_bold_line(
      [
        default_part(" Select Resource ".to_string()),
        help_part("[type to filter] ".to_string()),
      ],
      light_theme,
    )
  } else {
    mixed_bold_line(
      [
        default_part(" Select Resource ".to_string()),
        help_part(format!("{} to filter ", DEFAULT_KEYBINDING.filter.key)),
      ],
      light_theme,
    )
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
      .block(layout_block_default_line(title))
      .highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT),
    area,
    &mut local_state,
  );

  if filter_active {
    f.set_cursor_position(filter_cursor_position(
      area,
      " Select Resource [".chars().count(),
      filter,
    ));
  }

  // Sync the clamped selection back
  more_resources_menu.state.select(local_state.selected());
}

#[cfg(test)]
mod tests {
  use ratatui::{backend::TestBackend, style::Modifier, Terminal};

  use super::*;
  use crate::{
    app::pods::KubePod,
    ui::utils::{MACCHIATO_BLUE, MACCHIATO_RED, MACCHIATO_TEXT, MACCHIATO_YELLOW},
  };

  #[test]
  fn test_draw_resource_tabs_block() {
    let backend = TestBackend::new(100, 10);
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

    let buffer = terminal.backend().buffer();
    let lines: Vec<String> = (0..buffer.area.height)
      .map(|row| {
        (0..buffer.area.width)
          .map(|col| buffer[(col, row)].symbol())
          .collect::<String>()
      })
      .collect();
    assert_eq!(
      lines,
      vec![
        "┌ Resources ───────────────────────────────────────────────────────────────────────────────────────┐",
        "│ Pods [1] │ Services [0] <2> │ Nodes [0] <3> │ ConfigMaps [0] <4> │ StatefulSets [0] <5> │ Replica│",
        "│                                                                                                  │",
        "│ filter </> ──────────────────────────────────────────────────────────────────────────────────────│",
        "│                                                                                                  │",
        "│ Pods (ns: all) [1] Containers <Enter> | describe <d> | yaml <y> | logs <L> ──────────────────────│",
        "│   Namespace                Name                         Ready      Status    Restarts   Age      │",
        "│=> pod namespace test       pod name test                0/2        Failed    0          6h52m    │",
        "│                                                                                                  │",
        "└──────────────────────────────────────────────────────────────────────────────────────────────────┘",
      ]
    );

    assert_eq!(buffer[(0, 0)].fg, MACCHIATO_YELLOW);
    assert_eq!(buffer[(1, 0)].fg, MACCHIATO_YELLOW);
    assert!(buffer[(1, 0)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(17, 1)].fg, MACCHIATO_TEXT);
    assert_eq!(buffer[(1, 3)].fg, MACCHIATO_BLUE);
    assert_eq!(buffer[(0, 4)].fg, MACCHIATO_YELLOW);
    assert_eq!(buffer[(99, 4)].fg, MACCHIATO_YELLOW);
    assert_eq!(buffer[(1, 5)].fg, MACCHIATO_YELLOW);
    assert!(buffer[(1, 5)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(21, 5)].fg, MACCHIATO_BLUE);
    assert!(buffer[(21, 5)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(79, 5)].fg, MACCHIATO_YELLOW);
    assert_eq!(buffer[(1, 6)].fg, MACCHIATO_TEXT);
    assert_eq!(buffer[(99, 6)].fg, MACCHIATO_YELLOW);
    assert_eq!(buffer[(1, 7)].fg, MACCHIATO_RED);
    assert!(buffer[(1, 7)].modifier.contains(Modifier::REVERSED));
    assert_eq!(buffer[(99, 7)].fg, MACCHIATO_YELLOW);
  }
}
