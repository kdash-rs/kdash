use crate::app::{ActiveBlock, App, RouteId, StatefulTable, DEFAULT_KEYBINDING};
use crate::event::Key;

pub fn handle_app(key: Key, app: &mut App) {
  // First handle any global event and then move to block event
  match key {
    Key::Esc => {
      handle_escape(app);
    }
    _ if key == DEFAULT_KEYBINDING.quit => {
      app.should_quit = true;
    }
    _ if key == DEFAULT_KEYBINDING.help => {
      app.push_navigation_stack(RouteId::HelpMenu, ActiveBlock::Empty)
    }
    _ if key == DEFAULT_KEYBINDING.submit => {
      // todo
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_all_context => {
      app.route_contexts();
    }
    _ if key == DEFAULT_KEYBINDING.jump_to_current_context => {
      app.route_home();
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
    _ => handle_block_events(key, app),
  }
}

fn handle_table_up_down<T>(key: Key, item: &mut StatefulTable<T>) {
  match key {
    _ if key == DEFAULT_KEYBINDING.up => {
      item.previous();
    }
    _ if key == DEFAULT_KEYBINDING.down => {
      item.next();
    }
    _ => {}
  };
}

// Handle event for the current active block
fn handle_block_events(key: Key, app: &mut App) {
  match app.get_current_route().active_block {
    ActiveBlock::Pods => handle_table_up_down(key, &mut app.pods),
    ActiveBlock::Services => handle_table_up_down(key, &mut app.services),
    ActiveBlock::Nodes => handle_table_up_down(key, &mut app.nodes),
    ActiveBlock::Namespaces => handle_table_up_down(key, &mut app.namespaces),
    ActiveBlock::Contexts => handle_table_up_down(key, &mut app.contexts),
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
}

fn handle_escape(app: &mut App) {
  match app.get_current_route().id {
    RouteId::HelpMenu => {
      app.route_home();
    }
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
