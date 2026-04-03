use crossterm::event::{Event, KeyEvent, MouseEvent, MouseEventKind};
use kubectl_view_allocations::GroupBy;
use serde::Serialize;
use tui_input::backend::crossterm::EventHandler;

use crate::{
  app::{
    key_binding::DEFAULT_KEYBINDING,
    models::{KubeResource, Scrollable, ScrollableTxt, StatefulList, StatefulTable},
    secrets::KubeSecret,
    ActiveBlock, App, InputMode, Route, RouteId,
  },
  cmd::IoCmdEvent,
  event::Key,
};

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
            let _ok = handle_describe_decode_or_yaml_action(
              $key, $app, &res,
              IoCmdEvent::GetDescribe {
                kind: $kind.to_owned(),
                value: res.name.to_owned(),
                ns: Some(res.namespace.to_owned()),
              },
            ).await;
          }
        }
      )*
      $(
        $cblock => {
          if let Some(res) = handle_block_action($key, &$app.data.$cfield) {
            let _ok = handle_describe_decode_or_yaml_action(
              $key, $app, &res,
              IoCmdEvent::GetDescribe {
                kind: $ckind.to_owned(),
                value: res.name.to_owned(),
                ns: None,
              },
            ).await;
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
  ($match_expr:expr, $app:expr, $up:expr, $page:expr,
    [ $(($block:path, $field:ident)),* $(,)? ],
    extra: { $($extra_arms:tt)* }
  ) => {
    match $match_expr {
      $(
        $block => $app.data.$field.handle_scroll($up, $page),
      )*
      $($extra_arms)*
    }
  };
}

pub async fn handle_key_events(key: Key, key_event: KeyEvent, app: &mut App) {
  // if input is enabled capture keystrokes
  if app.app_input.input_mode == InputMode::Editing {
    if key == DEFAULT_KEYBINDING.esc.key {
      app.app_input.input_mode = InputMode::Normal;
    } else {
      app.app_input.input.handle_event(&Event::Key(key_event));
      app.data.selected.filter = Some(app.app_input.input.value().into());
    }
  } else if app.is_menu_active() && handle_menu_filter_key(key, app) {
    // Menu filter captured the key — done
  } else {
    // First handle any global event and then move to route event
    match key {
      _ if key == DEFAULT_KEYBINDING.esc.key => {
        handle_escape(app);
      }
      _ if key == DEFAULT_KEYBINDING.quit.key || key == DEFAULT_KEYBINDING.quit.alt.unwrap() => {
        app.should_quit = true;
      }
      _ if key == DEFAULT_KEYBINDING.up.key || key == DEFAULT_KEYBINDING.up.alt.unwrap() => {
        handle_block_scroll(app, true, false, false).await;
      }
      _ if key == DEFAULT_KEYBINDING.down.key || key == DEFAULT_KEYBINDING.down.alt.unwrap() => {
        handle_block_scroll(app, false, false, false).await;
      }
      _ if key == DEFAULT_KEYBINDING.pg_up.key => {
        handle_block_scroll(app, true, false, true).await;
      }
      _ if key == DEFAULT_KEYBINDING.pg_down.key => {
        handle_block_scroll(app, false, false, true).await;
      }
      _ if key == DEFAULT_KEYBINDING.toggle_theme.key => {
        app.light_theme = !app.light_theme;
      }
      _ if key == DEFAULT_KEYBINDING.refresh.key => {
        app.refresh();
      }
      _ if key == DEFAULT_KEYBINDING.help.key => {
        if app.get_current_route().active_block != ActiveBlock::Help {
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
      _ if key == DEFAULT_KEYBINDING.cycle_main_views.key => {
        app.cycle_main_routes();
      }
      _ => handle_route_events(key, app).await,
    }
  }
}

pub async fn handle_mouse_events(mouse: MouseEvent, app: &mut App) {
  match mouse.kind {
    // mouse scrolling is inverted
    MouseEventKind::ScrollDown => handle_block_scroll(app, true, true, false).await,
    MouseEventKind::ScrollUp => handle_block_scroll(app, false, true, false).await,
    _ => {}
  }
}

fn handle_escape(app: &mut App) {
  // dismiss error
  if !app.api_error.is_empty() {
    app.api_error = String::default();
  }

  // If menu is active with a filter, clear the filter first
  if app.is_menu_active() && !app.menu_filter.is_empty() {
    app.menu_filter.clear();
    return;
  }

  // Clear menu filter on any menu exit
  if app.is_menu_active() {
    app.menu_filter.clear();
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
      ActiveBlock::Logs => {
        app.cancel_log_stream();
        app.pop_navigation_stack();
      }
      _ => {
        if let ActiveBlock::More = app.get_prev_route().active_block {
          app.pop_navigation_stack();
        }
        if let ActiveBlock::DynamicView = app.get_prev_route().active_block {
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

// Handle event for the current active block
async fn handle_route_events(key: Key, app: &mut App) {
  // route specific events
  match app.get_current_route().id {
    // handle resource tabs on overview
    RouteId::Home => {
      match key {
        _ if key == DEFAULT_KEYBINDING.right.key
          || key == DEFAULT_KEYBINDING.right.alt.unwrap() =>
        {
          app.context_tabs.next();
          app.push_navigation_route(app.context_tabs.get_active_route().clone());
        }
        _ if key == DEFAULT_KEYBINDING.left.key || key == DEFAULT_KEYBINDING.left.alt.unwrap() => {
          app.context_tabs.previous();
          app.push_navigation_route(app.context_tabs.get_active_route().clone());
        }
        _ if key == DEFAULT_KEYBINDING.toggle_info.key => {
          app.show_info_bar = !app.show_info_bar;
        }
        _ if key == DEFAULT_KEYBINDING.toggle_global_filter.key => {
          if app.show_filter_bar {
            app.app_input.input_mode = InputMode::Normal;
            app.app_input.input.reset();
            app.data.selected.filter = None;
          } else {
            app.app_input.input_mode = InputMode::Editing;
          }
          app.show_filter_bar = !app.show_filter_bar;
        }
        _ if key == DEFAULT_KEYBINDING.toggle_global_filter_edit.key => {
          if app.show_filter_bar {
            app.app_input.input_mode = InputMode::Editing;
          }
        }
        _ if key == DEFAULT_KEYBINDING.select_all_namespace.key => app.data.selected.ns = None,
        _ if key == DEFAULT_KEYBINDING.jump_to_namespace.key => {
          if app.get_current_route().active_block != ActiveBlock::Namespaces {
            app.push_navigation_stack(RouteId::Home, ActiveBlock::Namespaces);
          }
        }
        // as these are tabs with index the order here matters, atleast for readability
        _ if key == DEFAULT_KEYBINDING.jump_to_pods.key => {
          let route = app.context_tabs.set_index(0).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_services.key => {
          let route = app.context_tabs.set_index(1).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_nodes.key => {
          let route = app.context_tabs.set_index(2).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_configmaps.key => {
          let route = app.context_tabs.set_index(3).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_statefulsets.key => {
          let route = app.context_tabs.set_index(4).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_replicasets.key => {
          let route = app.context_tabs.set_index(5).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_deployments.key => {
          let route = app.context_tabs.set_index(6).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_jobs.key => {
          let route = app.context_tabs.set_index(7).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_daemonsets.key => {
          let route = app.context_tabs.set_index(8).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_more_resources.key => {
          let route = app.context_tabs.set_index(9).route.clone();
          app.push_navigation_route(route);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_dynamic_resources.key => {
          let route = app.context_tabs.set_index(10).route.clone();
          app.push_navigation_route(route);
        }
        _ => {}
      };

      // handle block specific stuff
      handle_resource_action!(app.get_current_route().active_block, key, app,
        namespaced: [
          (ActiveBlock::Services, services, "service"),
          (ActiveBlock::Deployments, deployments, "deployment"),
          (ActiveBlock::ConfigMaps, config_maps, "configmap"),
          (ActiveBlock::StatefulSets, stateful_sets, "statefulset"),
          (ActiveBlock::ReplicaSets, replica_sets, "replicaset"),
          (ActiveBlock::Jobs, jobs, "job"),
          (ActiveBlock::DaemonSets, daemon_sets, "daemonset"),
          (ActiveBlock::CronJobs, cronjobs, "cronjob"),
          (ActiveBlock::Secrets, secrets, "secret"),
          (ActiveBlock::ReplicationControllers, replication_controllers, "replicationcontroller"),
          (ActiveBlock::Roles, roles, "roles"),
          (ActiveBlock::RoleBindings, role_bindings, "rolebindings"),
          (ActiveBlock::Ingresses, ingress, "ingress"),
          (ActiveBlock::PersistentVolumeClaims, persistent_volume_claims, "persistentvolumeclaims"),
          (ActiveBlock::ServiceAccounts, service_accounts, "serviceaccounts"),
          (ActiveBlock::NetworkPolicies, network_policies, "networkpolicy"),
        ],
        cluster: [
          (ActiveBlock::Nodes, nodes, "node"),
          (ActiveBlock::StorageClasses, storage_classes, "storageclass"),
          (ActiveBlock::ClusterRoles, cluster_roles, "clusterroles"),
          (ActiveBlock::ClusterRoleBindings, cluster_role_bindings, "clusterrolebinding"),
          (ActiveBlock::PersistentVolumes, persistent_volumes, "persistentvolumes"),
        ],
        extra: {
          ActiveBlock::Namespaces => {
            if let Some(ns) = handle_block_action(key, &app.data.namespaces) {
              app.data.selected.ns = Some(ns.name);
              app.cache_all_resource_data().await;
              app.pop_navigation_stack();
            }
          }
          ActiveBlock::Pods => {
            if let Some(pod) = handle_block_action(key, &app.data.pods) {
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
            if let Some(c) = handle_block_action(key, &app.data.containers) {
              app.data.selected.container = Some(c.name.clone());
              app.dispatch_container_logs(c.name).await;
            }
          }
      ActiveBlock::Logs => {
        if key == DEFAULT_KEYBINDING.log_auto_scroll.key {
          app.log_auto_scroll = !app.log_auto_scroll;
        } else if key == DEFAULT_KEYBINDING.copy_to_clipboard.key {
          copy_to_clipboard(app.data.logs.get_plain_text(), app);
        }
      }
      ActiveBlock::Describe | ActiveBlock::Yaml => {
        if key == DEFAULT_KEYBINDING.copy_to_clipboard.key {
          copy_to_clipboard(app.data.describe_out.get_txt(), app);
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
                app.push_navigation_route(Route {
                  id: RouteId::Home,
                  active_block,
                });
                let selected = app.data.dynamic_kinds.iter().find(|it| it.kind == title);
                app.data.selected.dynamic_kind = selected.cloned();
                app.data.dynamic_resources.set_items(vec![]);
              }
            }
          }
          ActiveBlock::DynamicResource => {
            if let Some(dynamic_res) = app.data.selected.dynamic_kind.as_ref() {
              if let Some(res) = handle_block_action(key, &app.data.dynamic_resources) {
                let _ok = handle_describe_decode_or_yaml_action(
                  key,
                  app,
                  &res,
                  IoCmdEvent::GetDescribe {
                    kind: dynamic_res.kind.to_owned(),
                    value: res.name.to_owned(),
                    ns: res.namespace.to_owned(),
                  },
                )
                .await;
              }
            }
          }
          ActiveBlock::Contexts | ActiveBlock::Utilization | ActiveBlock::Help => { /* Do nothing */ }
        }
      )
    }
    RouteId::Contexts => {
      if let Some(ctx) = handle_block_action(key, &app.data.contexts) {
        app.data.selected.context = Some(ctx.name);
        // Pre-select the namespace from the context if one is configured (#90)
        app.data.selected.ns = ctx.namespace;
        app.refresh();
      }
    }
    RouteId::Utilization => {
      if key == DEFAULT_KEYBINDING.cycle_group_by.key {
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
    RouteId::HelpMenu => { /* Do nothing */ }
  }
  // reset tick_count so that network requests are made faster
  if key == DEFAULT_KEYBINDING.submit.key {
    app.tick_count = 0;
  }
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

async fn handle_block_scroll(app: &mut App, up: bool, is_mouse: bool, page: bool) {
  handle_resource_scroll!(app.get_current_route().active_block, app, up, page,
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
      (ActiveBlock::NetworkPolicies, network_policies),
      (ActiveBlock::DynamicResource, dynamic_resources),
    ],
    extra: {
      ActiveBlock::Contexts => app.data.contexts.handle_scroll(up, page),
      ActiveBlock::Utilization => app.data.metrics.handle_scroll(up, page),
      ActiveBlock::Help => app.help_docs.handle_scroll(up, page),
      ActiveBlock::More => {
        let filtered_len = filter_menu_items(&app.more_resources_menu.items, &app.menu_filter).len();
        handle_menu_scroll(&mut app.more_resources_menu, up, page, filtered_len);
      }
      ActiveBlock::DynamicView => {
        let filtered_len = filter_menu_items(&app.dynamic_resources_menu.items, &app.menu_filter).len();
        handle_menu_scroll(&mut app.dynamic_resources_menu, up, page, filtered_len);
      }
      ActiveBlock::Logs => {
        app.log_auto_scroll = false;
        app.data.logs.handle_scroll(inverse_dir(up, is_mouse), page);
      }
      ActiveBlock::Describe | ActiveBlock::Yaml => app
        .data
        .describe_out
        .handle_scroll(inverse_dir(up, is_mouse), page),
    }
  )
}

/// Scroll within a menu, respecting filtered item count
fn handle_menu_scroll(
  menu: &mut StatefulList<(String, ActiveBlock)>,
  up: bool,
  page: bool,
  filtered_len: usize,
) {
  if filtered_len == 0 {
    return;
  }
  let increment = if page { 5 } else { 1 };
  let i = match menu.state.selected() {
    Some(i) => {
      if up {
        if i == 0 {
          filtered_len.saturating_sub(increment)
        } else {
          i.saturating_sub(increment)
        }
      } else if i >= filtered_len.saturating_sub(increment) {
        0
      } else {
        i + increment
      }
    }
    None => 0,
  };
  menu.state.select(Some(i));
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

/// inverse direction for natural scrolling on mouse and keyboard
fn inverse_dir(up: bool, is_mouse: bool) -> bool {
  if is_mouse {
    !up
  } else {
    up
  }
}

#[cfg(test)]
mod tests {
  use crossterm::event::KeyCode;
  use k8s_openapi::ByteString;

  use super::*;
  use crate::app::{contexts::KubeContext, pods::KubePod};

  #[test]
  fn test_inverse_dir() {
    assert!(inverse_dir(true, false));
    assert!(!inverse_dir(true, true));
  }

  #[tokio::test]

  async fn test_handle_key_events_for_filter() {
    let mut app = App::default();

    app.route_home();
    assert_eq!(app.app_input.input_mode, InputMode::Normal);

    let key_evt = KeyEvent::from(KeyCode::Char('f'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert!(app.show_filter_bar);
    assert_eq!(app.app_input.input_mode, InputMode::Editing);

    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.app_input.input_mode, InputMode::Normal);

    let key_evt = KeyEvent::from(KeyCode::Char('e'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.app_input.input_mode, InputMode::Editing);

    let key_evt = KeyEvent::from(KeyCode::Char('f'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.app_input.input_mode, InputMode::Editing);
    assert!(app.show_filter_bar);

    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.app_input.input_mode, InputMode::Normal);
    let key_evt = KeyEvent::from(KeyCode::Char('f'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.app_input.input_mode, InputMode::Normal);
    assert!(!app.show_filter_bar);
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
    handle_block_scroll(&mut app, false, true, false).await;
    assert_eq!(app.data.pods.state.selected(), Some(1));
    handle_block_scroll(&mut app, true, true, false).await;
    assert_eq!(app.data.pods.state.selected(), Some(0));

    // check logs keyboard scroll
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Logs);
    assert_eq!(app.data.logs.state.selected(), None);

    app.data.logs.add_record("record".to_string());
    app.data.logs.add_record("record 2".to_string());
    app.data.logs.add_record("record 3".to_string());

    handle_block_scroll(&mut app, true, false, false).await;
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

    let key_evt = KeyEvent::from(KeyCode::Char('c'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.menu_filter, "c");

    let key_evt = KeyEvent::from(KeyCode::Char('r'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.menu_filter, "cr");
  }

  #[tokio::test]
  async fn test_menu_filter_backspace_removes_char() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::More);

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

    let key_evt = KeyEvent::from(KeyCode::Backspace);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.menu_filter, "");
  }

  #[tokio::test]
  async fn test_menu_filter_escape_clears_filter_first() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::More);

    // Type a filter
    let key_evt = KeyEvent::from(KeyCode::Char('x'));
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.menu_filter, "x");

    // First Escape clears filter but stays in menu
    let key_evt = KeyEvent::from(KeyCode::Esc);
    handle_key_events(Key::from(key_evt), key_evt, &mut app).await;
    assert_eq!(app.menu_filter, "");
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

    // Should navigate to CronJobs and clear filter
    assert_eq!(app.menu_filter, "");
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
    handle_menu_scroll(&mut menu, false, false, 2);
    assert_eq!(menu.state.selected(), Some(1));

    // Scroll down wraps at filtered_len
    handle_menu_scroll(&mut menu, false, false, 2);
    assert_eq!(menu.state.selected(), Some(0));

    // Scroll up from 0 wraps to end of filtered
    handle_menu_scroll(&mut menu, true, false, 2);
    assert_eq!(menu.state.selected(), Some(1));
  }

  #[test]
  fn test_handle_menu_scroll_empty_filtered() {
    let mut menu = StatefulList::with_items(vec![("A".into(), ActiveBlock::CronJobs)]);
    menu.state.select(Some(0));
    // Should not panic with filtered_len=0
    handle_menu_scroll(&mut menu, false, false, 0);
    assert_eq!(menu.state.selected(), Some(0));
  }
}
