use ratatui::{
  layout::{Constraint, Rect},
  text::Line,
  widgets::{List, ListItem, ListState, Paragraph, Tabs},
  Frame,
};

use super::{
  utils::{
    centered_rect, default_part, filter_cursor_position, help_part, layout_block_default,
    layout_block_default_line, mixed_bold_line, mixed_line, split_hint_suffix, style_highlight,
    style_secondary, vertical_chunks_with_margin,
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

const TAB_PADDING_WIDTH: usize = 2;
const TAB_DIVIDER_WIDTH: usize = 1;

pub fn draw_resource_tabs_block(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let chunks =
    vertical_chunks_with_margin(vec![Constraint::Length(3), Constraint::Min(0)], area, 1);

  let mut block = layout_block_default(" Resources ");
  if app.get_current_route().active_block != ActiveBlock::Namespaces {
    block = block.style(style_secondary(app.light_theme))
  }

  let titles = resource_tab_titles(app);
  let visible_range = visible_resource_tab_range(
    &mut app.context_tabs.scroll_start,
    app.context_tabs.index,
    &titles,
    area.width.saturating_sub(2) as usize,
  );
  let visible_titles = titles[visible_range.clone()].to_vec();
  let selected_index = app.context_tabs.index.saturating_sub(visible_range.start);
  let tabs = Tabs::new(visible_titles)
    .block(block)
    .highlight_style(style_secondary(app.light_theme))
    .select(selected_index);

  f.render_widget(tabs, area);
  let separator_area = Rect {
    x: area.x + 1,
    y: chunks[0].y + chunks[0].height - 2,
    width: area.width.saturating_sub(2),
    height: 1,
  };
  f.render_widget(
    Paragraph::new("─".repeat(separator_area.width as usize))
      .style(style_secondary(app.light_theme)),
    separator_area,
  );
  let content_chunk = chunks[1];

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

fn resource_tab_titles(app: &App) -> Vec<Line<'static>> {
  app
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
    .collect()
}

fn visible_resource_tab_range(
  scroll_start: &mut usize,
  selected_index: usize,
  titles: &[Line<'_>],
  available_width: usize,
) -> std::ops::Range<usize> {
  if titles.is_empty() {
    *scroll_start = 0;
    return 0..0;
  }

  let selected_index = selected_index.min(titles.len().saturating_sub(1));
  *scroll_start = (*scroll_start).min(titles.len().saturating_sub(1));

  let widths: Vec<usize> = titles.iter().map(tab_width).collect();
  let current_end = visible_end(*scroll_start, &widths, available_width);
  let desired_left = selected_index.checked_sub(1);
  let desired_right = (selected_index + 1 < titles.len()).then_some(selected_index + 1);

  if selected_index < *scroll_start {
    *scroll_start = selected_index;
  } else if selected_index >= current_end {
    *scroll_start = reveal_selected_start(selected_index, &widths, available_width);
  }

  let mut best_start = *scroll_start;
  let mut best_end = visible_end(best_start, &widths, available_width);
  let mut best_missing_neighbors =
    missing_neighbors(best_start..best_end, desired_left, desired_right);
  let mut best_shift = best_start.abs_diff(*scroll_start);

  for candidate_start in 0..=selected_index {
    let candidate_end = visible_end(candidate_start, &widths, available_width);
    let candidate_range = candidate_start..candidate_end;
    if !candidate_range.contains(&selected_index) {
      continue;
    }

    let candidate_missing_neighbors =
      missing_neighbors(candidate_range.clone(), desired_left, desired_right);
    let candidate_shift = candidate_start.abs_diff(*scroll_start);

    if candidate_missing_neighbors < best_missing_neighbors
      || (candidate_missing_neighbors == best_missing_neighbors && candidate_shift < best_shift)
    {
      best_start = candidate_start;
      best_end = candidate_end;
      best_missing_neighbors = candidate_missing_neighbors;
      best_shift = candidate_shift;
    }
  }

  *scroll_start = best_start;
  best_start..best_end
}

fn reveal_selected_start(selected_index: usize, widths: &[usize], available_width: usize) -> usize {
  let mut start = selected_index;
  let mut used = widths[selected_index];

  while start > 0 {
    let next_used = used + TAB_DIVIDER_WIDTH + widths[start - 1];
    if next_used > available_width {
      break;
    }
    start -= 1;
    used = next_used;
  }

  start
}

fn visible_end(start: usize, widths: &[usize], available_width: usize) -> usize {
  let mut used = 0;
  let mut end = start;

  for (offset, width) in widths[start..].iter().enumerate() {
    let extra = if offset == 0 {
      *width
    } else {
      TAB_DIVIDER_WIDTH + *width
    };

    if used + extra > available_width {
      break;
    }

    used += extra;
    end = start + offset + 1;
  }

  if end == start {
    (start + 1).min(widths.len())
  } else {
    end
  }
}

fn tab_width(title: &Line<'_>) -> usize {
  TAB_PADDING_WIDTH + title_width(title)
}

fn missing_neighbors(
  range: std::ops::Range<usize>,
  left_neighbor: Option<usize>,
  right_neighbor: Option<usize>,
) -> usize {
  let mut missing = 0;

  if let Some(left_neighbor) = left_neighbor {
    if !range.contains(&left_neighbor) {
      missing += 1;
    }
  }

  if let Some(right_neighbor) = right_neighbor {
    if !range.contains(&right_neighbor) {
      missing += 1;
    }
  }

  missing
}

fn title_width(title: &Line<'_>) -> usize {
  title
    .spans
    .iter()
    .map(|span| span.content.chars().count())
    .sum()
}

fn tab_count_label(app: &App, index: usize) -> String {
  app
    .context_tab_resource_table(index)
    .map_or_else(|| "0".to_string(), |table| table.count_label())
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
    app::models::StatefulList,
    app::pods::KubePod,
    ui::utils::{MACCHIATO_RED, MACCHIATO_TEXT, MACCHIATO_YELLOW},
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
        "│ Pods [1] │ Services [0] <2> │ Nodes [0] <3> │ ConfigMaps [0] <4> │ StatefulSets [0] <5>          │",
        "│──────────────────────────────────────────────────────────────────────────────────────────────────│",
        "│                                                                                                  │",
        "│ Pods (ns: all) [1] containers <Enter> | filter </> | describe <d> | yaml <y> | logs <L>  | wide <│",
        "│   Namespace                Name                         Ready      Status    Restarts   Age      │",
        "│=> pod namespace test       pod name test                0/2        Failed    0          6h52m    │",
        "│                                                                                                  │",
        "│                                                                                                  │",
        "└──────────────────────────────────────────────────────────────────────────────────────────────────┘",
      ]
    );

    assert_eq!(buffer[(0, 0)].fg, MACCHIATO_YELLOW);
    assert_eq!(buffer[(1, 0)].fg, MACCHIATO_YELLOW);
    assert!(buffer[(1, 0)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(17, 1)].fg, MACCHIATO_TEXT);
    assert_eq!(buffer[(1, 4)].fg, MACCHIATO_YELLOW);
    assert!(buffer[(1, 4)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(1, 5)].fg, MACCHIATO_TEXT);
    assert_eq!(buffer[(1, 6)].fg, MACCHIATO_RED);
    assert!(buffer[(1, 6)].modifier.contains(Modifier::REVERSED));
    assert_eq!(buffer[(99, 9)].fg, MACCHIATO_YELLOW);
  }

  #[test]
  fn test_draw_resource_tabs_block_shows_active_filter_inline() {
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
        app.data.pods.filter = "pod".into();
        app.data.pods.filter_active = true;
        draw_resource_tabs_block(f, &mut app, size);
      })
      .unwrap();

    let lines: Vec<String> = (0..terminal.backend().buffer().area.height)
      .map(|row| {
        (0..terminal.backend().buffer().area.width)
          .map(|col| terminal.backend().buffer()[(col, row)].symbol())
          .collect::<String>()
      })
      .collect();

    let joined = lines.join("\n");
    assert!(joined.contains("[pod]"));
    assert!(joined.contains("clear <Esc>"));
    assert!(!joined.contains("containers <Enter>"));
    assert!(!joined.contains("describe <d>"));
    assert!(!joined.contains("yaml <y>"));
    assert!(!joined.contains("logs <L>"));
    assert!(!joined.contains("filter </> ─"));
  }

  #[test]
  fn test_draw_resource_tabs_block_keeps_leftmost_window_for_visible_selection() {
    let backend = TestBackend::new(55, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::default();
    let route = app.context_tabs.set_index(1).route.clone();
    app.push_navigation_route(route);

    terminal
      .draw(|f| {
        draw_resource_tabs_block(f, &mut app, f.area());
      })
      .unwrap();

    let row = buffer_row(terminal.backend().buffer(), 1);
    assert!(row.contains("Pods [0]"));
    assert!(row.contains("Services [0]"));
    assert!(row.contains("Nodes [0]"));
    assert_eq!(app.context_tabs.scroll_start, 0);
  }

  #[test]
  fn test_draw_resource_tabs_block_scrolls_to_far_right_selected_tab() {
    let backend = TestBackend::new(55, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::default();
    let route = app.context_tabs.set_index(8).route.clone();
    app.push_navigation_route(route);

    terminal
      .draw(|f| {
        draw_resource_tabs_block(f, &mut app, f.area());
      })
      .unwrap();

    let buffer = terminal.backend().buffer();
    let row = buffer_row(buffer, 1);
    assert!(row.contains("Jobs [0]"));
    assert!(row.contains("DaemonSets [0]"));
    assert!(row.contains("More [0]"));
    assert!(app.context_tabs.scroll_start > 0);

    let highlighted_col = row.find("DaemonSets [0]").unwrap() as u16 + 1;
    assert_eq!(buffer[(highlighted_col, 1)].fg, MACCHIATO_YELLOW);
  }

  #[test]
  fn test_draw_resource_tabs_block_minimal_reveal_keeps_neighbors_visible() {
    let backend = TestBackend::new(55, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::default();

    let route = app.context_tabs.set_index(8).route.clone();
    app.push_navigation_route(route);
    terminal
      .draw(|f| {
        draw_resource_tabs_block(f, &mut app, f.area());
      })
      .unwrap();
    let initial_scroll_start = app.context_tabs.scroll_start;

    let route = app.context_tabs.set_index(7).route.clone();
    app.push_navigation_route(route);
    terminal
      .draw(|f| {
        draw_resource_tabs_block(f, &mut app, f.area());
      })
      .unwrap();

    let row = buffer_row(terminal.backend().buffer(), 1);
    assert!(row.contains("Deployments [0]"));
    assert!(row.contains("Jobs [0]"));
    assert!(row.contains("DaemonSets [0]"));
    assert_eq!(app.context_tabs.scroll_start + 1, initial_scroll_start);
  }

  #[test]
  fn test_draw_resource_tabs_block_jump_to_hidden_tab_reveals_it() {
    let backend = TestBackend::new(55, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::default();
    let route = app.context_tabs.set_index(10).route.clone();
    app.push_navigation_route(route);

    terminal
      .draw(|f| {
        draw_resource_tabs_block(f, &mut app, f.area());
      })
      .unwrap();

    let row = buffer_row(terminal.backend().buffer(), 1);
    assert!(row.contains("More [0]"));
    assert!(row.contains("Dynamic [0]"));
    assert!(app.context_tabs.scroll_start > 0);
  }

  #[test]
  fn test_tab_count_label_returns_zero_for_unknown_index() {
    let app = App::default();

    assert_eq!(tab_count_label(&app, 99), "0");
  }

  #[test]
  fn test_tab_count_label_uses_visible_table_count() {
    let mut app = App::default();
    app
      .data
      .pods
      .set_items(vec![KubePod::default(), KubePod::default()]);

    assert_eq!(tab_count_label(&app, 0), "2");
  }

  #[test]
  fn test_draw_menu_shows_filter_prompt_when_active_without_text() {
    let lines = render_menu_lines(
      StatefulList::with_items(vec![("Secrets".into(), ActiveBlock::Secrets)]),
      "",
      true,
      &[],
    );
    let joined = lines.join("\n");

    assert!(joined.contains("Select Resource"));
    assert!(joined.contains("[type to filter]"));
  }

  #[test]
  fn test_draw_menu_shows_filter_text_and_counts() {
    let lines = render_menu_lines(
      StatefulList::with_items(vec![
        ("Secrets".into(), ActiveBlock::Secrets),
        ("Events".into(), ActiveBlock::Events),
      ]),
      "e",
      true,
      &[(ActiveBlock::Secrets, 3), (ActiveBlock::Events, 0)],
    );
    let joined = lines.join("\n");

    assert!(joined.contains("Select Resource [e]"));
    assert!(joined.contains("Secrets [3]"));
    assert!(joined.contains("Events"));
    assert!(!joined.contains("Events [0]"));
  }

  #[test]
  fn test_draw_menu_shows_filter_hint_when_inactive() {
    let lines = render_menu_lines(
      StatefulList::with_items(vec![("Secrets".into(), ActiveBlock::Secrets)]),
      "",
      false,
      &[],
    );
    let joined = lines.join("\n");

    assert!(joined.contains("Select Resource"));
    assert!(joined.contains("to filter"));
    assert!(joined.contains("</>"));
  }

  #[test]
  fn test_draw_menu_clamps_selection_to_filtered_items() {
    let mut menu = StatefulList::with_items(vec![
      ("Secrets".into(), ActiveBlock::Secrets),
      ("Events".into(), ActiveBlock::Events),
    ]);
    menu.state.select(Some(1));

    let backend = TestBackend::new(60, 10);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
      .draw(|f| {
        draw_menu(f, &mut menu, "sec", true, &[], false, f.area());
      })
      .unwrap();

    assert_eq!(menu.state.selected(), Some(0));
  }

  fn render_menu_lines(
    mut menu: StatefulList<(String, ActiveBlock)>,
    filter: &str,
    filter_active: bool,
    counts: &[(ActiveBlock, usize)],
  ) -> Vec<String> {
    let backend = TestBackend::new(60, 10);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
      .draw(|f| {
        draw_menu(f, &mut menu, filter, filter_active, counts, false, f.area());
      })
      .unwrap();

    let buffer = terminal.backend().buffer();
    (0..buffer.area.height)
      .map(|row| {
        (0..buffer.area.width)
          .map(|col| buffer[(col, row)].symbol())
          .collect::<String>()
      })
      .collect()
  }

  fn buffer_row(buffer: &ratatui::buffer::Buffer, row: u16) -> String {
    (0..buffer.area.width)
      .map(|col| buffer[(col, row)].symbol())
      .collect::<String>()
  }
}
