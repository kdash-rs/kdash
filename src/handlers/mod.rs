use crossterm::event::{MouseEvent, MouseEventKind};
use kubectl_view_allocations::GroupBy;
use serde::Serialize;

use super::{
  app::{
    key_binding::DEFAULT_KEYBINDING,
    models::{Scrollable, ScrollableTxt, StatefulTable},
    ActiveBlock, App, RouteId,
  },
  cmd::IoCmdEvent,
  event::Key,
};
use crate::app::models::KubeResource;

pub async fn handle_key_events(key: Key, app: &mut App) {
  // First handle any global event and then move to route event
  match key {
    _ if key == DEFAULT_KEYBINDING.esc.key => {
      handle_escape(app);
    }
    _ if key == DEFAULT_KEYBINDING.quit.key || key == DEFAULT_KEYBINDING.quit.alt.unwrap() => {
      app.should_quit = true;
    }
    _ if key == DEFAULT_KEYBINDING.down.key || key == DEFAULT_KEYBINDING.down.alt.unwrap() => {
      handle_scroll(app, true, false).await;
    }
    _ if key == DEFAULT_KEYBINDING.up.key || key == DEFAULT_KEYBINDING.up.alt.unwrap() => {
      handle_scroll(app, false, false).await;
    }
    _ if key == DEFAULT_KEYBINDING.toggle_theme.key => {
      app.light_theme = !app.light_theme;
    }
    _ if key == DEFAULT_KEYBINDING.refresh.key => {
      app.refresh();
    }
    _ if key == DEFAULT_KEYBINDING.help.key => {
      app.push_navigation_stack(RouteId::HelpMenu, ActiveBlock::Help)
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
    MouseEventKind::ScrollDown => handle_scroll(app, true, true).await,
    MouseEventKind::ScrollUp => handle_scroll(app, false, true).await,
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
      _ => {}
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
          app.push_navigation_stack(RouteId::Home, ActiveBlock::Namespaces);
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
          if let Some(ns) = handle_table_action(key, &mut app.data.namespaces) {
            app.data.selected.ns = Some(ns.name);
            app.pop_navigation_stack();
          }
        }
        ActiveBlock::Nodes => {
          if let Some(node) = handle_table_action(key, &mut app.data.nodes) {
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
          if let Some(pod) = handle_table_action(key, &mut app.data.pods) {
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
          if let Some(c) = handle_table_action(key, &mut app.data.containers) {
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
          if let Some(res) = handle_table_action(key, &mut app.data.services) {
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
          if let Some(res) = handle_table_action(key, &mut app.data.deployments) {
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
          if let Some(res) = handle_table_action(key, &mut app.data.config_maps) {
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
          if let Some(res) = handle_table_action(key, &mut app.data.stateful_sets) {
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
          if let Some(res) = handle_table_action(key, &mut app.data.replica_sets) {
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
          if let Some(res) = handle_table_action(key, &mut app.data.jobs) {
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
          if let Some(res) = handle_table_action(key, &mut app.data.daemon_sets) {
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
          if let Some(res) = handle_table_action(key, &mut app.data.cronjobs) {
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
        ActiveBlock::Contexts | ActiveBlock::Utilization | ActiveBlock::Help => { /* Do nothing */ }
      }
    }
    RouteId::Contexts => {
      if let Some(ctx) = handle_table_action(key, &mut app.data.contexts) {
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

fn handle_table_action<T: Clone>(key: Key, item: &mut StatefulTable<T>) -> Option<T> {
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

fn handle_table_scroll<T: Clone>(item: &mut StatefulTable<T>, down: bool) {
  if down {
    item.scroll_up();
  } else {
    item.scroll_down();
  }
}

// inverse direction for natural scrolling on mouse and keyboard
fn inverse_dir(down: bool, is_mouse: bool) -> bool {
  if is_mouse {
    down
  } else {
    !down
  }
}

async fn handle_scroll(app: &mut App, down: bool, is_mouse: bool) {
  match app.get_current_route().active_block {
    ActiveBlock::Namespaces => handle_table_scroll(&mut app.data.namespaces, down),
    ActiveBlock::Pods => handle_table_scroll(&mut app.data.pods, down),
    ActiveBlock::Containers => handle_table_scroll(&mut app.data.containers, down),
    ActiveBlock::Services => handle_table_scroll(&mut app.data.services, down),
    ActiveBlock::Nodes => handle_table_scroll(&mut app.data.nodes, down),
    ActiveBlock::ConfigMaps => handle_table_scroll(&mut app.data.config_maps, down),
    ActiveBlock::StatefulSets => handle_table_scroll(&mut app.data.stateful_sets, down),
    ActiveBlock::ReplicaSets => handle_table_scroll(&mut app.data.replica_sets, down),
    ActiveBlock::Deployments => handle_table_scroll(&mut app.data.deployments, down),
    ActiveBlock::Jobs => handle_table_scroll(&mut app.data.jobs, down),
    ActiveBlock::DaemonSets => handle_table_scroll(&mut app.data.daemon_sets, down),
    ActiveBlock::More => handle_table_scroll(&mut app.data.cronjobs, down), //TODO
    ActiveBlock::Contexts => handle_table_scroll(&mut app.data.contexts, down),
    ActiveBlock::Utilization => handle_table_scroll(&mut app.data.metrics, down),
    ActiveBlock::Logs => {
      app.log_auto_scroll = false;
      if inverse_dir(down, is_mouse) {
        app.data.logs.scroll_down();
      } else {
        app.data.logs.scroll_up();
      }
    }
    ActiveBlock::Describe | ActiveBlock::Yaml => {
      if inverse_dir(down, is_mouse) {
        app.data.describe_out.scroll_down();
      } else {
        app.data.describe_out.scroll_up();
      }
    }
    ActiveBlock::Help => handle_table_scroll(&mut app.help_docs, down),
  }
}

fn copy_to_clipboard(content: String) {
  use clipboard::{ClipboardContext, ClipboardProvider};

  let mut ctx: ClipboardContext = ClipboardProvider::new().expect("Unable to obtain clipboard");
  ctx
    .set_contents(content)
    .expect("Unable to set content to clipboard");
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::{contexts::KubeContext, pods::KubePod};

  #[test]
  fn test_inverse_dir() {
    assert!(!inverse_dir(true, false));
    assert!(inverse_dir(true, true));
  }

  #[test]
  fn test_handle_table_scroll() {
    let mut item: StatefulTable<&str> = StatefulTable::new();
    item.set_items(vec!["A", "B", "C"]);

    assert_eq!(item.state.selected(), Some(0));

    handle_table_scroll(&mut item, false);
    assert_eq!(item.state.selected(), Some(1));

    handle_table_scroll(&mut item, false);
    assert_eq!(item.state.selected(), Some(2));

    // circle back after last index
    handle_table_scroll(&mut item, false);
    assert_eq!(item.state.selected(), Some(2));
    // previous
    handle_table_scroll(&mut item, true);
    assert_eq!(item.state.selected(), Some(1));
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
      "---\napiVersion: v1\nkind: Pod\nmetadata: {}\n"
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
    handle_scroll(&mut app, false, true).await;
    assert_eq!(app.data.pods.state.selected(), Some(1));
    handle_scroll(&mut app, true, true).await;
    assert_eq!(app.data.pods.state.selected(), Some(0));

    // check logs keyboard scroll
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Logs);
    assert_eq!(app.data.logs.state.selected(), None);

    app.data.logs.add_record("record".to_string());
    app.data.logs.add_record("record 2".to_string());
    app.data.logs.add_record("record 3".to_string());

    handle_scroll(&mut app, true, false).await;
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
