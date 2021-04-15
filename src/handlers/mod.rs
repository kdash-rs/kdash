use crate::app::{
  models::{StatefulTable, DEFAULT_KEYBINDING},
  ActiveBlock, App, RouteId,
};
use crate::event::Key;

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
    _ if key == DEFAULT_KEYBINDING.jump_to_namespace => {
      app.set_active_block(Some(ActiveBlock::Namespaces));
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_pods => {
      app.context_tabs.set_index(0);
      app.set_active_block(Some(ActiveBlock::Pods));
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_services => {
      app.context_tabs.set_index(1);
      app.set_active_block(Some(ActiveBlock::Services));
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_nodes => {
      app.context_tabs.set_index(2);
      app.set_active_block(Some(ActiveBlock::Nodes));
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_deployments => {
      app.context_tabs.set_index(3);
      app.set_active_block(Some(ActiveBlock::Deployments));
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_configmaps => {
      app.context_tabs.set_index(4);
      app.set_active_block(Some(ActiveBlock::ConfigMaps));
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_statefulsets => {
      app.context_tabs.set_index(5);
      app.set_active_block(Some(ActiveBlock::StatefulSets));
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_replicasets => {
      app.context_tabs.set_index(6);
      app.set_active_block(Some(ActiveBlock::ReplicaSets));
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
      let _pod = handle_table_events(key, &mut app.pods);
    }
    ActiveBlock::Services => {
      let _svc = handle_table_events(key, &mut app.services);
    }
    ActiveBlock::Nodes => {
      let _node = handle_table_events(key, &mut app.nodes);
    }
    ActiveBlock::Namespaces => {
      let ns = handle_table_events(key, &mut app.namespaces);
      match ns {
        Some(v) => app.selected_ns = Some(v.name),
        None => { /*Do nothing */ }
      }
    }
    ActiveBlock::Contexts => {
      let _ctx = handle_table_events(key, &mut app.contexts);
    }
    // ActiveBlock::Dialog(_) => {
    //   dialog::handler(key, app);
    // }
    _ => {
      // do nothing
    }
  }
  match app.get_current_route().id {
    RouteId::Home => {
      match key {
        _ if key == DEFAULT_KEYBINDING.right => {
          app.context_tabs.next();
          app.set_active_block(app.context_tabs.active_block);
        }
        _ if key == DEFAULT_KEYBINDING.left => {
          app.context_tabs.previous();
          app.set_active_block(app.context_tabs.active_block);
        }
        _ => {}
      };
    }
    _ => {
      // do nothing
    }
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
