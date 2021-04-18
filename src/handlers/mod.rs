use super::app::{
  models::{StatefulTable, DEFAULT_KEYBINDING},
  ActiveBlock, App, RouteId,
};
use super::event::Key;

pub fn handle_app(key: Key, app: &mut App) {
  // First handle any global event and then move to block event
  match key {
    _ if key == DEFAULT_KEYBINDING.esc => {
      handle_escape(app);
    }
    _ if key == DEFAULT_KEYBINDING.quit => {
      app.should_quit = true;
    }
    _ if key == DEFAULT_KEYBINDING.toggle_theme => {
      app.light_theme = !app.light_theme;
    }
    _ if key == DEFAULT_KEYBINDING.refresh => {
      app.refresh = true;
    }
    _ if key == DEFAULT_KEYBINDING.help => {
      app.push_navigation_stack(RouteId::HelpMenu, ActiveBlock::Empty)
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_all_context => {
      app.route_contexts();
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_current_context => {
      app.route_home();
    }
    _ if key == DEFAULT_KEYBINDING.toggle_info => {
      app.show_info_bar = !app.show_info_bar;
    }
    _ if key == DEFAULT_KEYBINDING.select_all_namespace => app.data.selected_ns = None,
    _ if key == DEFAULT_KEYBINDING.jump_to_namespace => {
      app.push_navigation_stack(RouteId::Home, ActiveBlock::Namespaces);
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_pods => {
      app.context_tabs.set_index(0);
      app.push_navigation_stack(RouteId::Home, ActiveBlock::Pods);
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_services => {
      app.context_tabs.set_index(1);
      app.push_navigation_stack(RouteId::Home, ActiveBlock::Services);
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_nodes => {
      app.context_tabs.set_index(2);
      app.push_navigation_stack(RouteId::Home, ActiveBlock::Nodes);
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_deployments => {
      app.context_tabs.set_index(3);
      app.push_navigation_stack(RouteId::Home, ActiveBlock::Deployments);
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_configmaps => {
      app.context_tabs.set_index(4);
      app.push_navigation_stack(RouteId::Home, ActiveBlock::ConfigMaps);
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_statefulsets => {
      app.context_tabs.set_index(5);
      app.push_navigation_stack(RouteId::Home, ActiveBlock::StatefulSets);
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_replicasets => {
      app.context_tabs.set_index(6);
      app.push_navigation_stack(RouteId::Home, ActiveBlock::ReplicaSets);
    }
    _ => handle_block_events(key, app),
  }
}

fn handle_table_events<T: Clone>(key: Key, item: &mut StatefulTable<T>) -> Option<T> {
  match key {
    _ if key == DEFAULT_KEYBINDING.up => {
      item.previous();
      None
    }
    _ if key == DEFAULT_KEYBINDING.down => {
      item.next();
      None
    }
    _ if key == DEFAULT_KEYBINDING.submit => item.get_selected_item(),
    _ => None,
  }
}

// Handle event for the current active block
fn handle_block_events(key: Key, app: &mut App) {
  match app.get_current_route().active_block {
    ActiveBlock::Pods => {
      let pod = handle_table_events(key, &mut app.data.pods);
      if pod.is_some() {
        app.push_navigation_stack(RouteId::Home, ActiveBlock::Containers);
      }
    }
    ActiveBlock::Services => {
      let _svc = handle_table_events(key, &mut app.data.services);
    }
    ActiveBlock::Nodes => {
      let _node = handle_table_events(key, &mut app.data.nodes);
    }
    ActiveBlock::Namespaces => {
      let ns = handle_table_events(key, &mut app.data.namespaces);
      if let Some(v) = ns {
        app.data.selected_ns = Some(v.name);
        app.pop_navigation_stack();
      }
    }
    ActiveBlock::Contexts => {
      let _ctx = handle_table_events(key, &mut app.data.contexts);
    }
    // ActiveBlock::Dialog(_) => {
    //   dialog::handler(key, app);
    // }
    _ => {
      // do nothing
    }
  }
  if app.get_current_route().id == RouteId::Home {
    match key {
      _ if key == DEFAULT_KEYBINDING.right => {
        app.context_tabs.next();
        app.push_navigation_stack(
          RouteId::Home,
          app.context_tabs.active_block.unwrap_or(ActiveBlock::Empty),
        );
      }
      _ if key == DEFAULT_KEYBINDING.left => {
        app.context_tabs.previous();
        app.push_navigation_stack(
          RouteId::Home,
          app.context_tabs.active_block.unwrap_or(ActiveBlock::Empty),
        );
      }
      _ => {}
    };
  }
  // reset tick_count so that network requests are made faster
  if key == DEFAULT_KEYBINDING.submit {
    app.tick_count = 0;
  }
}

fn handle_escape(app: &mut App) {
  match app.get_current_route().id {
    // RouteId::HelpMenu => {
    //   app.route_home();
    // }
    _ => {
      app.route_home();
    }
  }
  match app.get_current_route().active_block {
    // ActiveBlock::Dialog(_) => {
    //   app.pop_navigation_stack();
    // }
    _ => {
      app.route_home();
    }
  }
}
