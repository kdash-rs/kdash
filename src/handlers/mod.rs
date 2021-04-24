use crossterm::event::{MouseEvent, MouseEventKind};

use super::app::{
  key_binding::DEFAULT_KEYBINDING,
  metrics::GroupBy,
  models::{ScrollableTxt, StatefulTable},
  ActiveBlock, App, RouteId,
};
use super::cmd::IoCmdEvent;
use super::event::Key;

pub async fn handle_key_events(key: Key, app: &mut App) {
  // First handle any global event and then move to route event
  match key {
    _ if key == DEFAULT_KEYBINDING.esc.key => {
      handle_escape(app);
    }
    _ if key == DEFAULT_KEYBINDING.quit.key => {
      app.should_quit = true;
    }
    _ if key == DEFAULT_KEYBINDING.down.key => {
      handle_scroll(app, true, false).await;
    }
    _ if key == DEFAULT_KEYBINDING.up.key => {
      handle_scroll(app, false, false).await;
    }
    _ if key == DEFAULT_KEYBINDING.toggle_theme.key => {
      app.light_theme = !app.light_theme;
    }
    _ if key == DEFAULT_KEYBINDING.refresh.key => {
      app.refresh = true;
    }
    _ if key == DEFAULT_KEYBINDING.help.key => {
      app.push_navigation_stack(RouteId::HelpMenu, ActiveBlock::Empty)
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
  match app.get_current_route().id {
    RouteId::HelpMenu | RouteId::Contexts | RouteId::Error => {
      app.pop_navigation_stack();
    }
    _ => match app.get_current_route().active_block {
      ActiveBlock::Namespaces
      | ActiveBlock::Logs
      | ActiveBlock::Containers
      | ActiveBlock::Describe => {
        app.pop_navigation_stack();
      }
      _ => {}
    },
  }
}

// Handle event for the current active block
async fn handle_route_events(key: Key, app: &mut App) {
  // route specific events
  match app.get_current_route().id {
    // handle resource tabs on overview
    RouteId::Home => {
      match key {
        _ if key == DEFAULT_KEYBINDING.right.key => {
          app.context_tabs.next();
          app.push_navigation_stack(
            RouteId::Home,
            app.context_tabs.active_block.unwrap_or(ActiveBlock::Empty),
          );
        }
        _ if key == DEFAULT_KEYBINDING.left.key => {
          app.context_tabs.previous();
          app.push_navigation_stack(
            RouteId::Home,
            app.context_tabs.active_block.unwrap_or(ActiveBlock::Empty),
          );
        }
        _ if key == DEFAULT_KEYBINDING.toggle_info.key => {
          app.show_info_bar = !app.show_info_bar;
        }
        _ if key == DEFAULT_KEYBINDING.select_all_namespace.key => app.data.selected_ns = None,
        _ if key == DEFAULT_KEYBINDING.jump_to_namespace.key => {
          app.push_navigation_stack(RouteId::Home, ActiveBlock::Namespaces);
        }
        // as these are tabs with index the order here matters, atleast for readability
        _ if key == DEFAULT_KEYBINDING.jump_to_pods.key => {
          app.context_tabs.set_index(0);
          app.push_navigation_stack(RouteId::Home, ActiveBlock::Pods);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_services.key => {
          app.context_tabs.set_index(1);
          app.push_navigation_stack(RouteId::Home, ActiveBlock::Services);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_nodes.key => {
          app.context_tabs.set_index(2);
          app.push_navigation_stack(RouteId::Home, ActiveBlock::Nodes);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_configmaps.key => {
          app.context_tabs.set_index(3);
          app.push_navigation_stack(RouteId::Home, ActiveBlock::ConfigMaps);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_statefulsets.key => {
          app.context_tabs.set_index(4);
          app.push_navigation_stack(RouteId::Home, ActiveBlock::StatefulSets);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_replicasets.key => {
          app.context_tabs.set_index(5);
          app.push_navigation_stack(RouteId::Home, ActiveBlock::ReplicaSets);
        }
        _ if key == DEFAULT_KEYBINDING.jump_to_deployments.key => {
          app.context_tabs.set_index(6);
          app.push_navigation_stack(RouteId::Home, ActiveBlock::Deployments);
        }
        _ => {}
      };

      // handle block specific stuff
      match app.get_current_route().active_block {
        ActiveBlock::Pods => {
          if key == DEFAULT_KEYBINDING.describe_resource.key {
            app.data.describe_out = ScrollableTxt::new();
            let pod = app.data.pods.get_selected_item();
            if let Some(p) = pod {
              app.push_navigation_stack(RouteId::Home, ActiveBlock::Describe);
              app
                .dispatch_cmd(IoCmdEvent::GetDescribe {
                  kind: "pod".to_string(),
                  value: p.name,
                  ns: Some(p.namespace),
                })
                .await;
            }
          } else {
            let pod = handle_table_action(key, &mut app.data.pods);
            if pod.is_some() {
              app.push_navigation_stack(RouteId::Home, ActiveBlock::Containers);
            }
          }
        }
        ActiveBlock::Containers => {
          let cont = handle_table_action(
            key,
            &mut app
              .data
              .pods
              .get_selected_item()
              .map_or(StatefulTable::new(), |c| c.containers),
          );
          if let Some(c) = cont {
            app.dispatch_container_logs(c.name).await;
          }
        }
        ActiveBlock::Services => {
          let _svc = handle_table_action(key, &mut app.data.services);
        }
        ActiveBlock::Nodes => {
          if key == DEFAULT_KEYBINDING.describe_resource.key {
            app.data.describe_out = ScrollableTxt::new();
            let node = app.data.nodes.get_selected_item();
            if let Some(n) = node {
              app.push_navigation_stack(RouteId::Home, ActiveBlock::Describe);
              app
                .dispatch_cmd(IoCmdEvent::GetDescribe {
                  kind: "node".to_string(),
                  value: n.name,
                  ns: None,
                })
                .await;
            }
          } else {
            let _node = handle_table_action(key, &mut app.data.nodes);
          }
        }
        ActiveBlock::Namespaces => {
          let ns = handle_table_action(key, &mut app.data.namespaces);
          if let Some(v) = ns {
            app.data.selected_ns = Some(v.name);
            app.pop_navigation_stack();
          }
        }
        ActiveBlock::Logs => {
          if key == DEFAULT_KEYBINDING.log_auto_scroll.key {
            app.log_auto_scroll = !app.log_auto_scroll;
          }
        }
        _ => {
          // do nothing
        }
      }
    }
    RouteId::Contexts => {
      let _ctx = handle_table_action(key, &mut app.data.contexts);
    }
    RouteId::Utilization => {
      if key == DEFAULT_KEYBINDING.cycle_group_by.key {
        let next_group = match app.utilization_group_by.len() {
          4 => {
            vec![GroupBy::Resource, GroupBy::Node, GroupBy::Namespace]
          }
          3 => {
            vec![GroupBy::Resource, GroupBy::Node]
          }
          2 => {
            vec![GroupBy::Resource]
          }
          _ => {
            vec![
              GroupBy::Resource,
              GroupBy::Node,
              GroupBy::Namespace,
              GroupBy::Pod,
            ]
          }
        };
        app.utilization_group_by = next_group;
        app.tick_count = 0; // to force network request
      }
    }
    _ => {}
  }
  // reset tick_count so that network requests are made faster
  if key == DEFAULT_KEYBINDING.submit.key {
    app.tick_count = 0;
  }
}

fn handle_table_action<T: Clone>(key: Key, item: &mut StatefulTable<T>) -> Option<T> {
  match key {
    _ if key == DEFAULT_KEYBINDING.submit.key
      || key == DEFAULT_KEYBINDING.describe_resource.key =>
    {
      item.get_selected_item()
    }
    _ => None,
  }
}

fn handle_table_scroll<T: Clone>(item: &mut StatefulTable<T>, down: bool) {
  if down {
    item.previous();
  } else {
    item.next();
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
    ActiveBlock::Pods => handle_table_scroll(&mut app.data.pods, down),
    ActiveBlock::Containers => handle_table_scroll(
      &mut app
        .data
        .pods
        .get_selected_item()
        .map_or(StatefulTable::new(), |c| c.containers),
      down,
    ),
    ActiveBlock::Services => handle_table_scroll(&mut app.data.services, down),
    ActiveBlock::Nodes => handle_table_scroll(&mut app.data.nodes, down),
    ActiveBlock::Namespaces => handle_table_scroll(&mut app.data.namespaces, down),
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
    ActiveBlock::Describe => {
      if inverse_dir(down, is_mouse) {
        app.data.describe_out.scroll_down();
      } else {
        app.data.describe_out.scroll_up();
      }
    }
    _ => {}
  }
}
