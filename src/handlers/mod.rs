use anyhow::anyhow;
use crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};
use kubectl_view_allocations::GroupBy;
use serde::Serialize;
use std::{
  fs,
  path::{Path, PathBuf},
};

use crate::{
  app::{
    actions::{InputAction, InputModal, Modal, ResourceAction},
    key_binding::DEFAULT_KEYBINDING,
    models::{
      HasPodSelector, KubeResource, Scrollable, ScrollableTxt, StatefulList, StatefulTable,
    },
    secrets::KubeSecret,
    troubleshoot::ResourceKind,
    ActiveBlock, App, PendingShellExec, Route, RouteId,
  },
  cmd::IoCmdEvent,
  event::Key,
  network::{IoEvent, ResourcePatch},
};

/// Handles Enter/`o` key on a workload resource: describe/yaml, drill-down to pods, or aggregate logs.
macro_rules! handle_workload_action {
  ($key:expr, $app:expr, $field:ident, $kind:expr) => {
    if $key == DEFAULT_KEYBINDING.aggregate_logs.key {
      // `o` key — aggregate logs from all pods
      if let Some(res) = $app.data.$field.get_selected_item_copy() {
        if let Some(selector) = res.pod_label_selector() {
          $app
            .dispatch_aggregate_logs(
              res.name.clone(),
              res.namespace.clone(),
              selector,
              $kind.to_owned(),
              RouteId::Home,
            )
            .await;
        }
      }
    } else if let Some(res) = handle_block_action($key, &$app.data.$field) {
      let ok = handle_describe_decode_or_yaml_action(
        $key,
        $app,
        &res,
        IoCmdEvent::GetDescribe {
          kind: $kind.to_owned(),
          value: res.name.to_owned(),
          ns: Some(res.namespace.to_owned()),
        },
      )
      .await;
      if !ok {
        // Enter key pressed — drill down to the resource's pods
        if let Some(selector) = res.pod_label_selector() {
          $app
            .dispatch_resource_pods(
              res.namespace.clone(),
              selector,
              $kind.to_owned(),
              RouteId::Home,
            )
            .await;
        }
      }
    }
  };
}

/// Dispatches block action (describe/yaml/decode) for standard resource types.
/// Wraps the entire match expression. Special-case arms go in the `extra` block.
macro_rules! handle_resource_action {
  ($match_expr:expr, $key:expr, $app:expr,
    namespaced: [ $(($block:path, $field:ident, $kind:expr)),* $(,)? ],
    cluster: [ $(($cblock:path, $cfield:ident, $ckind:expr)),* $(,)? ],
    extra: { $($extra_arms:tt)* }
  ) => {
    match $match_expr {
      $(
        $block => {
          if let Some(res) = handle_block_action($key, &$app.data.$field) {
            handle_leaf_resource_action(
              $key,
              $app,
              &res,
              $kind.to_owned(),
              Some(res.namespace.to_owned()),
            )
            .await;
          }
        }
      )*
      $(
        $cblock => {
          if let Some(res) = handle_block_action($key, &$app.data.$cfield) {
            handle_leaf_resource_action($key, $app, &res, $ckind.to_owned(), None).await;
          }
        }
      )*
      $($extra_arms)*
    }
  };
}

/// Dispatches scroll for standard resource types.
/// Wraps the entire match expression. Special-case arms go in the `extra` block.
macro_rules! handle_resource_scroll {
  ($match_expr:expr, $app:expr, $event:expr, $is_mouse:expr,
    [ $(($block:path, $field:ident)),* $(,)? ],
    extra: { $($extra_arms:tt)* }
  ) => {
    match $match_expr {
      $(
        $block => $app.data.$field.handle_scroll($event),
      )*
      $($extra_arms)*
    }
  };
}

pub async fn handle_key_events(key: Key, key_event: KeyEvent, app: &mut App) {
  let _ = key_event;

  // Overlay modals and the action menu consume input before anything else.
  if app.modal.is_some() {
    handle_modal_key(key, app).await;
    return;
  }
  if app.input_modal.is_some() {
    handle_input_modal_key(key, app);
    return;
  }
  if app.action_menu.is_some() {
    handle_action_menu_key(key, app).await;
    return;
  }

  let resource_filter_active = app
    .current_resource_table()
    .is_some_and(|table| table.is_filter_active());

  if app.is_menu_active() && app.menu_filter_active && handle_menu_filter_key(key, app) {
    // Menu filter captured the key — done
  } else if app.is_menu_active() && !app.menu_filter_active && key == DEFAULT_KEYBINDING.filter.key
  {
    // Activate menu filter mode
    app.menu_filter_active = true;
  } else if resource_filter_active && handle_resource_filter_key(key, app) {
    // Resource filter captured the key — done
  } else if app.get_current_route().active_block == ActiveBlock::Namespaces
    && app.ns_filter_active
    && handle_namespace_filter_key(key, app)
  {
    // Namespace filter captured the key — done
  } else {
    // First handle any global event and then move to route event
    match key {
      _ if key == DEFAULT_KEYBINDING.esc.key => {
        handle_escape(app);
      }
      _ if key == DEFAULT_KEYBINDING.quit.key || key == DEFAULT_KEYBINDING.quit.alt.unwrap() => {
        app.should_quit = true;
      }
      // Keep raw arrow navigation working even with remapped keybindings.
      // In alternate-screen mode without mouse capture, some terminals translate
      // mouse wheel scrolling into Up/Down key events.
      _ if key == DEFAULT_KEYBINDING.up.key
        || key == DEFAULT_KEYBINDING.up.alt.unwrap()
        || key == Key::Up =>
      {
        handle_block_scroll(app, ScrollEvent::up(), false).await;
      }
      _ if key == DEFAULT_KEYBINDING.down.key
        || key == DEFAULT_KEYBINDING.down.alt.unwrap()
        || key == Key::Down =>
      {
        handle_block_scroll(app, ScrollEvent::down(), false).await;
      }
      _ if key == DEFAULT_KEYBINDING.pg_up.key => {
        handle_block_scroll(app, ScrollEvent::Relative(-10), false).await;
      }
      _ if key == DEFAULT_KEYBINDING.pg_down.key => {
        handle_block_scroll(app, ScrollEvent::Relative(10), false).await;
      }
      _ if key == DEFAULT_KEYBINDING.home.key => {
        handle_block_scroll(app, ScrollEvent::Absolute(0), false).await;
      }
      _ if key == DEFAULT_KEYBINDING.end.key => {
        handle_block_scroll(app, ScrollEvent::End, false).await;
      }
      _ if key == DEFAULT_KEYBINDING.toggle_theme.key => {
        app.cycle_theme();
        app.set_status_message(format!("Theme: {}", app.palette.name));
      }
      _ if key == DEFAULT_KEYBINDING.cycle_theme_prev.key => {
        app.cycle_theme_prev();
        app.set_status_message(format!("Theme: {}", app.palette.name));
      }
      _ if key == DEFAULT_KEYBINDING.toggle_wide_columns.key => {
        app.wide_columns = !app.wide_columns;
      }
      _ if key == DEFAULT_KEYBINDING.refresh.key => {
        app.refresh();
      }
      _ if key == DEFAULT_KEYBINDING.dump_error_log.key => {
        dump_error_history(app, None);
      }
      _ if key == DEFAULT_KEYBINDING.help.key => {
        if app.get_current_route().active_block != ActiveBlock::Help {
          app.help_scroll = 0;
          app.push_navigation_stack(RouteId::HelpMenu, ActiveBlock::Help);
        }
      }
      _ if key == DEFAULT_KEYBINDING.jump_to_all_context.key => {
        app.route_contexts();
      }
      _ if key == DEFAULT_KEYBINDING.jump_to_current_context.key => {
        app.route_home();
      }
      _ if key == DEFAULT_KEYBINDING.jump_to_utilization.key => {
        app.route_utilization();
      }
      _ if key == DEFAULT_KEYBINDING.jump_to_troubleshoot.key => {
        app.route_troubleshoot();
      }
      _ if key == DEFAULT_KEYBINDING.cycle_main_views.key => {
        app.cycle_main_routes();
      }
      _ if key == DEFAULT_KEYBINDING.cycle_main_views_prev.key => {
        app.cycle_main_routes_prev();
      }
      _ if key == DEFAULT_KEYBINDING.open_action_menu.key => {
        let block = app.get_current_route().active_block;
        app.open_action_menu(block);
      }
      _ => handle_route_events(key, app).await,
    }
  }
}

/// Handle keys while a confirmation modal overlay is active.
async fn handle_modal_key(key: Key, app: &mut App) {
  if key == Key::Char('y') || key == DEFAULT_KEYBINDING.submit.key {
    if let Some(modal) = app.modal.take() {
      app.dispatch(modal.on_confirm).await;
      // Refresh the affected view promptly after a write.
      app.tick_count = 0;
    }
  } else if key == Key::Char('n') || key == DEFAULT_KEYBINDING.esc.key {
    app.close_modal();
  }
}

/// Handle keys while a single-line input overlay is active. Printable chars edit
/// the buffer, `Enter` validates (chaining into a confirm modal on success or
/// showing an inline error on failure), `Esc` cancels.
fn handle_input_modal_key(key: Key, app: &mut App) {
  if key == DEFAULT_KEYBINDING.submit.key {
    let result = app.input_modal.as_ref().map(|input| input.validate());
    match result {
      Some(Ok(modal)) => {
        app.close_input_modal();
        app.open_modal(modal);
      }
      Some(Err(err)) => {
        if let Some(input) = app.input_modal.as_mut() {
          input.error = Some(err);
        }
      }
      None => {}
    }
  } else if key == DEFAULT_KEYBINDING.esc.key {
    app.close_input_modal();
  } else if let Some(input) = app.input_modal.as_mut() {
    match key {
      Key::Char(c) => {
        input.buffer.push(c);
        input.error = None;
      }
      Key::Backspace => {
        input.buffer.pop();
        input.error = None;
      }
      _ => {}
    }
  }
}

/// Resolve the `(name, namespace)` of the selected row for a mutable block.
fn selected_target(app: &App, block: ActiveBlock) -> Option<(String, Option<String>)> {
  macro_rules! namespaced {
    ($field:ident) => {
      app
        .data
        .$field
        .get_selected_item_copy()
        .map(|res| (res.name.clone(), Some(res.namespace.clone())))
    };
  }
  macro_rules! cluster {
    ($field:ident) => {
      app
        .data
        .$field
        .get_selected_item_copy()
        .map(|res| (res.name.clone(), None))
    };
  }

  match block {
    ActiveBlock::Pods => namespaced!(pods),
    ActiveBlock::Services => namespaced!(services),
    ActiveBlock::ConfigMaps => namespaced!(config_maps),
    ActiveBlock::Secrets => namespaced!(secrets),
    ActiveBlock::StatefulSets => namespaced!(stateful_sets),
    ActiveBlock::ReplicaSets => namespaced!(replica_sets),
    ActiveBlock::Deployments => namespaced!(deployments),
    ActiveBlock::Jobs => namespaced!(jobs),
    ActiveBlock::DaemonSets => namespaced!(daemon_sets),
    ActiveBlock::CronJobs => namespaced!(cronjobs),
    ActiveBlock::ReplicationControllers => namespaced!(replication_controllers),
    ActiveBlock::Roles => namespaced!(roles),
    ActiveBlock::RoleBindings => namespaced!(role_bindings),
    ActiveBlock::Ingresses => namespaced!(ingress),
    ActiveBlock::PersistentVolumeClaims => namespaced!(persistent_volume_claims),
    ActiveBlock::NetworkPolicies => namespaced!(network_policies),
    ActiveBlock::ServiceAccounts => namespaced!(service_accounts),
    ActiveBlock::Events => namespaced!(events),
    ActiveBlock::Nodes => cluster!(nodes),
    ActiveBlock::PersistentVolumes => cluster!(persistent_volumes),
    ActiveBlock::StorageClasses => cluster!(storage_classes),
    ActiveBlock::ClusterRoles => cluster!(cluster_roles),
    ActiveBlock::ClusterRoleBindings => cluster!(cluster_role_bindings),
    ActiveBlock::DynamicResource => app
      .data
      .dynamic_resources
      .get_selected_item_copy()
      .map(|res| (res.name.clone(), res.namespace.clone())),
    _ => None,
  }
}

/// Human-readable kind label for confirmation prompts.
fn resource_kind_label(app: &App, block: ActiveBlock) -> String {
  let label = match block {
    ActiveBlock::Pods => "pod",
    ActiveBlock::Services => "service",
    ActiveBlock::ConfigMaps => "configmap",
    ActiveBlock::Secrets => "secret",
    ActiveBlock::StatefulSets => "statefulset",
    ActiveBlock::ReplicaSets => "replicaset",
    ActiveBlock::Deployments => "deployment",
    ActiveBlock::Jobs => "job",
    ActiveBlock::DaemonSets => "daemonset",
    ActiveBlock::CronJobs => "cronjob",
    ActiveBlock::ReplicationControllers => "replicationcontroller",
    ActiveBlock::Roles => "role",
    ActiveBlock::RoleBindings => "rolebinding",
    ActiveBlock::Ingresses => "ingress",
    ActiveBlock::PersistentVolumeClaims => "persistentvolumeclaim",
    ActiveBlock::NetworkPolicies => "networkpolicy",
    ActiveBlock::ServiceAccounts => "serviceaccount",
    ActiveBlock::Events => "event",
    ActiveBlock::Nodes => "node",
    ActiveBlock::PersistentVolumes => "persistentvolume",
    ActiveBlock::StorageClasses => "storageclass",
    ActiveBlock::ClusterRoles => "clusterrole",
    ActiveBlock::ClusterRoleBindings => "clusterrolebinding",
    ActiveBlock::DynamicResource => {
      return app
        .data
        .selected
        .dynamic_kind
        .as_ref()
        .map(|kind| kind.kind.to_lowercase())
        .unwrap_or_else(|| "resource".to_owned());
    }
    _ => "resource",
  };
  label.to_owned()
}

/// Open a delete-confirmation modal for the selected row in the current block.
async fn handle_delete_resource(app: &mut App) {
  let block = app.get_current_route().active_block;
  let Some((name, namespace)) = selected_target(app, block) else {
    return;
  };
  let kind = resource_kind_label(app, block);
  let prompt = match &namespace {
    Some(ns) => format!(
      "Delete {} '{}' in namespace '{}'? This cannot be undone.",
      kind, name, ns
    ),
    None => format!("Delete {} '{}'? This cannot be undone.", kind, name),
  };
  app.open_modal(Modal::confirm(
    "Confirm delete",
    prompt,
    IoEvent::DeleteResource {
      block,
      name,
      namespace,
    },
  ));
}

/// Open a restart-confirmation modal for the selected workload. Only offered for
/// the rollout-capable kinds (deployments, statefulsets, daemonsets).
async fn handle_restart_resource(app: &mut App) {
  let block = app.get_current_route().active_block;
  if !matches!(
    block,
    ActiveBlock::Deployments | ActiveBlock::StatefulSets | ActiveBlock::DaemonSets
  ) {
    return;
  }
  let Some((name, namespace)) = selected_target(app, block) else {
    return;
  };
  let kind = resource_kind_label(app, block);
  let prompt = match &namespace {
    Some(ns) => format!("Rollout restart {} '{}' in namespace '{}'?", kind, name, ns),
    None => format!("Rollout restart {} '{}'?", kind, name),
  };
  app.open_modal(Modal::confirm(
    "Confirm restart",
    prompt,
    IoEvent::PatchResource {
      block,
      name,
      namespace,
      patch: ResourcePatch::RolloutRestart,
    },
  ));
}

/// Workloads that expose a `spec.replicas` count we can scale.
fn is_scalable(block: ActiveBlock) -> bool {
  matches!(
    block,
    ActiveBlock::Deployments
      | ActiveBlock::StatefulSets
      | ActiveBlock::ReplicaSets
      | ActiveBlock::ReplicationControllers
  )
}

/// Current desired replica count of the selected workload, read from the typed
/// object so it works uniformly across scalable kinds. Used to prefill the input.
fn current_replicas(app: &App, block: ActiveBlock) -> Option<i32> {
  match block {
    ActiveBlock::Deployments => app
      .data
      .deployments
      .get_selected_item_copy()
      .and_then(|r| r.get_k8s_obj().spec.as_ref().and_then(|s| s.replicas)),
    ActiveBlock::StatefulSets => app
      .data
      .stateful_sets
      .get_selected_item_copy()
      .and_then(|r| r.get_k8s_obj().spec.as_ref().and_then(|s| s.replicas)),
    ActiveBlock::ReplicaSets => app
      .data
      .replica_sets
      .get_selected_item_copy()
      .and_then(|r| r.get_k8s_obj().spec.as_ref().and_then(|s| s.replicas)),
    ActiveBlock::ReplicationControllers => app
      .data
      .replication_controllers
      .get_selected_item_copy()
      .and_then(|r| r.get_k8s_obj().spec.as_ref().and_then(|s| s.replicas)),
    _ => None,
  }
}

/// Open the scale input modal for the selected workload, prefilled with the
/// current replica count. Menu-only (no hotkey); only offered for scalable kinds.
fn handle_scale_resource(app: &mut App) {
  let block = app.get_current_route().active_block;
  if !is_scalable(block) {
    return;
  }
  let Some((name, namespace)) = selected_target(app, block) else {
    return;
  };
  let kind = resource_kind_label(app, block);
  let buffer = current_replicas(app, block)
    .map(|n| n.to_string())
    .unwrap_or_default();
  app.open_input_modal(InputModal {
    title: "Scale".to_owned(),
    prompt: format!("New replica count for {} '{}':", kind, name),
    buffer,
    error: None,
    action: InputAction::Scale {
      block,
      name,
      namespace,
      kind,
    },
  });
}

/// Handle keys while the `m` action menu overlay is active.
async fn handle_action_menu_key(key: Key, app: &mut App) {
  match key {
    _ if key == DEFAULT_KEYBINDING.esc.key => app.close_action_menu(),
    _ if key == DEFAULT_KEYBINDING.up.key
      || key == DEFAULT_KEYBINDING.up.alt.unwrap()
      || key == Key::Up =>
    {
      if let Some(menu) = app.action_menu.as_mut() {
        menu.handle_scroll(ScrollEvent::up());
      }
    }
    _ if key == DEFAULT_KEYBINDING.down.key
      || key == DEFAULT_KEYBINDING.down.alt.unwrap()
      || key == Key::Down =>
    {
      if let Some(menu) = app.action_menu.as_mut() {
        menu.handle_scroll(ScrollEvent::down());
      }
    }
    _ if key == DEFAULT_KEYBINDING.submit.key => {
      let selected = app.action_menu.as_ref().and_then(|menu| {
        menu
          .state
          .selected()
          .and_then(|i| menu.items.get(i).copied())
      });
      app.close_action_menu();
      if let Some(action) = selected {
        execute_resource_action(action, app).await;
      }
    }
    _ => {}
  }
}

/// Dispatch a menu-selected action. Hotkey-backed actions replay their key so
/// the menu and hotkey share one path; menu-only actions are handled directly.
async fn execute_resource_action(action: ResourceAction, app: &mut App) {
  match action {
    ResourceAction::Scale => handle_scale_resource(app),
    ResourceAction::Cordon => handle_cordon_toggle(app).await,
    ResourceAction::Suspend => handle_cronjob_suspend_toggle(app).await,
    ResourceAction::Trigger => handle_cronjob_trigger(app).await,
    other => {
      let block = app.get_current_route().active_block;
      if let Some(key) = other.hotkey(block) {
        handle_route_events(key, app).await;
      }
    }
  }
}

/// Open a suspend/resume confirmation for the selected cronjob. The direction is
/// derived from the cronjob's current `spec.suspend` state.
async fn handle_cronjob_suspend_toggle(app: &mut App) {
  let Some(cronjob) = app.data.cronjobs.get_selected_item_copy() else {
    return;
  };
  let suspend = !cronjob.suspend;
  let verb = if suspend { "Suspend" } else { "Resume" };
  app.open_modal(Modal::confirm(
    "Confirm suspend",
    format!(
      "{} cronjob '{}' in namespace '{}'?",
      verb, cronjob.name, cronjob.namespace
    ),
    IoEvent::PatchResource {
      block: ActiveBlock::CronJobs,
      name: cronjob.name.clone(),
      namespace: Some(cronjob.namespace.clone()),
      patch: ResourcePatch::SetSuspend(suspend),
    },
  ));
}

/// Open a confirmation to trigger an immediate run of the selected cronjob.
async fn handle_cronjob_trigger(app: &mut App) {
  let Some(cronjob) = app.data.cronjobs.get_selected_item_copy() else {
    return;
  };
  app.open_modal(Modal::confirm(
    "Confirm trigger",
    format!(
      "Trigger cronjob '{}' now? This creates a one-off Job.",
      cronjob.name
    ),
    IoEvent::TriggerCronJob {
      name: cronjob.name.clone(),
      namespace: cronjob.namespace.clone(),
    },
  ));
}

/// Open a cordon/uncordon confirmation for the selected node. The direction is
/// derived from the node's current `spec.unschedulable` state.
async fn handle_cordon_toggle(app: &mut App) {
  let Some(node) = app.data.nodes.get_selected_item_copy() else {
    return;
  };
  let currently_unschedulable = node
    .get_k8s_obj()
    .spec
    .as_ref()
    .and_then(|spec| spec.unschedulable)
    .unwrap_or(false);
  let cordon = !currently_unschedulable;
  let verb = if cordon { "Cordon" } else { "Uncordon" };
  app.open_modal(Modal::confirm(
    "Confirm cordon",
    format!("{} node '{}'?", verb, node.name),
    IoEvent::PatchResource {
      block: ActiveBlock::Nodes,
      name: node.name.clone(),
      namespace: None,
      patch: ResourcePatch::SetUnschedulable(cordon),
    },
  ));
}

pub async fn handle_mouse_events(mouse: MouseEvent, app: &mut App) {
  match mouse.kind {
    // mouse scrolling is inverted by passing is_mouse=true downstream code
    // which calls [inverse_dir] where intended for mouse input
    MouseEventKind::ScrollDown => handle_block_scroll(app, ScrollEvent::down(), true).await,
    MouseEventKind::ScrollUp => handle_block_scroll(app, ScrollEvent::up(), true).await,
    _ => {}
  }
}

fn handle_escape(app: &mut App) {
  // dismiss error
  if !app.api_error.is_empty() {
    app.api_error = String::default();
  } else if !app.status_message.is_empty() {
    app.clear_status_message();
  }

  // If menu filter is active, deactivate it first (clear text if any, else deactivate)
  if app.is_menu_active() && app.menu_filter_active {
    clear_or_deactivate_filter(&mut app.menu_filter, &mut app.menu_filter_active);
    return;
  }

  if app.get_current_route().active_block == ActiveBlock::Namespaces && app.ns_filter_active {
    clear_or_deactivate_filter(&mut app.ns_filter, &mut app.ns_filter_active);
    return;
  }

  if let Some((filter, filter_active, _)) = app.current_resource_filter_mut() {
    if *filter_active {
      clear_or_deactivate_filter(filter, filter_active);
      return;
    }
  }

  // Clear menu filter state on any menu exit
  if app.is_menu_active() {
    app.menu_filter.clear();
    app.menu_filter_active = false;
  }

  match app.get_current_route().id {
    RouteId::HelpMenu => {
      app.pop_navigation_stack();
    }
    _ => match app.get_current_route().active_block {
      ActiveBlock::Namespaces
      | ActiveBlock::Containers
      | ActiveBlock::Yaml
      | ActiveBlock::Describe => {
        app.pop_navigation_stack();
      }
      ActiveBlock::Pods if app.data.selected.pod_selector.is_some() => {
        // Exiting a filtered pod view from workload drill-down
        app.data.selected.pod_selector = None;
        app.data.selected.pod_selector_ns = None;
        app.data.selected.pod_selector_resource = None;
        app.pop_navigation_stack();
      }
      ActiveBlock::Logs => {
        app.cancel_log_stream();
        app.log_previous = false;
        // Clear resource context when leaving aggregate logs
        if app.data.selected.pod_selector.is_none() {
          app.data.selected.pod_selector_resource = None;
        }
        app.pop_navigation_stack();
      }
      _ => {
        // A resource opened from the More / Dynamic picker sits on top of that
        // menu route, so Esc returns to the picker. Evaluate the prev route
        // exactly once — re-reading it after a pop made stacked menus pop twice
        // (e.g. landing on the Dynamic menu after picking from More).
        if matches!(
          app.get_prev_route().active_block,
          ActiveBlock::More | ActiveBlock::DynamicView
        ) {
          app.pop_navigation_stack();
        }
      }
    },
  }
}

/// Handle character/backspace keys for menu filter input.
/// Returns true if the key was consumed, false to let it pass through.
fn handle_menu_filter_key(key: Key, app: &mut App) -> bool {
  match key {
    Key::Char(c) => {
      app.menu_filter.push(c);
      // Reset selection to first item when filter changes
      let menu = get_active_menu_mut(app);
      menu.state.select(Some(0));
      true
    }
    Key::Backspace => {
      app.menu_filter.pop();
      let menu = get_active_menu_mut(app);
      menu.state.select(Some(0));
      true
    }
    _ => false,
  }
}

fn handle_filter_text_key(filter: &mut String, key: Key) -> bool {
  match key {
    Key::Char(c) => {
      filter.push(c);
      true
    }
    Key::Backspace => {
      filter.pop();
      true
    }
    _ => false,
  }
}

fn clear_or_deactivate_filter(filter: &mut String, active: &mut bool) {
  if filter.is_empty() {
    *active = false;
  } else {
    filter.clear();
  }
}

fn handle_resource_filter_key(key: Key, app: &mut App) -> bool {
  if let Some((filter, _, state)) = app.current_resource_filter_mut() {
    let handled = handle_filter_text_key(filter, key);
    if handled {
      state.select(Some(0));
    }
    handled
  } else {
    false
  }
}

fn handle_namespace_filter_key(key: Key, app: &mut App) -> bool {
  let handled = handle_filter_text_key(&mut app.ns_filter, key);
  if handled {
    app.data.namespaces.state.select(Some(0));
  }
  handled
}

fn get_active_menu_mut(app: &mut App) -> &mut StatefulList<(String, ActiveBlock)> {
  match app.get_current_route().active_block {
    ActiveBlock::DynamicView => &mut app.dynamic_resources_menu,
    _ => &mut app.more_resources_menu,
  }
}

/// Filter menu items by the given filter string using case-insensitive substring + glob matching.
pub fn filter_menu_items<'a>(
  items: &'a [(String, ActiveBlock)],
  filter: &str,
) -> Vec<(usize, &'a (String, ActiveBlock))> {
  if filter.is_empty() {
    return items.iter().enumerate().collect();
  }
  let filter_lower = filter.to_lowercase();
  items
    .iter()
    .enumerate()
    .filter(|(_, (name, _))| {
      let name_lower = name.to_lowercase();
      name_lower.contains(&filter_lower) || glob_match::glob_match(&filter_lower, &name_lower)
    })
    .collect()
}

async fn handle_describe_decode_or_yaml_action<T, S>(
  key: Key,
  app: &mut App,
  res: &T,
  action: IoCmdEvent,
) -> bool
where
  T: KubeResource<S> + 'static,
  S: Serialize,
{
  if key == DEFAULT_KEYBINDING.describe_resource.key {
    app.data.describe_out = ScrollableTxt::new();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Describe);
    app.dispatch_cmd(action).await;
    true
  } else if key == DEFAULT_KEYBINDING.resource_yaml.key {
    let yaml = res.resource_to_yaml();
    app.data.describe_out = ScrollableTxt::with_string(yaml);
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Yaml);
    true
  } else if key == DEFAULT_KEYBINDING.decode_secret.key {
    // make sure the resources is of type 'KubeSecret'
    let of_any = res as &dyn std::any::Any;
    if let Some(secret) = of_any.downcast_ref::<KubeSecret>() {
      let display_output = secret.decode_secret();
      app.data.describe_out = ScrollableTxt::with_string(display_output);
      app.push_navigation_stack(RouteId::Home, ActiveBlock::Describe);
      true
    } else {
      // resource is not a secret
      false
    }
  } else {
    false
  }
}

async fn handle_leaf_resource_action<T, S>(
  key: Key,
  app: &mut App,
  res: &T,
  kind: String,
  ns: Option<String>,
) where
  T: KubeResource<S> + 'static,
  S: Serialize,
{
  let describe_action = IoCmdEvent::GetDescribe {
    kind,
    value: res.get_name().to_owned(),
    ns,
  };
  let handled = handle_describe_decode_or_yaml_action(key, app, res, describe_action.clone()).await;
  dispatch_describe_on_submit(key, app, handled, describe_action).await;
}

async fn dispatch_describe_on_submit(
  key: Key,
  app: &mut App,
  handled: bool,
  describe_action: IoCmdEvent,
) {
  if !handled && key == DEFAULT_KEYBINDING.submit.key {
    app.data.describe_out = ScrollableTxt::new();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Describe);
    app.dispatch_cmd(describe_action).await;
  }
}

// Handle event for the current active block
async fn handle_route_events(key: Key, app: &mut App) {
  // route specific events
  match app.get_current_route().id {
    // handle resource tabs on overview
    RouteId::Home => {
      match key {
        _ if key == DEFAULT_KEYBINDING.right.key
          || key == DEFAULT_KEYBINDING.right.alt.unwrap()
          || key == Key::Right =>
        {
          app.deactivate_current_resource_filter();
          app.context_tabs.next();
          app.push_navigation_route(app.context_tabs.get_active_route().clone());
        }
        _ if key == DEFAULT_KEYBINDING.left.key
          || key == DEFAULT_KEYBINDING.left.alt.unwrap()
          || key == Key::Left =>
        {
          app.deactivate_current_resource_filter();
          app.context_tabs.previous();
          app.push_navigation_route(app.context_tabs.get_active_route().clone());
        }
        _ if key == DEFAULT_KEYBINDING.filter.key => {
          if app.get_current_route().active_block == ActiveBlock::Namespaces {
            app.ns_filter_active = true;
          } else if let Some((_, filter_active, _)) = app.current_resource_filter_mut() {
            *filter_active = true;
          }
        }
        _ if key == DEFAULT_KEYBINDING.toggle_info.key => {
          app.show_info_bar = !app.show_info_bar;
        }
        _ if key == DEFAULT_KEYBINDING.select_all_namespace.key => app.data.selected.ns = None,
        _ if key == DEFAULT_KEYBINDING.delete_resource.key => {
          handle_delete_resource(app).await;
        }
        _ if key == DEFAULT_KEYBINDING.restart_resource.key => {
          handle_restart_resource(app).await;
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_namespace.key
          && app.get_current_route().active_block != ActiveBlock::Namespaces =>
        {
          app.push_navigation_stack(RouteId::Home, ActiveBlock::Namespaces);
        }
        // as these are tabs with index the order here matters, atleast for readability
        _ if key == DEFAULT_KEYBINDING.jump_to_pods.key => {
          // Clear any workload drill-down state so the pod view shows all pods
          app.data.selected.pod_selector = None;
          app.data.selected.pod_selector_ns = None;
          app.data.selected.pod_selector_resource = None;
          app.deactivate_current_resource_filter();
          let route = app.context_tabs.set_index(0).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_services.key => {
          app.deactivate_current_resource_filter();
          let route = app.context_tabs.set_index(1).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_nodes.key => {
          app.deactivate_current_resource_filter();
          let route = app.context_tabs.set_index(2).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_configmaps.key => {
          app.deactivate_current_resource_filter();
          let route = app.context_tabs.set_index(3).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_statefulsets.key => {
          app.deactivate_current_resource_filter();
          let route = app.context_tabs.set_index(4).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_replicasets.key => {
          app.deactivate_current_resource_filter();
          let route = app.context_tabs.set_index(5).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_deployments.key => {
          app.deactivate_current_resource_filter();
          let route = app.context_tabs.set_index(6).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_jobs.key => {
          app.deactivate_current_resource_filter();
          let route = app.context_tabs.set_index(7).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_daemonsets.key => {
          app.deactivate_current_resource_filter();
          let route = app.context_tabs.set_index(8).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_more_resources.key => {
          app.deactivate_current_resource_filter();
          let route = app.context_tabs.set_index(9).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_dynamic_resources.key => {
          app.deactivate_current_resource_filter();
          let route = app.context_tabs.set_index(10).route.clone();
          app.push_navigation_route(route);
        }
        _ => {}
      };

      // handle block specific stuff
      handle_resource_action!(app.get_current_route().active_block, key, app,
        namespaced: [
          (ActiveBlock::Services, services, "service"),
          (ActiveBlock::ConfigMaps, config_maps, "configmap"),
          (ActiveBlock::Secrets, secrets, "secret"),
          (ActiveBlock::Roles, roles, "roles"),
          (ActiveBlock::RoleBindings, role_bindings, "rolebindings"),
          (ActiveBlock::Ingresses, ingress, "ingress"),
          (ActiveBlock::PersistentVolumeClaims, persistent_volume_claims, "persistentvolumeclaims"),
          (ActiveBlock::ServiceAccounts, service_accounts, "serviceaccounts"),
          (ActiveBlock::Events, events, "event"),
          (ActiveBlock::NetworkPolicies, network_policies, "networkpolicy"),
        ],
        cluster: [
          (ActiveBlock::StorageClasses, storage_classes, "storageclass"),
          (ActiveBlock::ClusterRoles, cluster_roles, "clusterroles"),
          (ActiveBlock::ClusterRoleBindings, cluster_role_bindings, "clusterrolebinding"),
          (ActiveBlock::PersistentVolumes, persistent_volumes, "persistentvolumes"),
        ],
        extra: {
          ActiveBlock::Nodes => {
            if let Some(res) = handle_block_action(key, &app.data.nodes) {
              let ok = handle_describe_decode_or_yaml_action(
                key,
                app,
                &res,
                IoCmdEvent::GetDescribe {
                  kind: "node".to_owned(),
                  value: res.name.to_owned(),
                  ns: None,
                },
              )
              .await;
              if !ok {
                app.dispatch_node_pods(res.name.clone(), RouteId::Home).await;
              }
            }
          }
          ActiveBlock::Deployments => {
            handle_workload_action!(key, app, deployments, "deployment");
          }
          ActiveBlock::StatefulSets => {
            handle_workload_action!(key, app, stateful_sets, "statefulset");
          }
          ActiveBlock::ReplicaSets => {
            handle_workload_action!(key, app, replica_sets, "replicaset");
          }
          ActiveBlock::Jobs => {
            handle_workload_action!(key, app, jobs, "job");
          }
          ActiveBlock::DaemonSets => {
            handle_workload_action!(key, app, daemon_sets, "daemonset");
          }
          ActiveBlock::CronJobs => {
            handle_workload_action!(key, app, cronjobs, "cronjob");
          }
          ActiveBlock::ReplicationControllers => {
            handle_workload_action!(key, app, replication_controllers, "replicationcontroller");
          }
          ActiveBlock::Namespaces => {
            if let Some(ns) = handle_block_action(key, &app.data.namespaces) {
              app.data.selected.ns = Some(ns.name);
              app.cache_essential_data().await;
              app.queue_background_resource_cache();
              app.pop_navigation_stack();
            }
          }
          ActiveBlock::Pods => {
            if key == DEFAULT_KEYBINDING.aggregate_logs.key {
              if let Some(pod) = app.data.pods.get_selected_item_copy() {
                app.data.selected.pod = Some(pod.name.clone());
                app.data.selected.pod_selector_resource = Some("pod".into());
                app.data.containers.set_items(pod.containers);
                app.dispatch_pod_logs(pod.name, RouteId::Home).await;
              }
            } else if key == DEFAULT_KEYBINDING.previous_logs.key {
              handle_previous_logs_for_pod(app, RouteId::Home).await;
            } else if let Some(pod) = handle_block_action(key, &app.data.pods) {
              let ok = handle_describe_decode_or_yaml_action(
                key,
                app,
                &pod,
                IoCmdEvent::GetDescribe {
                  kind: "pod".to_owned(),
                  value: pod.name.to_owned(),
                  ns: Some(pod.namespace.to_owned()),
                },
              )
              .await;
              if !ok {
                app.push_navigation_stack(RouteId::Home, ActiveBlock::Containers);
                app.data.selected.pod = Some(pod.name);
                app.data.containers.set_items(pod.containers);
              }
            }
          }
          ActiveBlock::Containers => {
            if key == DEFAULT_KEYBINDING.shell_exec.key {
              queue_selected_container_shell_exec(app);
            } else if key == DEFAULT_KEYBINDING.previous_logs.key {
              handle_previous_logs_for_container(app, RouteId::Home).await;
            } else if let Some(c) = handle_block_action(key, &app.data.containers) {
              app.data.selected.container = Some(c.name.clone());
              app.dispatch_container_logs(c.name, RouteId::Home).await;
            }
          }
          ActiveBlock::Logs => {
            if key == DEFAULT_KEYBINDING.log_auto_scroll.key {
              if app.log_auto_scroll {
                app.data.logs.freeze_follow_position();
              }
              app.log_auto_scroll = !app.log_auto_scroll;
            } else if key == DEFAULT_KEYBINDING.copy_to_clipboard.key {
              copy_to_clipboard(app.data.logs.get_plain_text(), app);
            }
          }
          ActiveBlock::Describe | ActiveBlock::Yaml => {
            if key == DEFAULT_KEYBINDING.copy_to_clipboard.key {
              copy_to_clipboard(app.data.describe_out.get_txt().to_owned(), app);
            }
          }
          ActiveBlock::More => {
            if key == DEFAULT_KEYBINDING.submit.key {
              let filtered = filter_menu_items(&app.more_resources_menu.items, &app.menu_filter);
              let selected_item = app
                .more_resources_menu
                .state
                .selected()
                .and_then(|i| filtered.get(i))
                .map(|(_, item)| (*item).clone());
              if let Some((_title, active_block)) = selected_item {
                app.menu_filter.clear();
                app.menu_filter_active = false;
                app.push_navigation_route(Route {
                  id: RouteId::Home,
                  active_block,
                });
              }
            }
          }
          ActiveBlock::DynamicView => {
            if key == DEFAULT_KEYBINDING.submit.key {
              let filtered = filter_menu_items(&app.dynamic_resources_menu.items, &app.menu_filter);
              let selected_item = app
                .dynamic_resources_menu
                .state
                .selected()
                .and_then(|i| filtered.get(i))
                .map(|(_, item)| (*item).clone());
              if let Some((title, active_block)) = selected_item {
                app.menu_filter.clear();
                app.menu_filter_active = false;
                app.push_navigation_route(Route {
                  id: RouteId::Home,
                  active_block,
                });
                let selected = app.data.dynamic_kinds.iter().find(|it| it.kind == title);
                app.data.selected.dynamic_kind = selected.cloned();
                if !app.apply_cached_dynamic_resources() {
                  app.data.dynamic_resources.set_items(vec![]);
                }
              }
            }
          }
          ActiveBlock::DynamicResource => {
            if let Some(dynamic_res) = app.data.selected.dynamic_kind.as_ref() {
              if let Some(res) = handle_block_action(key, &app.data.dynamic_resources) {
                let describe_action = IoCmdEvent::GetDescribe {
                  kind: dynamic_res.kind.to_owned(),
                  value: res.name.to_owned(),
                  ns: res.namespace.to_owned(),
                };
                let ok = handle_describe_decode_or_yaml_action(
                  key,
                  app,
                  &res,
                  describe_action.clone(),
                )
                .await;
                dispatch_describe_on_submit(key, app, ok, describe_action).await;
              }
            }
          }
          ActiveBlock::Contexts | ActiveBlock::Utilization | ActiveBlock::Troubleshoot | ActiveBlock::Help => { /* Do nothing */ }
        }
      )
    }
    RouteId::Contexts => {
      if key == DEFAULT_KEYBINDING.filter.key {
        if let Some((_, filter_active, _)) = app.current_resource_filter_mut() {
          *filter_active = true;
        }
      } else if let Some(ctx) = handle_block_action(key, &app.data.contexts) {
        app.data.selected.context = Some(ctx.name);
        // Pre-select the namespace from the context if one is configured (#90)
        app.data.selected.ns = ctx.namespace;
        app.refresh();
      }
    }
    RouteId::Utilization => {
      if key == DEFAULT_KEYBINDING.filter.key {
        if let Some((_, filter_active, _)) = app.current_resource_filter_mut() {
          *filter_active = true;
        }
      } else if key == DEFAULT_KEYBINDING.cycle_group_by.key {
        if app.utilization_group_by.len() == 1 {
          app.utilization_group_by = vec![
            GroupBy::resource,
            GroupBy::node,
            GroupBy::namespace,
            GroupBy::pod,
          ];
        } else {
          // keep removing items until just one is left
          app.utilization_group_by.pop();
        }
        app.tick_count = 0; // to force network request
      }
    }
    RouteId::Troubleshoot => {
      if key == DEFAULT_KEYBINDING.filter.key {
        if let Some((_, filter_active, _)) = app.current_resource_filter_mut() {
          *filter_active = true;
          return;
        }
      }

      match app.get_current_route().active_block {
        ActiveBlock::Containers => {
          if key == DEFAULT_KEYBINDING.shell_exec.key {
            queue_selected_container_shell_exec(app);
          } else if key == DEFAULT_KEYBINDING.previous_logs.key {
            handle_previous_logs_for_container(app, RouteId::Troubleshoot).await;
          } else if let Some(c) = handle_block_action(key, &app.data.containers) {
            app.data.selected.container = Some(c.name.clone());
            app
              .dispatch_container_logs(c.name, RouteId::Troubleshoot)
              .await;
          }
        }
        ActiveBlock::Logs => {
          if key == DEFAULT_KEYBINDING.log_auto_scroll.key {
            if app.log_auto_scroll {
              app.data.logs.freeze_follow_position();
            }
            app.log_auto_scroll = !app.log_auto_scroll;
          } else if key == DEFAULT_KEYBINDING.copy_to_clipboard.key {
            copy_to_clipboard(app.data.logs.get_plain_text(), app);
          }
        }
        ActiveBlock::Troubleshoot => {
          if key == DEFAULT_KEYBINDING.submit.key {
            if let Some(finding) = handle_block_action(key, &app.data.troubleshoot_findings) {
              if finding.resource_kind == ResourceKind::Pod {
                // Drill into containers for pod findings
                if let Some(idx) = app.data.pods.items.iter().position(|p| {
                  p.name == finding.resource_name
                    && finding
                      .namespace
                      .as_deref()
                      .is_some_and(|ns| p.namespace == ns)
                }) {
                  let pod = app.data.pods.items[idx].clone();
                  app.data.pods.state.select(Some(idx));
                  app.data.selected.pod = Some(pod.name);
                  app.data.containers.set_items(pod.containers);
                  app.push_navigation_stack(RouteId::Troubleshoot, ActiveBlock::Containers);
                }
              } else {
                // Describe for non-pod findings
                let (kind, value, ns) = finding.describe_target();
                app.data.describe_out = ScrollableTxt::new();
                app.push_navigation_stack(RouteId::Troubleshoot, ActiveBlock::Describe);
                app
                  .dispatch_cmd(IoCmdEvent::GetDescribe {
                    kind,
                    value: value.to_owned(),
                    ns: ns.map(str::to_owned),
                  })
                  .await;
              }
            }
          } else if key == DEFAULT_KEYBINDING.describe_resource.key {
            if let Some(finding) = handle_block_action(key, &app.data.troubleshoot_findings) {
              let (kind, value, ns) = finding.describe_target();
              app.data.describe_out = ScrollableTxt::new();
              app.push_navigation_stack(RouteId::Troubleshoot, ActiveBlock::Describe);
              app
                .dispatch_cmd(IoCmdEvent::GetDescribe {
                  kind,
                  value: value.to_owned(),
                  ns: ns.map(str::to_owned),
                })
                .await;
            }
          } else if key == DEFAULT_KEYBINDING.resource_yaml.key {
            if let Some(finding) = handle_block_action(key, &app.data.troubleshoot_findings) {
              let yaml = match finding.resource_kind {
                ResourceKind::Pod => app
                  .data
                  .pods
                  .items
                  .iter()
                  .find(|p| {
                    p.name == finding.resource_name
                      && finding
                        .namespace
                        .as_deref()
                        .is_some_and(|ns| p.namespace == ns)
                  })
                  .map(|p| p.resource_to_yaml())
                  .unwrap_or_default(),
                ResourceKind::Pvc => app
                  .data
                  .persistent_volume_claims
                  .items
                  .iter()
                  .find(|pvc| {
                    pvc.name == finding.resource_name
                      && finding
                        .namespace
                        .as_deref()
                        .is_some_and(|ns| pvc.namespace == ns)
                  })
                  .map(|pvc| pvc.resource_to_yaml())
                  .unwrap_or_default(),
                ResourceKind::ReplicaSet => app
                  .data
                  .replica_sets
                  .items
                  .iter()
                  .find(|rs| {
                    rs.name == finding.resource_name
                      && finding
                        .namespace
                        .as_deref()
                        .is_some_and(|ns| rs.namespace == ns)
                  })
                  .map(|rs| rs.resource_to_yaml())
                  .unwrap_or_default(),
              };
              app.data.describe_out = ScrollableTxt::with_string(yaml);
              app.push_navigation_stack(RouteId::Troubleshoot, ActiveBlock::Yaml);
            }
          }
        }
        _ => {}
      }
    }
    RouteId::HelpMenu => {
      if key == DEFAULT_KEYBINDING.filter.key {
        if let Some((_, filter_active, _)) = app.current_resource_filter_mut() {
          *filter_active = true;
        }
      }
    }
  }
  // reset tick_count so that network requests are made faster
  if key == DEFAULT_KEYBINDING.submit.key {
    app.tick_count = 0;
  }
}

/// View previous logs for the selected container in the Containers view.
async fn handle_previous_logs_for_container(app: &mut App, route_id: RouteId) {
  let Some(container) = app.data.containers.get_selected_item_copy() else {
    return;
  };
  app.data.selected.container = Some(container.name.clone());
  app.dispatch_previous_logs(container.name, route_id).await;
}

/// View previous logs from the Pods view, resolving the target container the
/// same way shell-exec does (single/selected container).
async fn handle_previous_logs_for_pod(app: &mut App, route_id: RouteId) {
  let Some(pod) = app.data.pods.get_selected_item_copy() else {
    return;
  };
  let Some(container) = resolve_shell_container(app, &pod) else {
    app.handle_error(anyhow!(
      "Open the containers view to pick a container for previous logs on pod {}",
      pod.name
    ));
    return;
  };
  app.data.selected.pod = Some(pod.name.clone());
  app.data.selected.container = Some(container.name.clone());
  app.dispatch_previous_logs(container.name, route_id).await;
}

fn queue_selected_container_shell_exec(app: &mut App) {
  let Some(pod) = app.data.pods.get_selected_item_copy() else {
    app.handle_error(anyhow!("No pod selected for shell exec"));
    return;
  };

  let Some(container) = resolve_shell_container(app, &pod) else {
    app.handle_error(anyhow!(
      "No container selected for shell exec on pod {}",
      pod.name
    ));
    return;
  };

  app.queue_shell_exec(PendingShellExec {
    namespace: pod.namespace,
    pod: pod.name,
    container: container.name,
  });
}

fn resolve_shell_container(
  app: &App,
  pod: &crate::app::pods::KubePod,
) -> Option<crate::app::pods::KubeContainer> {
  if let Some(container) = app.data.containers.get_selected_item_copy() {
    return Some(container);
  }

  if let Some(selected_name) = app.data.selected.container.as_ref() {
    if let Some(container) = pod
      .containers
      .iter()
      .find(|container| container.name == *selected_name)
    {
      return Some(container.clone());
    }
  }

  let mut non_init = pod.containers.iter().filter(|container| !container.init);
  let first_non_init = non_init.next();
  if let Some(container) = first_non_init {
    if non_init.next().is_none() {
      return Some(container.clone());
    }
  }

  if pod.containers.len() == 1 {
    return pod.containers.first().cloned();
  }

  None
}

fn handle_block_action<T: Clone>(key: Key, item: &StatefulTable<T>) -> Option<T> {
  match key {
    _ if key == DEFAULT_KEYBINDING.submit.key
      || key == DEFAULT_KEYBINDING.describe_resource.key
      || key == DEFAULT_KEYBINDING.resource_yaml.key
      || key == DEFAULT_KEYBINDING.decode_secret.key =>
    {
      item.get_selected_item_copy()
    }
    _ => None,
  }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ScrollEvent {
  /// Scroll to an absolute position (negative indices either wrap around or scroll to the top)
  Absolute(isize),
  Relative(isize),
  /// Scroll to the end position (which might not have a known absolute position)
  End,
}

impl ScrollEvent {
  pub fn down() -> Self {
    ScrollEvent::Relative(1)
  }
  pub fn up() -> Self {
    ScrollEvent::Relative(-1)
  }
}

async fn handle_block_scroll(app: &mut App, event: ScrollEvent, is_mouse: bool) {
  handle_resource_scroll!(app.get_current_route().active_block, app, event, is_mouse,
    [
      (ActiveBlock::Namespaces, namespaces),
      (ActiveBlock::Pods, pods),
      (ActiveBlock::Containers, containers),
      (ActiveBlock::Services, services),
      (ActiveBlock::Nodes, nodes),
      (ActiveBlock::ConfigMaps, config_maps),
      (ActiveBlock::StatefulSets, stateful_sets),
      (ActiveBlock::ReplicaSets, replica_sets),
      (ActiveBlock::Deployments, deployments),
      (ActiveBlock::Jobs, jobs),
      (ActiveBlock::DaemonSets, daemon_sets),
      (ActiveBlock::CronJobs, cronjobs),
      (ActiveBlock::Secrets, secrets),
      (ActiveBlock::ReplicationControllers, replication_controllers),
      (ActiveBlock::StorageClasses, storage_classes),
      (ActiveBlock::Roles, roles),
      (ActiveBlock::RoleBindings, role_bindings),
      (ActiveBlock::ClusterRoles, cluster_roles),
      (ActiveBlock::ClusterRoleBindings, cluster_role_bindings),
      (ActiveBlock::PersistentVolumeClaims, persistent_volume_claims),
      (ActiveBlock::PersistentVolumes, persistent_volumes),
      (ActiveBlock::Ingresses, ingress),
      (ActiveBlock::ServiceAccounts, service_accounts),
      (ActiveBlock::Events, events),
      (ActiveBlock::NetworkPolicies, network_policies),
      (ActiveBlock::DynamicResource, dynamic_resources),
    ],
    extra: {
      ActiveBlock::Contexts => app.data.contexts.handle_scroll(event),
      ActiveBlock::Utilization => app.data.metrics.handle_scroll(event),
      ActiveBlock::Troubleshoot => app.data.troubleshoot_findings.handle_scroll(event),
      ActiveBlock::Help => {
        // Grouped help is a scrolled paragraph; the offset is clamped to the
        // content height at render time.
        app.help_scroll = match event {
          ScrollEvent::Absolute(n) => n.clamp(0, u16::MAX as isize) as u16,
          ScrollEvent::Relative(delta) if delta < 0 => {
            app.help_scroll.saturating_sub(delta.unsigned_abs() as u16)
          }
          ScrollEvent::Relative(delta) => app.help_scroll.saturating_add(delta as u16),
          ScrollEvent::End => u16::MAX,
        };
      }
      ActiveBlock::More => {
        let filtered_len = filter_menu_items(&app.more_resources_menu.items, &app.menu_filter).len();
        handle_menu_scroll(&mut app.more_resources_menu, event, filtered_len);
      }
      ActiveBlock::DynamicView => {
        let filtered_len = filter_menu_items(&app.dynamic_resources_menu.items, &app.menu_filter).len();
        handle_menu_scroll(&mut app.dynamic_resources_menu, event, filtered_len);
      }
      ActiveBlock::Logs => {
        if app.log_auto_scroll {
          app.data.logs.freeze_follow_position();
          app.log_auto_scroll = false;
        }
        app.data.logs.handle_scroll(inverse_dir(event, is_mouse));
      }
      ActiveBlock::Describe | ActiveBlock::Yaml => app
        .data
        .describe_out
        .handle_scroll(inverse_dir(event, is_mouse)),
    }
  )
}

/// Scroll within a menu, respecting filtered item count
fn handle_menu_scroll(
  menu: &mut StatefulList<(String, ActiveBlock)>,
  event: ScrollEvent,
  filtered_len: usize,
) {
  if filtered_len == 0 {
    return;
  }

  // duplicated because we wrap at filtered_len, not total len
  let newpos = match event {
    ScrollEvent::Absolute(newpos) => newpos,
    ScrollEvent::Relative(delta) => menu.current_pos().unwrap_or(0) as isize + delta,
    ScrollEvent::End => filtered_len as isize - 1,
  }
  .rem_euclid(filtered_len as isize);
  menu.state.select(Some(newpos as usize));
}

fn copy_to_clipboard(content: String, app: &mut App) {
  use std::thread;

  use anyhow::anyhow;
  use copypasta::{ClipboardContext, ClipboardProvider};

  match ClipboardContext::new() {
    Ok(mut ctx) => match ctx.set_contents(content) {
      // without this sleep the clipboard is not set in some OSes
      Ok(_) => thread::sleep(std::time::Duration::from_millis(100)),
      Err(_) => app.handle_error(anyhow!("Unable to set clipboard contents".to_string())),
    },
    Err(err) => {
      app.handle_error(anyhow!("Unable to obtain clipboard: {}", err));
    }
  };
}

fn dump_error_history(app: &mut App, output_dir: Option<&Path>) {
  match write_error_history_file(&app.error_history, output_dir) {
    Ok(path) => app.set_status_message(format!("Saved recent errors to {}", path.display())),
    Err(error) => app.handle_error(anyhow::anyhow!("Unable to write error log: {}", error)),
  }
}

fn write_error_history_file(
  history: &std::collections::VecDeque<crate::app::ErrorRecord>,
  output_dir: Option<&Path>,
) -> std::io::Result<PathBuf> {
  let dir = match output_dir {
    Some(path) => path.to_path_buf(),
    None => std::env::current_dir()?,
  };

  let path = dir.join(format!(
    "kdash-errors-{}.log",
    chrono::Local::now().format("%Y%m%d%H%M%S")
  ));

  fs::write(&path, format_error_history(history))?;
  Ok(path)
}

fn format_error_history(history: &std::collections::VecDeque<crate::app::ErrorRecord>) -> String {
  if history.is_empty() {
    "No errors recorded\n".to_owned()
  } else {
    let mut rendered = history
      .iter()
      .map(|record| format!("[{}] {}", record.timestamp, record.message))
      .collect::<Vec<_>>()
      .join("\n");
    rendered.push('\n');
    rendered
  }
}

/// inverse direction for natural scrolling on mouse and keyboard
fn inverse_dir(event: ScrollEvent, is_mouse: bool) -> ScrollEvent {
  match event {
    ScrollEvent::Relative(delta) if is_mouse => ScrollEvent::Relative(-delta),
    other => other,
  }
}

#[cfg(test)]
mod tests {
  use crossterm::event::KeyCode;
  use k8s_openapi::ByteString;
  use kube::{
    api::ObjectMeta,
    core::{ApiResource, DynamicObject},
    discovery::Scope,
  };
  use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
  };
  use tokio::sync::mpsc;

  use super::*;
  use crate::app::{
    contexts::KubeContext,
    dynamic::{dynamic_cache_key, KubeDynamicKind, KubeDynamicResource},
    pods::{KubeContainer, KubePod},
    PendingShellExec,
  };

  #[test]
  fn test_inverse_dir() {
    assert_eq!(inverse_dir(ScrollEvent::down(), false), ScrollEvent::down());
    assert_eq!(inverse_dir(ScrollEvent::down(), true), ScrollEvent::up());
  }

  fn temp_test_dir(name: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_nanos();
    let path = std::env::temp_dir().join(format!("kdash-{name}-{suffix}"));
    fs::create_dir_all(&path).expect("temp test dir should be created");
    path
  }

  #[test]
  fn test_write_error_history_file_writes_recent_errors() {
    let dir = temp_test_dir("error-dump");
    let mut app = App::default();
    app.record_error("first error".into());
    app.record_error("second error".into());

    let path = write_error_history_file(&app.error_history, Some(&dir)).unwrap();
    let contents = fs::read_to_string(path).unwrap();

    assert!(contents.contains("first error"));
    assert!(contents.contains("second error"));
  }

  #[test]
  fn test_write_error_history_file_writes_empty_message_when_no_errors() {
    let dir = temp_test_dir("empty-error-dump");
    let app = App::default();

    let path = write_error_history_file(&app.error_history, Some(&dir)).unwrap();
    let contents = fs::read_to_string(path).unwrap();

    assert_eq!(contents, "No errors recorded\n");
  }

  #[tokio::test]
  async fn test_dump_error_key_creates_file_and_sets_status_message() {
    let dir = temp_test_dir("dump-key");
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();

    let mut app = App::default();
    app.record_error("boom".into());

    let key_evt = KeyEvent {
      code: KeyCode::Char('D'),
      modifiers: crossterm::event::KeyModifiers::SHIFT,
      kind: crossterm::event::KeyEventKind::Press,
      state: crossterm::event::KeyEventState::NONE,
    };
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    let created_files = fs::read_dir(&dir)
      .unwrap()
      .map(|entry| entry.unwrap().file_name().into_string().unwrap())
      .collect::<Vec<_>>();

    std::env::set_current_dir(original_dir).unwrap();

    assert!(created_files
      .iter()
      .any(|name| name.starts_with("kdash-errors-") && name.ends_with(".log")));
    assert!(app.api_error.is_empty());
    assert!(app.status_message.text().contains("Saved recent errors to"));
  }

  #[tokio::test]
  async fn test_shell_exec_key_in_containers_queues_request() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Containers);
    let mut pod = KubePod::default();
    pod.namespace = "team-a".into();
    pod.name = "pod-1".into();
    app.data.pods.set_items(vec![pod]);
    let mut container = KubeContainer::default();
    container.name = "app".into();
    app.data.containers.set_items(vec![container]);

    let key_evt = KeyEvent::from(KeyCode::Char('s'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    assert_eq!(
      app.pending_shell_exec(),
      Some(&PendingShellExec {
        namespace: "team-a".into(),
        pod: "pod-1".into(),
        container: "app".into(),
      })
    );
    assert!(app.api_error.is_empty());
  }

  #[tokio::test]
  async fn test_shell_exec_key_in_containers_requires_selected_container() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Containers);
    let mut pod = KubePod::default();
    pod.namespace = "team-a".into();
    pod.name = "pod-1".into();
    app.data.pods.set_items(vec![pod]);

    let key_evt = KeyEvent::from(KeyCode::Char('s'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    assert_eq!(app.pending_shell_exec(), None);
    assert_eq!(
      app.api_error,
      "No container selected for shell exec on pod pod-1"
    );
  }

  #[tokio::test]
  async fn test_shell_exec_key_in_containers_uses_selected_container_fallback() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Containers);
    let mut pod = KubePod::default();
    pod.namespace = "team-a".into();
    pod.name = "pod-1".into();
    let mut sidecar = KubeContainer::default();
    sidecar.name = "sidecar".into();
    sidecar.pod_name = "pod-1".into();
    let mut app_container = KubeContainer::default();
    app_container.name = "app".into();
    app_container.pod_name = "pod-1".into();
    pod.containers = vec![app_container.clone(), sidecar];
    app.data.pods.set_items(vec![pod]);
    app.data.selected.container = Some("app".into());
    app.data.containers.items.clear();

    let key_evt = KeyEvent::from(KeyCode::Char('s'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    assert_eq!(
      app.pending_shell_exec(),
      Some(&PendingShellExec {
        namespace: "team-a".into(),
        pod: "pod-1".into(),
        container: "app".into(),
      })
    );
    assert!(app.api_error.is_empty());
  }

  #[tokio::test]
  async fn test_shell_exec_key_in_containers_uses_single_non_init_container_fallback() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Containers);
    let mut pod = KubePod::default();
    pod.namespace = "team-a".into();
    pod.name = "pod-1".into();
    let mut app_container = KubeContainer::default();
    app_container.name = "app".into();
    app_container.pod_name = "pod-1".into();
    let mut init_container = KubeContainer::default();
    init_container.name = "init-db".into();
    init_container.pod_name = "pod-1".into();
    init_container.init = true;
    pod.containers = vec![app_container, init_container];
    app.data.pods.set_items(vec![pod]);
    app.data.containers.items.clear();

    let key_evt = KeyEvent::from(KeyCode::Char('s'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    assert_eq!(
      app.pending_shell_exec(),
      Some(&PendingShellExec {
        namespace: "team-a".into(),
        pod: "pod-1".into(),
        container: "app".into(),
      })
    );
    assert!(app.api_error.is_empty());
  }

  #[tokio::test]
  async fn test_shell_exec_key_in_containers_requires_selection_when_multiple_main_containers() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Containers);
    let mut pod = KubePod::default();
    pod.namespace = "team-a".into();
    pod.name = "pod-1".into();
    let mut app_container = KubeContainer::default();
    app_container.name = "app".into();
    app_container.pod_name = "pod-1".into();
    let mut sidecar = KubeContainer::default();
    sidecar.name = "sidecar".into();
    sidecar.pod_name = "pod-1".into();
    pod.containers = vec![app_container, sidecar];
    app.data.pods.set_items(vec![pod]);
    app.data.containers.items.clear();

    let key_evt = KeyEvent::from(KeyCode::Char('s'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    assert_eq!(app.pending_shell_exec(), None);
    assert_eq!(
      app.api_error,
      "No container selected for shell exec on pod pod-1"
    );
  }

  #[tokio::test]
  async fn test_shell_exec_key_in_containers_requires_selected_pod() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Containers);
    let mut container = KubeContainer::default();
    container.name = "app".into();
    app.data.containers.set_items(vec![container]);

    let key_evt = KeyEvent::from(KeyCode::Char('s'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    assert_eq!(app.pending_shell_exec(), None);
    assert_eq!(app.api_error, "No pod selected for shell exec");
  }

  #[tokio::test]
  async fn test_shell_exec_key_does_not_replace_log_auto_scroll_in_logs() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Logs);
    assert!(app.log_auto_scroll);

    let key_evt = KeyEvent::from(KeyCode::Char('s'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    assert!(!app.log_auto_scroll);
    assert_eq!(app.pending_shell_exec(), None);
  }

  #[tokio::test]
  async fn test_shell_exec_key_in_troubleshoot_containers_queues_request() {
    let mut app = App::default();
    app.route_troubleshoot();
    app.push_navigation_stack(RouteId::Troubleshoot, ActiveBlock::Containers);

    let mut pod = KubePod::default();
    pod.namespace = "team-a".into();
    pod.name = "pod-1".into();
    app.data.pods.set_items(vec![pod]);

    let mut container = KubeContainer::default();
    container.name = "app".into();
    app.data.containers.set_items(vec![container]);

    let key_evt = KeyEvent::from(KeyCode::Char('s'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    assert_eq!(
      app.pending_shell_exec(),
      Some(&PendingShellExec {
        namespace: "team-a".into(),
        pod: "pod-1".into(),
        container: "app".into(),
      })
    );
    assert!(app.api_error.is_empty());
  }

  #[tokio::test]
  async fn test_resource_filter_key_flow() {
    let mut app = App::default();
    app.route_home();
    assert!(!app.data.pods.filter_active);
    assert!(app.data.pods.filter.is_empty());

    let key_evt = KeyEvent::from(KeyCode::Char('/'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(app.data.pods.filter_active);

    for c in ['w', 'e', 'b'] {
      let key_evt = KeyEvent::from(KeyCode::Char(c));
      handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    }
    assert_eq!(app.data.pods.filter, "web");

    let key_evt = KeyEvent::from(KeyCode::Backspace);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.data.pods.filter, "we");

    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(app.data.pods.filter_active);
    assert!(app.data.pods.filter.is_empty());

    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(!app.data.pods.filter_active);
  }

  #[tokio::test]
  async fn test_containers_filter_key_flow() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Containers);
    assert!(!app.data.containers.filter_active);
    assert!(app.data.containers.filter.is_empty());

    let key_evt = KeyEvent::from(KeyCode::Char('/'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(app.data.containers.filter_active);

    for c in ['n', 'g', 'i', 'n', 'x'] {
      let key_evt = KeyEvent::from(KeyCode::Char(c));
      handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    }
    assert_eq!(app.data.containers.filter, "nginx");

    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(app.data.containers.filter_active);
    assert!(app.data.containers.filter.is_empty());

    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(!app.data.containers.filter_active);
  }

  #[tokio::test]
  async fn test_action_menu_opens_and_closes_on_pods() {
    use crate::app::actions::ResourceAction;

    let mut app = App::default();
    app.route_home();

    let m = KeyEvent::from(KeyCode::Char('m'));
    handle_key_events(Key::from(m), m, &mut app).await;

    let menu = app.action_menu.as_ref().expect("action menu should open");
    assert_eq!(
      menu.items,
      vec![
        ResourceAction::Describe,
        ResourceAction::Yaml,
        ResourceAction::Logs,
        ResourceAction::PreviousLogs,
        ResourceAction::Delete
      ]
    );

    let esc = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(esc), esc, &mut app).await;
    assert!(app.action_menu.is_none());
  }

  #[tokio::test]
  async fn test_action_menu_not_opened_on_blocks_without_actions() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::More);

    let m = KeyEvent::from(KeyCode::Char('m'));
    handle_key_events(Key::from(m), m, &mut app).await;

    assert!(app.action_menu.is_none());
  }

  #[tokio::test]
  async fn test_action_menu_swallows_unrelated_keys_while_open() {
    let mut app = App::default();
    app.route_home();

    let m = KeyEvent::from(KeyCode::Char('m'));
    handle_key_events(Key::from(m), m, &mut app).await;
    assert!(app.action_menu.is_some());
    let block_before = app.get_current_route().active_block;

    // A tab-switch key must be swallowed by the menu, not change the view.
    let right = KeyEvent::from(KeyCode::Right);
    handle_key_events(Key::from(right), right, &mut app).await;

    assert!(app.action_menu.is_some());
    assert_eq!(app.get_current_route().active_block, block_before);
  }

  #[tokio::test]
  async fn test_action_menu_shell_replays_shell_exec_in_containers() {
    use crate::app::actions::ResourceAction;

    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Containers);

    let mut pod = KubePod::default();
    pod.namespace = "team-a".into();
    pod.name = "pod-1".into();
    app.data.pods.set_items(vec![pod]);
    let mut container = KubeContainer::default();
    container.name = "app".into();
    app.data.containers.set_items(vec![container]);

    let m = KeyEvent::from(KeyCode::Char('m'));
    handle_key_events(Key::from(m), m, &mut app).await;
    assert_eq!(
      app.action_menu.as_ref().unwrap().items,
      vec![
        ResourceAction::Logs,
        ResourceAction::PreviousLogs,
        ResourceAction::Shell
      ]
    );

    // Navigate to Shell (index 2) and select it.
    for _ in 0..2 {
      let down = KeyEvent::from(KeyCode::Down);
      handle_key_events(Key::from(down), down, &mut app).await;
    }
    let enter = KeyEvent::from(KeyCode::Enter);
    handle_key_events(Key::from(enter), enter, &mut app).await;

    assert!(app.action_menu.is_none());
    assert_eq!(
      app.pending_shell_exec(),
      Some(&PendingShellExec {
        namespace: "team-a".into(),
        pod: "pod-1".into(),
        container: "app".into(),
      })
    );
  }

  fn ctrl_key(c: char) -> KeyEvent {
    KeyEvent {
      code: KeyCode::Char(c),
      modifiers: crossterm::event::KeyModifiers::CONTROL,
      kind: crossterm::event::KeyEventKind::Press,
      state: crossterm::event::KeyEventState::NONE,
    }
  }

  fn make_node(name: &str, unschedulable: bool) -> crate::app::nodes::KubeNode {
    use k8s_openapi::api::core::v1::{Node, NodeSpec, Pod};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ListMeta, ObjectMeta};
    use kube::{api::ObjectList, core::TypeMeta};

    let node = Node {
      metadata: ObjectMeta {
        name: Some(name.into()),
        ..Default::default()
      },
      spec: Some(NodeSpec {
        unschedulable: Some(unschedulable),
        ..Default::default()
      }),
      ..Default::default()
    };
    let pods = ObjectList::<Pod> {
      types: TypeMeta {
        api_version: "v1".into(),
        kind: "List".into(),
      },
      metadata: ListMeta::default(),
      items: vec![],
    };
    let seed = tokio::sync::Mutex::new(App::default());
    let mut guard = seed.try_lock().expect("uncontended lock");
    crate::app::nodes::KubeNode::from_api_with_pods(&node, &pods, &mut guard)
  }

  async fn open_menu_and_select(app: &mut App, steps: usize) {
    let m = KeyEvent::from(KeyCode::Char('m'));
    handle_key_events(Key::from(m), m, app).await;
    for _ in 0..steps {
      let down = KeyEvent::from(KeyCode::Down);
      handle_key_events(Key::from(down), down, app).await;
    }
    let enter = KeyEvent::from(KeyCode::Enter);
    handle_key_events(Key::from(enter), enter, app).await;
  }

  /// Drive a whole key sequence through one loop. `handle_key_events` produces a
  /// large async future, and a separate `.await` per key inflates the caller's
  /// future enough to overflow Windows' 1 MB test stack; a single await point
  /// inside this loop keeps it small.
  async fn send_keys(app: &mut App, codes: &[KeyCode]) {
    for &code in codes {
      let evt = KeyEvent::from(code);
      handle_key_events(Key::from(evt), evt, app).await;
    }
  }

  #[tokio::test]
  async fn test_menu_cordon_on_schedulable_node_confirms_cordon() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Nodes);
    app.data.nodes.set_items(vec![make_node("n1", false)]);

    // Nodes menu: Describe, YAML, Cordon, Delete → Cordon at index 2.
    open_menu_and_select(&mut app, 2).await;

    let modal = app
      .modal
      .as_ref()
      .expect("cordon should open a confirm modal");
    assert!(modal.prompt.contains("Cordon node 'n1'"));
    assert_eq!(
      modal.on_confirm,
      IoEvent::PatchResource {
        block: ActiveBlock::Nodes,
        name: "n1".into(),
        namespace: None,
        patch: ResourcePatch::SetUnschedulable(true),
      }
    );
  }

  #[tokio::test]
  async fn test_menu_cordon_on_cordoned_node_confirms_uncordon() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Nodes);
    app.data.nodes.set_items(vec![make_node("n1", true)]);

    open_menu_and_select(&mut app, 2).await;

    let modal = app
      .modal
      .as_ref()
      .expect("uncordon should open a confirm modal");
    assert!(modal.prompt.contains("Uncordon node 'n1'"));
    assert_eq!(
      modal.on_confirm,
      IoEvent::PatchResource {
        block: ActiveBlock::Nodes,
        name: "n1".into(),
        namespace: None,
        patch: ResourcePatch::SetUnschedulable(false),
      }
    );
  }

  #[tokio::test]
  async fn test_delete_key_opens_confirm_modal_with_correct_event() {
    let mut app = App::default();
    app.route_home();
    let mut pod = KubePod::default();
    pod.namespace = "team-a".into();
    pod.name = "pod-1".into();
    app.data.pods.set_items(vec![pod]);

    let ctrl_d = ctrl_key('d');
    handle_key_events(Key::from(ctrl_d), ctrl_d, &mut app).await;

    let modal = app
      .modal
      .as_ref()
      .expect("delete should open a confirm modal");
    assert_eq!(
      modal.on_confirm,
      IoEvent::DeleteResource {
        block: ActiveBlock::Pods,
        name: "pod-1".into(),
        namespace: Some("team-a".into()),
      }
    );
    assert!(modal.prompt.contains("pod-1"));
    assert!(modal.prompt.contains("team-a"));
  }

  #[tokio::test]
  async fn test_delete_key_is_noop_without_selected_row() {
    let mut app = App::default();
    app.route_home();
    // No pods set → nothing selected.

    let ctrl_d = ctrl_key('d');
    handle_key_events(Key::from(ctrl_d), ctrl_d, &mut app).await;

    assert!(app.modal.is_none());
  }

  #[tokio::test]
  async fn test_modal_cancel_closes_without_dispatch() {
    let mut app = App::default();
    app.route_home();
    let mut pod = KubePod::default();
    pod.namespace = "team-a".into();
    pod.name = "pod-1".into();
    app.data.pods.set_items(vec![pod]);

    let ctrl_d = ctrl_key('d');
    handle_key_events(Key::from(ctrl_d), ctrl_d, &mut app).await;
    assert!(app.modal.is_some());

    let n = KeyEvent::from(KeyCode::Char('n'));
    handle_key_events(Key::from(n), n, &mut app).await;
    assert!(app.modal.is_none());
  }

  #[tokio::test]
  async fn test_modal_confirm_clears_modal() {
    let mut app = App::default();
    app.route_home();
    let mut pod = KubePod::default();
    pod.namespace = "team-a".into();
    pod.name = "pod-1".into();
    app.data.pods.set_items(vec![pod]);

    let ctrl_d = ctrl_key('d');
    handle_key_events(Key::from(ctrl_d), ctrl_d, &mut app).await;
    assert!(app.modal.is_some());

    let y = KeyEvent::from(KeyCode::Char('y'));
    handle_key_events(Key::from(y), y, &mut app).await;
    assert!(app.modal.is_none());
  }

  #[tokio::test]
  async fn test_action_menu_delete_entry_opens_confirm_modal() {
    let mut app = App::default();
    app.route_home();
    let mut pod = KubePod::default();
    pod.namespace = "team-a".into();
    pod.name = "pod-1".into();
    app.data.pods.set_items(vec![pod]);

    // Open the action menu and move to the Delete entry
    // (Describe, YAML, Logs, Previous logs, Delete → index 4).
    let m = KeyEvent::from(KeyCode::Char('m'));
    handle_key_events(Key::from(m), m, &mut app).await;
    for _ in 0..4 {
      let down = KeyEvent::from(KeyCode::Down);
      handle_key_events(Key::from(down), down, &mut app).await;
    }
    let enter = KeyEvent::from(KeyCode::Enter);
    handle_key_events(Key::from(enter), enter, &mut app).await;

    assert!(app.action_menu.is_none());
    let modal = app
      .modal
      .as_ref()
      .expect("menu delete should open confirm modal");
    assert_eq!(
      modal.on_confirm,
      IoEvent::DeleteResource {
        block: ActiveBlock::Pods,
        name: "pod-1".into(),
        namespace: Some("team-a".into()),
      }
    );
  }

  #[tokio::test]
  async fn test_restart_key_opens_confirm_for_deployment() {
    use crate::app::deployments::KubeDeployment;
    use k8s_openapi::api::apps::v1::Deployment;

    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Deployments);
    let mut dep = KubeDeployment::from(Deployment::default());
    dep.name = "web".into();
    dep.namespace = "team-a".into();
    app.data.deployments.set_items(vec![dep]);

    let r = KeyEvent::from(KeyCode::Char('r'));
    handle_key_events(Key::from(r), r, &mut app).await;

    let modal = app
      .modal
      .as_ref()
      .expect("restart should open a confirm modal");
    assert_eq!(
      modal.on_confirm,
      IoEvent::PatchResource {
        block: ActiveBlock::Deployments,
        name: "web".into(),
        namespace: Some("team-a".into()),
        patch: ResourcePatch::RolloutRestart,
      }
    );
  }

  fn deployment_with_replicas(
    name: &str,
    ns: &str,
    replicas: Option<i32>,
  ) -> crate::app::deployments::KubeDeployment {
    use crate::app::deployments::KubeDeployment;
    use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec};

    let dep = Deployment {
      spec: replicas.map(|r| DeploymentSpec {
        replicas: Some(r),
        ..Default::default()
      }),
      ..Default::default()
    };
    let mut dep = KubeDeployment::from(dep);
    dep.name = name.into();
    dep.namespace = ns.into();
    dep
  }

  #[tokio::test]
  async fn test_menu_scale_deployment_opens_input_prefilled() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Deployments);
    app
      .data
      .deployments
      .set_items(vec![deployment_with_replicas("web", "team-a", Some(2))]);

    // Deployments menu: Describe, YAML, Logs, Restart, Scale, Delete → Scale at index 4.
    open_menu_and_select(&mut app, 4).await;

    let input = app
      .input_modal
      .as_ref()
      .expect("scale should open an input modal");
    // Prefilled with the current replica count.
    assert_eq!(input.buffer, "2");
    assert_eq!(
      input.action,
      InputAction::Scale {
        block: ActiveBlock::Deployments,
        name: "web".into(),
        namespace: Some("team-a".into()),
        kind: "deployment".into(),
      }
    );
    // No confirm modal yet — only the input overlay.
    assert!(app.modal.is_none());
  }

  #[tokio::test]
  async fn test_scale_input_submit_chains_to_confirm_modal() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Deployments);
    app
      .data
      .deployments
      .set_items(vec![deployment_with_replicas("web", "team-a", Some(2))]);

    // Menu: Describe, YAML, Logs, Restart, Scale, Delete → Scale at index 4.
    // Open the input modal, replace the prefilled "2" with "5", then submit.
    send_keys(
      &mut app,
      &[
        KeyCode::Char('m'),
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Enter,
        KeyCode::Backspace,
        KeyCode::Char('5'),
        KeyCode::Enter,
      ],
    )
    .await;

    // Input modal closed, confirm modal opened with the scale patch.
    assert!(app.input_modal.is_none());
    let modal = app
      .modal
      .as_ref()
      .expect("valid scale submit should chain into a confirm modal");
    assert!(modal.prompt.contains("to 5 replica(s)"));
    assert_eq!(
      modal.on_confirm,
      IoEvent::PatchResource {
        block: ActiveBlock::Deployments,
        name: "web".into(),
        namespace: Some("team-a".into()),
        patch: ResourcePatch::SetReplicas(5),
      }
    );
  }

  #[tokio::test]
  async fn test_scale_input_invalid_keeps_modal_open_with_error() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Deployments);
    app
      .data
      .deployments
      .set_items(vec![deployment_with_replicas("web", "team-a", Some(2))]);

    // Open the scale input modal (Scale at index 4), type a non-numeric value
    // and submit.
    send_keys(
      &mut app,
      &[
        KeyCode::Char('m'),
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Enter,
        KeyCode::Char('x'),
        KeyCode::Enter,
      ],
    )
    .await;

    // Stays open with an inline error; no confirm modal, no dispatch.
    let input = app
      .input_modal
      .as_ref()
      .expect("invalid input keeps the modal open");
    assert!(input.error.is_some());
    assert!(app.modal.is_none());
  }

  #[tokio::test]
  async fn test_scale_input_esc_cancels() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Deployments);
    app
      .data
      .deployments
      .set_items(vec![deployment_with_replicas("web", "team-a", Some(2))]);

    // Open the scale input modal (Scale at index 4), then cancel with Esc.
    send_keys(
      &mut app,
      &[
        KeyCode::Char('m'),
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Enter,
        KeyCode::Esc,
      ],
    )
    .await;

    assert!(app.input_modal.is_none());
    assert!(app.modal.is_none());
  }

  #[tokio::test]
  async fn test_restart_key_is_noop_on_non_workload_block() {
    let mut app = App::default();
    app.route_home();
    let mut pod = KubePod::default();
    pod.name = "p".into();
    pod.namespace = "n".into();
    app.data.pods.set_items(vec![pod]);

    let r = KeyEvent::from(KeyCode::Char('r'));
    handle_key_events(Key::from(r), r, &mut app).await;

    assert!(app.modal.is_none());
  }

  #[tokio::test]
  async fn test_menu_suspend_cronjob_confirms_suspend() {
    use crate::app::cronjobs::KubeCronJob;
    use k8s_openapi::api::batch::v1::CronJob;

    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::CronJobs);
    let mut cronjob = KubeCronJob::from(CronJob::default());
    cronjob.name = "backup".into();
    cronjob.namespace = "default".into();
    cronjob.suspend = false;
    app.data.cronjobs.set_items(vec![cronjob]);

    // CronJobs menu: Describe, YAML, Logs, Suspend, Trigger, Delete → Suspend at index 3.
    open_menu_and_select(&mut app, 3).await;

    let modal = app
      .modal
      .as_ref()
      .expect("suspend should open a confirm modal");
    assert_eq!(
      modal.on_confirm,
      IoEvent::PatchResource {
        block: ActiveBlock::CronJobs,
        name: "backup".into(),
        namespace: Some("default".into()),
        patch: ResourcePatch::SetSuspend(true),
      }
    );
  }

  #[tokio::test]
  async fn test_menu_trigger_cronjob_confirms_trigger() {
    use crate::app::cronjobs::KubeCronJob;
    use k8s_openapi::api::batch::v1::CronJob;

    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::CronJobs);
    let mut cronjob = KubeCronJob::from(CronJob::default());
    cronjob.name = "backup".into();
    cronjob.namespace = "default".into();
    app.data.cronjobs.set_items(vec![cronjob]);

    // CronJobs menu: Describe, YAML, Logs, Suspend, Trigger, Delete → Trigger at index 4.
    open_menu_and_select(&mut app, 4).await;

    let modal = app
      .modal
      .as_ref()
      .expect("trigger should open a confirm modal");
    assert_eq!(
      modal.on_confirm,
      IoEvent::TriggerCronJob {
        name: "backup".into(),
        namespace: "default".into(),
      }
    );
  }

  #[tokio::test]
  async fn test_previous_logs_key_in_containers_opens_previous_log_view() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Containers);
    let mut pod = KubePod::default();
    pod.namespace = "team-a".into();
    pod.name = "pod-1".into();
    app.data.pods.set_items(vec![pod]);
    let mut container = KubeContainer::default();
    container.name = "app".into();
    app.data.containers.set_items(vec![container]);

    let p = KeyEvent::from(KeyCode::Char('p'));
    handle_key_events(Key::from(p), p, &mut app).await;

    assert_eq!(app.get_current_route().active_block, ActiveBlock::Logs);
    assert!(app.log_previous);
    assert_eq!(app.data.selected.container.as_deref(), Some("app"));
    // The log id must match the selected container so the log view's render
    // guard (`container == logs.id`) shows the fetched previous logs.
    assert_eq!(app.data.logs.id, "app");
  }

  #[tokio::test]
  async fn test_leaving_previous_logs_resets_flag() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Containers);
    let mut pod = KubePod::default();
    pod.namespace = "team-a".into();
    pod.name = "pod-1".into();
    app.data.pods.set_items(vec![pod]);
    let mut container = KubeContainer::default();
    container.name = "app".into();
    app.data.containers.set_items(vec![container]);

    let p = KeyEvent::from(KeyCode::Char('p'));
    handle_key_events(Key::from(p), p, &mut app).await;
    assert!(app.log_previous);

    let esc = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(esc), esc, &mut app).await;
    assert!(!app.log_previous);
    assert_eq!(
      app.get_current_route().active_block,
      ActiveBlock::Containers
    );
  }

  #[tokio::test]
  async fn test_tab_switch_deactivates_resource_filter_but_preserves_text() {
    let mut app = App::default();
    app.route_home();
    app.data.pods.filter = "web".into();
    app.data.pods.filter_active = true;

    let key_evt = KeyEvent::from(KeyCode::Right);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    assert_eq!(app.data.pods.filter, "web");
    assert!(!app.data.pods.filter_active);
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Services);
  }

  #[tokio::test]
  async fn test_namespace_filter_key_flow() {
    let mut app = App::default();
    app.route_home();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Namespaces);

    let key_evt = KeyEvent::from(KeyCode::Char('/'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(app.ns_filter_active);

    for c in ['p', 'r', 'o', 'd'] {
      let key_evt = KeyEvent::from(KeyCode::Char(c));
      handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    }
    assert_eq!(app.ns_filter, "prod");

    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(app.ns_filter_active);
    assert!(app.ns_filter.is_empty());

    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(!app.ns_filter_active);
  }

  #[tokio::test]
  async fn test_contexts_filter_key_flow() {
    let mut app = App::default();
    app.route_contexts();
    assert!(!app.data.contexts.filter_active);
    assert!(app.data.contexts.filter.is_empty());

    let key_evt = KeyEvent::from(KeyCode::Char('/'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(app.data.contexts.filter_active);

    for c in ['p', 'r', 'o', 'd'] {
      let key_evt = KeyEvent::from(KeyCode::Char(c));
      handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    }
    assert_eq!(app.data.contexts.filter, "prod");

    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(app.data.contexts.filter_active);
    assert!(app.data.contexts.filter.is_empty());

    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(!app.data.contexts.filter_active);
  }

  #[tokio::test]
  async fn test_utilization_filter_key_flow() {
    let mut app = App::default();
    app.route_utilization();
    assert!(!app.data.metrics.filter_active);
    assert!(app.data.metrics.filter.is_empty());

    let key_evt = KeyEvent::from(KeyCode::Char('/'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(app.data.metrics.filter_active);

    for c in ['c', 'p', 'u'] {
      let key_evt = KeyEvent::from(KeyCode::Char(c));
      handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    }
    assert_eq!(app.data.metrics.filter, "cpu");

    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(app.data.metrics.filter_active);
    assert!(app.data.metrics.filter.is_empty());

    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(!app.data.metrics.filter_active);
  }

  #[tokio::test]
  async fn test_troubleshoot_filter_key_flow() {
    let mut app = App::default();
    app.route_troubleshoot();
    assert!(!app.data.troubleshoot_findings.filter_active);
    assert!(app.data.troubleshoot_findings.filter.is_empty());

    let key_evt = KeyEvent::from(KeyCode::Char('/'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(app.data.troubleshoot_findings.filter_active);

    for c in ['p', 'o', 'd'] {
      let key_evt = KeyEvent::from(KeyCode::Char(c));
      handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    }
    assert_eq!(app.data.troubleshoot_findings.filter, "pod");

    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(app.data.troubleshoot_findings.filter_active);
    assert!(app.data.troubleshoot_findings.filter.is_empty());

    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(!app.data.troubleshoot_findings.filter_active);
  }

  #[tokio::test]
  async fn test_help_scroll_keys_adjust_offset() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::HelpMenu, ActiveBlock::Help);
    assert_eq!(app.help_scroll, 0);

    // Down scrolls; clamped-to-zero saturating-sub on up.
    let down = KeyEvent::from(KeyCode::Down);
    handle_key_events(Key::from(down), down, &mut app).await;
    assert_eq!(app.help_scroll, 1);

    let up = KeyEvent::from(KeyCode::Up);
    handle_key_events(Key::from(up), up, &mut app).await;
    assert_eq!(app.help_scroll, 0);
    handle_key_events(Key::from(up), up, &mut app).await;
    assert_eq!(app.help_scroll, 0);
  }

  #[tokio::test]
  async fn test_handle_describe_or_yaml_action() {
    let mut app = App::default();

    app.route_home();
    assert_eq!(app.data.pods.state.selected(), None);

    let item = KubePod::default();

    assert!(
      handle_describe_decode_or_yaml_action(
        Key::Char('d'),
        &mut app,
        &item,
        IoCmdEvent::GetDescribe {
          kind: "pod".to_owned(),
          value: "name".to_owned(),
          ns: Some("namespace".to_owned()),
        }
      )
      .await
    );

    assert_eq!(app.get_current_route().active_block, ActiveBlock::Describe);
    assert_eq!(app.data.describe_out.get_txt(), "");

    assert!(
      handle_describe_decode_or_yaml_action(
        Key::Char('y'),
        &mut app,
        &item,
        IoCmdEvent::GetDescribe {
          kind: "pod".to_owned(),
          value: "name".to_owned(),
          ns: Some("namespace".to_owned()),
        }
      )
      .await
    );

    assert_eq!(app.get_current_route().active_block, ActiveBlock::Yaml);
    assert_eq!(
      app.data.describe_out.get_txt(),
      "apiVersion: v1\nkind: Pod\nmetadata: {}\n"
    );

    assert!(
      !handle_describe_decode_or_yaml_action(
        Key::Char('s'),
        &mut app,
        &item,
        IoCmdEvent::GetDescribe {
          kind: "pod".to_owned(),
          value: "name".to_owned(),
          ns: Some("namespace".to_owned()),
        }
      )
      .await
    );
  }

  #[tokio::test]
  async fn test_decode_secret() {
    const DATA1: &str = "Hello, World!";
    const DATA2: &str =
      "Neque porro quisquam est qui dolorem ipsum quia dolor sit amet, consectetur, adipisci velit";

    let mut app = App::default();
    app.route_home();

    let mut secret = KubeSecret::default();
    // ByteString base64 encodes the data
    secret
      .data
      .insert(String::from("key1"), ByteString(DATA1.as_bytes().into()));
    secret
      .data
      .insert(String::from("key2"), ByteString(DATA2.as_bytes().into()));

    // ensure that 'x' decodes the secret data
    assert!(
      handle_describe_decode_or_yaml_action(
        Key::Char('x'),
        &mut app,
        &secret,
        IoCmdEvent::GetDescribe {
          kind: "secret".to_owned(),
          value: "name".to_owned(),
          ns: Some("namespace".to_owned()),
        }
      )
      .await
    );

    assert!(app
      .data
      .describe_out
      .get_txt()
      .contains(format!("key1: {}", DATA1).as_str()));
    assert!(app
      .data
      .describe_out
      .get_txt()
      .contains(format!("key2: {}", DATA2).as_str()));
  }

  #[tokio::test]
  async fn test_handle_scroll() {
    let mut app = App::default();

    app.route_home();
    assert_eq!(app.data.pods.state.selected(), None);

    app
      .data
      .pods
      .set_items(vec![KubePod::default(), KubePod::default()]);

    // mouse scroll
    assert_eq!(app.data.pods.state.selected(), Some(0));
    handle_block_scroll(&mut app, ScrollEvent::down(), true).await;
    assert_eq!(app.data.pods.state.selected(), Some(1));
    handle_block_scroll(&mut app, ScrollEvent::up(), true).await;
    assert_eq!(app.data.pods.state.selected(), Some(0));
    handle_block_scroll(&mut app, ScrollEvent::End, false).await;
    assert_eq!(app.data.pods.state.selected(), Some(1));

    // check logs keyboard scroll
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Logs);
    assert_eq!(app.data.logs.state.selected(), None);

    app.data.logs.add_record("record".to_string());
    app.data.logs.add_record("record 2".to_string());
    app.data.logs.add_record("record 3".to_string());

    handle_block_scroll(&mut app, ScrollEvent::down(), false).await;
    assert_eq!(app.data.logs.state.selected(), Some(0));
  }

  #[tokio::test]
  async fn test_context_switch() {
    let mut app = App::default();
    let ctx = KubeContext {
      name: "test".into(),
      ..KubeContext::default()
    };
    app.data.contexts.set_items(vec![ctx]);

    assert_eq!(app.data.selected.context, None);
    app.route_contexts();
    handle_route_events(Key::Enter, &mut app).await;

    assert_eq!(app.data.selected.context, Some("test".into()));
    assert!(app.refresh);
  }

  #[tokio::test]
  async fn test_context_switch_preselects_namespace() {
    let mut app = App::default();
    let ctx = KubeContext {
      name: "prod".into(),
      namespace: Some("prod-ns".into()),
      ..KubeContext::default()
    };
    app.data.contexts.set_items(vec![ctx]);

    assert_eq!(app.data.selected.ns, None);
    app.route_contexts();
    handle_route_events(Key::Enter, &mut app).await;

    assert_eq!(app.data.selected.context, Some("prod".into()));
    assert_eq!(app.data.selected.ns, Some("prod-ns".into()));
    assert!(app.refresh);
  }

  #[tokio::test]
  async fn test_context_switch_no_namespace_clears_ns() {
    let mut app = App::default();
    app.data.selected.ns = Some("old-ns".into());
    let ctx = KubeContext {
      name: "dev".into(),
      namespace: None,
      ..KubeContext::default()
    };
    app.data.contexts.set_items(vec![ctx]);

    app.route_contexts();
    handle_route_events(Key::Enter, &mut app).await;

    assert_eq!(app.data.selected.context, Some("dev".into()));
    assert_eq!(app.data.selected.ns, None);
    assert!(app.refresh);
  }

  #[test]
  fn test_filter_menu_items_empty_filter_returns_all() {
    let items = vec![
      ("CronJobs".into(), ActiveBlock::CronJobs),
      ("Secrets".into(), ActiveBlock::Secrets),
      ("Roles".into(), ActiveBlock::Roles),
    ];
    let filtered = filter_menu_items(&items, "");
    assert_eq!(filtered.len(), 3);
  }

  #[test]
  fn test_filter_menu_items_substring_match() {
    let items = vec![
      ("CronJobs".into(), ActiveBlock::CronJobs),
      ("Secrets".into(), ActiveBlock::Secrets),
      ("Roles".into(), ActiveBlock::Roles),
    ];
    let filtered = filter_menu_items(&items, "cron");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].1 .0, "CronJobs");
  }

  #[test]
  fn test_filter_menu_items_case_insensitive() {
    let items = vec![
      ("CronJobs".into(), ActiveBlock::CronJobs),
      ("Secrets".into(), ActiveBlock::Secrets),
    ];
    let filtered = filter_menu_items(&items, "CRON");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].1 .0, "CronJobs");
  }

  #[test]
  fn test_filter_menu_items_glob_match() {
    let items = vec![
      ("ClusterRoles".into(), ActiveBlock::ClusterRoles),
      (
        "ClusterRoleBinding".into(),
        ActiveBlock::ClusterRoleBindings,
      ),
      ("CronJobs".into(), ActiveBlock::CronJobs),
    ];
    let filtered = filter_menu_items(&items, "cluster*");
    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].1 .0, "ClusterRoles");
    assert_eq!(filtered[1].1 .0, "ClusterRoleBinding");
  }

  #[test]
  fn test_filter_menu_items_no_match() {
    let items = vec![
      ("CronJobs".into(), ActiveBlock::CronJobs),
      ("Secrets".into(), ActiveBlock::Secrets),
    ];
    let filtered = filter_menu_items(&items, "zzz");
    assert_eq!(filtered.len(), 0);
  }

  #[test]
  fn test_filter_menu_items_preserves_original_index() {
    let items = vec![
      ("CronJobs".into(), ActiveBlock::CronJobs),
      ("Secrets".into(), ActiveBlock::Secrets),
      ("Roles".into(), ActiveBlock::Roles),
    ];
    let filtered = filter_menu_items(&items, "role");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].0, 2); // original index
  }

  #[tokio::test]
  async fn test_menu_filter_captures_character_keys() {
    let mut app = App::default();
    // Navigate to More menu
    app.push_navigation_stack(RouteId::Home, ActiveBlock::More);

    // Activate filter mode with '/'
    let key_evt = KeyEvent::from(KeyCode::Char('/'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(app.menu_filter_active);

    let key_evt = KeyEvent::from(KeyCode::Char('c'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.menu_filter, "c");

    let key_evt = KeyEvent::from(KeyCode::Char('r'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.menu_filter, "cr");
  }

  #[tokio::test]
  async fn test_menu_filter_requires_slash_to_activate() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::More);

    // Typing without '/' should not filter
    let key_evt = KeyEvent::from(KeyCode::Char('c'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.menu_filter, "");
    assert!(!app.menu_filter_active);
  }

  #[tokio::test]
  async fn test_menu_filter_backspace_removes_char() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::More);
    app.menu_filter_active = true;

    let key_evt = KeyEvent::from(KeyCode::Char('a'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    let key_evt = KeyEvent::from(KeyCode::Char('b'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.menu_filter, "ab");

    let key_evt = KeyEvent::from(KeyCode::Backspace);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.menu_filter, "a");
  }

  #[tokio::test]
  async fn test_menu_filter_backspace_on_empty_does_not_panic() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::More);
    app.menu_filter_active = true;

    let key_evt = KeyEvent::from(KeyCode::Backspace);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.menu_filter, "");
  }

  #[tokio::test]
  async fn test_menu_filter_escape_clears_filter_first() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::More);
    app.menu_filter_active = true;

    // Type a filter
    let key_evt = KeyEvent::from(KeyCode::Char('x'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.menu_filter, "x");

    // First Escape clears filter but stays in menu
    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.menu_filter, "");
    assert!(app.menu_filter_active); // still active, just cleared text
    assert_eq!(app.get_current_route().active_block, ActiveBlock::More);

    // Second Escape deactivates filter mode
    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(!app.menu_filter_active);
    assert_eq!(app.get_current_route().active_block, ActiveBlock::More);
  }

  #[tokio::test]
  async fn test_menu_filter_escape_on_empty_closes_menu() {
    let mut app = App::default();
    // Push a base route then the menu
    app.push_navigation_stack(RouteId::Home, ActiveBlock::More);

    // Escape with empty filter
    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.menu_filter, "");
  }

  #[tokio::test]
  async fn test_menu_filter_enter_selects_filtered_item() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::More);
    app.menu_filter_active = true;

    // Type "cron" to filter to CronJobs
    for c in "cron".chars() {
      let key_evt = KeyEvent::from(KeyCode::Char(c));
      handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    }
    assert_eq!(app.menu_filter, "cron");

    // Selection should be at 0 (first filtered item)
    assert_eq!(app.more_resources_menu.state.selected(), Some(0));

    // Press Enter to select
    let key_evt = KeyEvent::from(KeyCode::Enter);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    // Should navigate to CronJobs, clear filter, and deactivate filter mode
    assert_eq!(app.menu_filter, "");
    assert!(!app.menu_filter_active);
    assert_eq!(app.get_current_route().active_block, ActiveBlock::CronJobs);
  }

  #[test]
  fn test_handle_menu_scroll_within_filtered_bounds() {
    let mut menu = StatefulList::with_items(vec![
      ("A".into(), ActiveBlock::CronJobs),
      ("B".into(), ActiveBlock::Secrets),
      ("C".into(), ActiveBlock::Roles),
    ]);

    // Scroll down within filtered_len=2
    menu.state.select(Some(0));
    handle_menu_scroll(&mut menu, ScrollEvent::down(), 2);
    assert_eq!(menu.state.selected(), Some(1));

    // Scroll down wraps at filtered_len
    handle_menu_scroll(&mut menu, ScrollEvent::down(), 2);
    assert_eq!(menu.state.selected(), Some(0));

    // Scroll up from 0 wraps to end of filtered
    handle_menu_scroll(&mut menu, ScrollEvent::up(), 2);
    assert_eq!(menu.state.selected(), Some(1));
  }

  #[test]
  fn test_handle_menu_scroll_empty_filtered() {
    let mut menu = StatefulList::with_items(vec![("A".into(), ActiveBlock::CronJobs)]);
    menu.state.select(Some(0));
    // Should not panic with filtered_len=0
    handle_menu_scroll(&mut menu, ScrollEvent::Relative(0), 0);
    assert_eq!(menu.state.selected(), Some(0));
  }

  #[tokio::test]
  async fn test_dispatch_resource_pods_sets_selector_state() {
    let mut app = App::default();
    app.route_home();

    app
      .dispatch_resource_pods(
        "default".into(),
        "app=nginx".into(),
        "deployment".into(),
        RouteId::Home,
      )
      .await;

    assert_eq!(app.data.selected.pod_selector, Some("app=nginx".into()));
    assert_eq!(app.data.selected.pod_selector_ns, Some("default".into()));
    assert_eq!(
      app.data.selected.pod_selector_resource,
      Some("deployment".into())
    );
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Pods);
  }

  #[tokio::test]
  async fn test_dispatch_aggregate_logs_sets_state() {
    let mut app = App::default();
    app.route_home();

    app
      .dispatch_aggregate_logs(
        "my-deploy".into(),
        "default".into(),
        "app=nginx".into(),
        "deployment".into(),
        RouteId::Home,
      )
      .await;

    assert_eq!(app.data.logs.id, "agg:my-deploy");
    assert_eq!(
      app.data.selected.pod_selector_resource,
      Some("deployment".into())
    );
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Logs);
  }

  #[tokio::test]
  async fn test_escape_from_more_selected_resource_returns_to_more_not_dynamic() {
    let mut app = App::default();
    // Both pickers visited (Dynamic, then More), then a resource picked from More.
    app.push_navigation_stack(RouteId::Home, ActiveBlock::DynamicView);
    app.push_navigation_stack(RouteId::Home, ActiveBlock::More);
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Secrets);

    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    // Returns to the picker actually used (More), not the stacked Dynamic menu.
    assert_eq!(app.get_current_route().active_block, ActiveBlock::More);
  }

  #[tokio::test]
  async fn test_escape_from_filtered_pods_clears_selector_state() {
    let mut app = App::default();
    app.route_home();

    // Simulate drill-down state
    app.data.selected.pod_selector = Some("app=nginx".into());
    app.data.selected.pod_selector_ns = Some("default".into());
    app.data.selected.pod_selector_resource = Some("deployment".into());
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Pods);

    // Press Esc
    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    assert_eq!(app.data.selected.pod_selector, None);
    assert_eq!(app.data.selected.pod_selector_ns, None);
    assert_eq!(app.data.selected.pod_selector_resource, None);
  }

  #[tokio::test]
  async fn test_escape_from_aggregate_logs_clears_resource_context() {
    let mut app = App::default();
    app.route_home();

    // Simulate aggregate logs state (no pod_selector set)
    app.data.selected.pod_selector_resource = Some("deployment".into());
    app.data.logs = crate::app::models::LogsState::new("agg:my-deploy".into());
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Logs);

    // Press Esc
    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    // Resource context should be cleared for aggregate logs
    assert_eq!(app.data.selected.pod_selector_resource, None);
  }

  #[tokio::test]
  async fn test_escape_from_drilldown_logs_preserves_resource_context() {
    let mut app = App::default();
    app.route_home();

    // Simulate drill-down: Deployment → Pods → Container → Logs
    app.data.selected.pod_selector = Some("app=nginx".into());
    app.data.selected.pod_selector_ns = Some("default".into());
    app.data.selected.pod_selector_resource = Some("deployment".into());
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Pods);
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Containers);
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Logs);

    // Press Esc from Logs — should go back to Containers, resource context preserved
    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    assert_eq!(
      app.data.selected.pod_selector_resource,
      Some("deployment".into())
    );
    assert_eq!(app.data.selected.pod_selector, Some("app=nginx".into()));
  }

  #[tokio::test]
  async fn test_jump_to_pods_clears_selector_state() {
    let mut app = App::default();
    app.route_home();

    // Simulate leftover drill-down state
    app.data.selected.pod_selector = Some("app=nginx".into());
    app.data.selected.pod_selector_ns = Some("default".into());
    app.data.selected.pod_selector_resource = Some("deployment".into());

    // Press '1' to jump to pods tab
    let key_evt = KeyEvent::from(KeyCode::Char('1'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    assert_eq!(app.data.selected.pod_selector, None);
    assert_eq!(app.data.selected.pod_selector_ns, None);
    assert_eq!(app.data.selected.pod_selector_resource, None);
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Pods);
  }

  #[tokio::test]
  async fn test_enter_on_leaf_resource_runs_describe() {
    let mut app = App::default();
    app.route_home();

    // Navigate to Secrets (a leaf resource with no child views)
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Secrets);

    let mut secret = KubeSecret::default();
    secret.name = "my-secret".into();
    secret.namespace = "default".into();
    app.data.secrets.set_items(vec![secret]);

    // Press Enter
    let key_evt = KeyEvent::from(KeyCode::Enter);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    // Should navigate to Describe view
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Describe);
  }

  #[tokio::test]
  async fn test_dispatch_node_pods_sets_state() {
    let mut app = App::default();
    app.route_home();

    app
      .dispatch_node_pods("my-node-01".into(), RouteId::Home)
      .await;

    assert_eq!(app.data.selected.pod_selector, Some("my-node-01".into()));
    assert_eq!(app.data.selected.pod_selector_ns, None);
    assert_eq!(app.data.selected.pod_selector_resource, Some("node".into()));
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Pods);
  }

  #[tokio::test]
  async fn test_dynamic_view_selection_uses_cached_items_immediately() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::DynamicView);
    app.dynamic_resources_menu =
      StatefulList::with_items(vec![("Widget".into(), ActiveBlock::DynamicResource)]);
    app.dynamic_resources_menu.state.select(Some(0));

    let kind = KubeDynamicKind::new(
      ApiResource {
        group: "example.com".into(),
        version: "v1".into(),
        api_version: "example.com/v1".into(),
        kind: "Widget".into(),
        plural: "widgets".into(),
      },
      Scope::Namespaced,
    );
    app.data.dynamic_kinds = vec![kind.clone()];
    app.data.selected.ns = Some("team-a".into());

    let cached_items = vec![KubeDynamicResource::from(DynamicObject {
      types: None,
      metadata: ObjectMeta {
        name: Some("widget-1".into()),
        namespace: Some("team-a".into()),
        ..Default::default()
      },
      data: Default::default(),
    })];
    app.data.dynamic_resource_cache.insert(
      dynamic_cache_key(&kind, Some("team-a")),
      cached_items.clone(),
    );

    let key_evt = KeyEvent::from(KeyCode::Enter);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    assert_eq!(
      app.get_current_route().active_block,
      ActiveBlock::DynamicResource
    );
    assert_eq!(
      app
        .data
        .selected
        .dynamic_kind
        .as_ref()
        .map(|it| it.kind.as_str()),
      Some("Widget")
    );
    assert_eq!(app.data.dynamic_resources.items, cached_items);
  }

  #[tokio::test]
  async fn test_enter_on_dynamic_resource_runs_describe() {
    let (sync_io_tx, _sync_io_rx) = mpsc::channel(10);
    let (sync_io_stream_tx, _sync_io_stream_rx) = mpsc::channel(10);
    let (sync_io_cmd_tx, mut sync_io_cmd_rx) = mpsc::channel::<IoCmdEvent>(10);
    let mut app = App::new(
      sync_io_tx,
      sync_io_stream_tx,
      sync_io_cmd_tx,
      false,
      1,
      App::default().log_tail_lines,
      crate::config::KdashConfig::default(),
    );
    app.push_navigation_stack(RouteId::Home, ActiveBlock::DynamicResource);

    let kind = KubeDynamicKind::new(
      ApiResource {
        group: "example.com".into(),
        version: "v1".into(),
        api_version: "example.com/v1".into(),
        kind: "Widget".into(),
        plural: "widgets".into(),
      },
      Scope::Namespaced,
    );
    app.data.selected.dynamic_kind = Some(kind);
    app.data.dynamic_resources =
      StatefulTable::with_items(vec![KubeDynamicResource::from(DynamicObject {
        types: None,
        metadata: ObjectMeta {
          name: Some("widget-1".into()),
          namespace: Some("team-a".into()),
          ..Default::default()
        },
        data: Default::default(),
      })]);

    let key_evt = KeyEvent::from(KeyCode::Enter);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;

    assert_eq!(app.get_current_route().active_block, ActiveBlock::Describe);
    assert_eq!(
      sync_io_cmd_rx.recv().await.unwrap(),
      IoCmdEvent::GetDescribe {
        kind: "Widget".into(),
        value: "widget-1".into(),
        ns: Some("team-a".into()),
      }
    );
  }
}
