use crossterm::event::{MouseEvent, MouseEventKind};
use kubectl_view_allocations::GroupBy;
use serde::Serialize;

use crate::{
  app::{
    key_binding::DEFAULT_KEYBINDING,
    models::{KubeResource, Scrollable, ScrollableTxt, StatefulTable},
    ActiveBlock, App, Route, RouteId,
  },
  cmd::IoCmdEvent,
  event::Key,
};

pub async fn handle_key_events(key: Key, app: &mut App) {
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
  match app.get_current_route().id {
    RouteId::HelpMenu => {
      app.pop_navigation_stack();
    }
    _ => match app.get_current_route().active_block {
      ActiveBlock::Namespaces
      | ActiveBlock::Logs
      | ActiveBlock::Containers
      | ActiveBlock::Yaml
      | ActiveBlock::Describe => {
        app.pop_navigation_stack();
      }
      _ => {
        if let ActiveBlock::More = app.get_prev_route().active_block {
          app.pop_navigation_stack();
        }
      }
    },
  }
}

async fn handle_describe_or_yaml_action<T, S>(
  key: Key,
  app: &mut App,
  res: &T,
  action: IoCmdEvent,
) -> bool
where
  T: KubeResource<S>,
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
        _ => {}
      };

      // handle block specific stuff
      match app.get_current_route().active_block {
        ActiveBlock::Namespaces => {
          if let Some(ns) = handle_block_action(key, &mut app.data.namespaces) {
            app.data.selected.ns = Some(ns.name);
            app.pop_navigation_stack();
          }
        }
        ActiveBlock::Nodes => {
          if let Some(node) = handle_block_action(key, &mut app.data.nodes) {
            let _ok = handle_describe_or_yaml_action(
              key,
              app,
              &node,
              IoCmdEvent::GetDescribe {
                kind: "node".to_owned(),
                value: node.name.to_owned(),
                ns: None,
              },
            )
            .await;
          }
        }
        ActiveBlock::Pods => {
          if let Some(pod) = handle_block_action(key, &mut app.data.pods) {
            let ok = handle_describe_or_yaml_action(
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
          if let Some(c) = handle_block_action(key, &mut app.data.containers) {
            app.data.selected.container = Some(c.name.clone());
            app.dispatch_container_logs(c.name).await;
          }
        }
        ActiveBlock::Logs => {
          if key == DEFAULT_KEYBINDING.log_auto_scroll.key {
            app.log_auto_scroll = !app.log_auto_scroll;
          } else if key == DEFAULT_KEYBINDING.copy_to_clipboard.key {
            copy_to_clipboard(app.data.logs.get_plain_text());
          }
        }
        ActiveBlock::Describe | ActiveBlock::Yaml => {
          if key == DEFAULT_KEYBINDING.copy_to_clipboard.key {
            copy_to_clipboard(app.data.describe_out.get_txt());
          }
        }
        ActiveBlock::Services => {
          if let Some(res) = handle_block_action(key, &mut app.data.services) {
            let _ok = handle_describe_or_yaml_action(
              key,
              app,
              &res,
              IoCmdEvent::GetDescribe {
                kind: "service".to_owned(),
                value: res.name.to_owned(),
                ns: Some(res.namespace.to_owned()),
              },
            )
            .await;
          }
        }
        ActiveBlock::Deployments => {
          if let Some(res) = handle_block_action(key, &mut app.data.deployments) {
            let _ok = handle_describe_or_yaml_action(
              key,
              app,
              &res,
              IoCmdEvent::GetDescribe {
                kind: "deployment".to_owned(),
                value: res.name.to_owned(),
                ns: Some(res.namespace.to_owned()),
              },
            )
            .await;
          }
        }
        ActiveBlock::ConfigMaps => {
          if let Some(res) = handle_block_action(key, &mut app.data.config_maps) {
            let _ok = handle_describe_or_yaml_action(
              key,
              app,
              &res,
              IoCmdEvent::GetDescribe {
                kind: "configmap".to_owned(),
                value: res.name.to_owned(),
                ns: Some(res.namespace.to_owned()),
              },
            )
            .await;
          }
        }
        ActiveBlock::StatefulSets => {
          if let Some(res) = handle_block_action(key, &mut app.data.stateful_sets) {
            let _ok = handle_describe_or_yaml_action(
              key,
              app,
              &res,
              IoCmdEvent::GetDescribe {
                kind: "statefulset".to_owned(),
                value: res.name.to_owned(),
                ns: Some(res.namespace.to_owned()),
              },
            )
            .await;
          }
        }
        ActiveBlock::ReplicaSets => {
          if let Some(res) = handle_block_action(key, &mut app.data.replica_sets) {
            let _ok = handle_describe_or_yaml_action(
              key,
              app,
              &res,
              IoCmdEvent::GetDescribe {
                kind: "replicaset".to_owned(),
                value: res.name.to_owned(),
                ns: Some(res.namespace.to_owned()),
              },
            )
            .await;
          }
        }
        ActiveBlock::Jobs => {
          if let Some(res) = handle_block_action(key, &mut app.data.jobs) {
            let _ok = handle_describe_or_yaml_action(
              key,
              app,
              &res,
              IoCmdEvent::GetDescribe {
                kind: "job".to_owned(),
                value: res.name.to_owned(),
                ns: Some(res.namespace.to_owned()),
              },
            )
            .await;
          }
        }
        ActiveBlock::DaemonSets => {
          if let Some(res) = handle_block_action(key, &mut app.data.daemon_sets) {
            let _ok = handle_describe_or_yaml_action(
              key,
              app,
              &res,
              IoCmdEvent::GetDescribe {
                kind: "daemonset".to_owned(),
                value: res.name.to_owned(),
                ns: Some(res.namespace.to_owned()),
              },
            )
            .await;
          }
        }
        ActiveBlock::More => {
          if key == DEFAULT_KEYBINDING.submit.key {
            if let Some((_title, active_block)) = app
              .more_resources_menu
              .state
              .selected()
              .map(|i| app.more_resources_menu.items[i].clone())
            {
              app.push_navigation_route(Route {
                id: RouteId::Home,
                active_block,
              });
            }
          }
        }
        ActiveBlock::CronJobs => {
          if let Some(res) = handle_block_action(key, &mut app.data.cronjobs) {
            let _ok = handle_describe_or_yaml_action(
              key,
              app,
              &res,
              IoCmdEvent::GetDescribe {
                kind: "cronjob".to_owned(),
                value: res.name.to_owned(),
                ns: Some(res.namespace.to_owned()),
              },
            )
            .await;
          }
        }
        ActiveBlock::Secrets => {
          if let Some(res) = handle_block_action(key, &mut app.data.secrets) {
            let _ok = handle_describe_or_yaml_action(
              key,
              app,
              &res,
              IoCmdEvent::GetDescribe {
                kind: "secret".to_owned(),
                value: res.name.to_owned(),
                ns: Some(res.namespace.to_owned()),
              },
            )
            .await;
          }
        }
        ActiveBlock::RplCtrl => {
          if let Some(res) = handle_block_action(key, &mut app.data.rpl_ctrls) {
            let _ok = handle_describe_or_yaml_action(
              key,
              app,
              &res,
              IoCmdEvent::GetDescribe {
                kind: "replicationcontroller".to_owned(),
                value: res.name.to_owned(),
                ns: Some(res.namespace.to_owned()),
              },
            )
            .await;
          }
        }
        ActiveBlock::StorageClasses => {
          if let Some(res) = handle_block_action(key, &mut app.data.storage_classes) {
            let _ok = handle_describe_or_yaml_action(
              key,
              app,
              &res,
              IoCmdEvent::GetDescribe {
                kind: "storageclass".to_owned(),
                value: res.name.to_owned(),
                ns: None,
              },
            )
            .await;
          }
        }
        ActiveBlock::Roles => {
          if let Some(res) = handle_block_action(key, &mut app.data.roles) {
            let _ok = handle_describe_or_yaml_action(
              key,
              app,
              &res,
              IoCmdEvent::GetDescribe {
                kind: "roles".to_owned(),
                value: res.name.to_owned(),
                ns: Some(res.namespace.to_owned()),
              },
            )
            .await;
          }
        }
        ActiveBlock::ClusterRoles => {
          if let Some(res) = handle_block_action(key, &mut app.data.clusterroles) {
            let _ok = handle_describe_or_yaml_action(
              key,
              app,
              &res,
              IoCmdEvent::GetDescribe {
                kind: "clusterroles".to_owned(),
                value: res.name.to_owned(),
                ns: None,
              },
            )
            .await;
          }
        }
        ActiveBlock::Contexts | ActiveBlock::Utilization | ActiveBlock::Help => { /* Do nothing */ }
      }
    }
    RouteId::Contexts => {
      if let Some(ctx) = handle_block_action(key, &mut app.data.contexts) {
        app.data.selected.context = Some(ctx.name);
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

fn handle_block_action<T: Clone>(key: Key, item: &mut StatefulTable<T>) -> Option<T> {
  match key {
    _ if key == DEFAULT_KEYBINDING.submit.key
      || key == DEFAULT_KEYBINDING.describe_resource.key
      || key == DEFAULT_KEYBINDING.resource_yaml.key =>
    {
      item.get_selected_item_copy()
    }
    _ => None,
  }
}

async fn handle_block_scroll(app: &mut App, up: bool, is_mouse: bool, page: bool) {
  match app.get_current_route().active_block {
    ActiveBlock::Namespaces => app.data.namespaces.handle_scroll(up, page),
    ActiveBlock::Pods => app.data.pods.handle_scroll(up, page),
    ActiveBlock::Containers => app.data.containers.handle_scroll(up, page),
    ActiveBlock::Services => app.data.services.handle_scroll(up, page),
    ActiveBlock::Nodes => app.data.nodes.handle_scroll(up, page),
    ActiveBlock::ConfigMaps => app.data.config_maps.handle_scroll(up, page),
    ActiveBlock::StatefulSets => app.data.stateful_sets.handle_scroll(up, page),
    ActiveBlock::ReplicaSets => app.data.replica_sets.handle_scroll(up, page),
    ActiveBlock::Deployments => app.data.deployments.handle_scroll(up, page),
    ActiveBlock::Jobs => app.data.jobs.handle_scroll(up, page),
    ActiveBlock::DaemonSets => app.data.daemon_sets.handle_scroll(up, page),
    ActiveBlock::CronJobs => app.data.cronjobs.handle_scroll(up, page),
    ActiveBlock::Secrets => app.data.secrets.handle_scroll(up, page),
    ActiveBlock::RplCtrl => app.data.rpl_ctrls.handle_scroll(up, page),
    ActiveBlock::StorageClasses => app.data.storage_classes.handle_scroll(up, page),
    ActiveBlock::Roles => app.data.roles.handle_scroll(up, page),
    ActiveBlock::ClusterRoles => app.data.clusterroles.handle_scroll(up, page),
    ActiveBlock::Contexts => app.data.contexts.handle_scroll(up, page),
    ActiveBlock::Utilization => app.data.metrics.handle_scroll(up, page),
    ActiveBlock::Help => app.help_docs.handle_scroll(up, page),
    ActiveBlock::More => app.more_resources_menu.handle_scroll(up, page),
    ActiveBlock::Logs => {
      app.log_auto_scroll = false;
      app.data.logs.handle_scroll(inverse_dir(up, is_mouse), page);
    }
    ActiveBlock::Describe | ActiveBlock::Yaml => app
      .data
      .describe_out
      .handle_scroll(inverse_dir(up, is_mouse), page),
  }
}

#[cfg(target_arch = "x86_64")]
fn copy_to_clipboard(content: String) {
  use clipboard::{ClipboardContext, ClipboardProvider};

  let mut ctx: ClipboardContext = ClipboardProvider::new().expect("Unable to obtain clipboard");
  ctx
    .set_contents(content)
    .expect("Unable to set content to clipboard");
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
fn copy_to_clipboard(_content: String) {
  // do nothing as its a PITA to compile for ARM with XCB and this feature is not that important
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
  use super::*;
  use crate::app::{contexts::KubeContext, pods::KubePod};

  #[test]
  fn test_inverse_dir() {
    assert!(inverse_dir(true, false));
    assert!(!inverse_dir(true, true));
  }

  #[tokio::test]
  async fn test_handle_describe_or_yaml_action() {
    let mut app = App::default();

    app.route_home();
    assert_eq!(app.data.pods.state.selected(), None);

    let item = KubePod::default();

    assert!(
      handle_describe_or_yaml_action(
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
      handle_describe_or_yaml_action(
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
      !handle_describe_or_yaml_action(
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
}
