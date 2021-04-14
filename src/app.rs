use crate::{event::Key, network::IoEvent};
use anyhow::anyhow;
use kube::config::Kubeconfig;
use std::sync::mpsc::Sender;
use tui::{layout::Rect, widgets::TableState};

#[derive(Clone)]
pub struct KeyBindings {
  pub esc: Key,
  pub quit: Key,
  pub help: Key,
  pub submit: Key,
  pub left: Key,
  pub right: Key,
  pub up: Key,
  pub down: Key,
  pub refresh: Key,
  pub toggle_theme: Key,
  pub jump_to_all_context: Key,
  pub jump_to_current_context: Key,
  pub jump_to_namespace: Key,
  pub jump_to_pods: Key,
  pub jump_to_services: Key,
  pub jump_to_nodes: Key,
}

pub const DEFAULT_KEYBINDING: KeyBindings = KeyBindings {
  esc: Key::Esc,
  quit: Key::Char('q'),
  help: Key::Char('?'),
  submit: Key::Enter,
  left: Key::Left,
  right: Key::Right,
  up: Key::Up,
  down: Key::Down,
  refresh: Key::Ctrl('r'),
  toggle_theme: Key::Char('t'),
  jump_to_all_context: Key::Char('a'),
  jump_to_current_context: Key::Char('c'),
  jump_to_namespace: Key::Char('n'),
  jump_to_pods: Key::Char('p'),
  jump_to_services: Key::Char('s'),
  jump_to_nodes: Key::Char('N'),
};

pub struct StatefulTable<T> {
  pub state: TableState,
  pub items: Vec<T>,
}

impl<T> StatefulTable<T> {
  pub fn new() -> StatefulTable<T> {
    StatefulTable {
      state: TableState::default(),
      items: Vec::new(),
    }
  }

  pub fn set_items(&mut self, items: Vec<T>) {
    self.items = items;
    if self.items.len() > 0 {
      let i = self
        .state
        .selected()
        .map_or(0, |i| if i > 0 { i } else { 0 });
      self.state.select(Some(i));
    }
  }

  pub fn next(&mut self) {
    let i = self.state.selected().map_or(0, |i| {
      if i >= self.items.len().wrapping_sub(1) {
        0
      } else {
        i + 1
      }
    });
    self.state.select(Some(i));
  }

  pub fn previous(&mut self) {
    let i = self.state.selected().map_or(0, |i| {
      if i == 0 {
        self.items.len().wrapping_sub(1)
      } else {
        i - 1
      }
    });
    self.state.select(Some(i));
  }

  pub fn unselect(&mut self) {
    self.state.select(None);
  }
}

impl<T: Clone> StatefulTable<T> {
  pub fn get_selected_item(&mut self) -> Option<T> {
    self
      .state
      .selected()
      .and_then(|i| Some(self.items[i].clone()))
  }
}

pub struct TabsState {
  pub titles: Vec<&'static str>,
  pub index: usize,
  pub active_block_ids: Option<Vec<ActiveBlock>>,
  pub active_block: Option<ActiveBlock>,
}

impl TabsState {
  pub fn new(titles: Vec<&'static str>) -> TabsState {
    TabsState {
      titles,
      index: 0,
      active_block_ids: None,
      active_block: None,
    }
  }
  pub fn with_active_blocks(titles: Vec<&'static str>, blocks: Vec<ActiveBlock>) -> TabsState {
    TabsState {
      titles,
      index: 0,
      active_block: Some(blocks[0]),
      active_block_ids: Some(blocks),
    }
  }
  pub fn set_index(&mut self, index: usize) {
    self.index = index;
    self.set_active();
  }
  pub fn set_active(&mut self) {
    self.active_block = self
      .active_block_ids
      .as_ref()
      .and_then(|ids| Some(ids[self.index]));
  }
  pub fn next(&mut self) {
    self.index = (self.index + 1) % self.titles.len();
    self.set_active();
  }
  pub fn previous(&mut self) {
    if self.index > 0 {
      self.index -= 1;
    } else {
      self.index = self.titles.len() - 1;
    }
    self.set_active();
  }
}

pub struct CLI {
  pub name: String,
  pub version: String,
  pub status: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveBlock {
  Empty,
  Pods,
  Services,
  Nodes,
  Namespaces,
  Contexts,
  Dialog(),
}

#[derive(Clone, PartialEq, Debug)]
pub enum RouteId {
  Error,
  Home,
  Contexts,
  HelpMenu,
}

#[derive(Debug)]
pub struct Route {
  pub id: RouteId,
  pub active_block: ActiveBlock,
}

const DEFAULT_ROUTE: Route = Route {
  id: RouteId::Home,
  active_block: ActiveBlock::Pods,
};

// struts for kubernetes data
#[derive(Clone)]
pub struct KubeContext {
  pub name: String,
  pub cluster: String,
  pub user: String,
  pub namespace: Option<String>,
  pub is_active: bool,
}

#[derive(Clone)]
pub struct KubeNode {
  pub name: String,
  pub status: String,
  pub cpu: u8,
  pub mem: u8,
}

#[derive(Clone)]
pub struct KubeNs {
  pub name: String,
  pub status: String,
}

#[derive(Clone)]
pub struct KubeSvs {
  pub name: String,
  pub type_: String,
}

#[derive(Clone)]
pub struct KubePods {
  pub name: String,
  pub namespace: String,
  pub ready: String,
  pub restarts: u8,
  pub status: String,
  pub cpu: String,
  pub mem: String,
}

// main app state
pub struct App {
  navigation_stack: Vec<Route>,
  pub io_tx: Option<Sender<IoEvent>>,
  pub title: &'static str,
  pub should_quit: bool,
  pub main_tabs: TabsState,
  pub context_tabs: TabsState,
  pub show_chart: bool,
  pub is_loading: bool,
  pub is_routing: bool,
  pub tick_until_poll: u64,
  pub tick_count: u64,
  pub enhanced_graphics: bool,
  pub help_docs_size: u32,
  pub help_menu_page: u32,
  pub help_menu_max_lines: u32,
  pub help_menu_offset: u32,
  pub home_scroll: u16,
  pub api_error: String,
  pub dialog: Option<String>,
  pub confirm: bool,
  pub size: Rect,
  pub light_theme: bool,
  pub refresh: bool,
  pub clis: Vec<CLI>,
  pub kubeconfig: Option<Kubeconfig>,
  pub contexts: StatefulTable<KubeContext>,
  pub active_context: Option<KubeContext>,
  pub nodes: StatefulTable<KubeNode>,
  pub namespaces: StatefulTable<KubeNs>,
  pub pods: StatefulTable<KubePods>,
  pub services: StatefulTable<KubeSvs>,
  pub selected_ns: Option<String>,
  // TODO useless remove
  pub progress: f64,
}

impl Default for App {
  fn default() -> Self {
    App {
      navigation_stack: vec![DEFAULT_ROUTE],
      io_tx: None,
      title: " KDash - The only Kubernetes dashboard you will ever need! ",
      should_quit: false,
      main_tabs: TabsState::new(vec!["Active Context <c>", "All Contexts <a>"]),
      context_tabs: TabsState::with_active_blocks(
        vec!["Pods <p>", "Services <s>", "Nodes <N>"],
        vec![ActiveBlock::Pods, ActiveBlock::Services, ActiveBlock::Nodes],
      ),
      show_chart: true,
      is_loading: false,
      is_routing: false,
      light_theme: false,
      refresh: true,
      tick_until_poll: 0,
      tick_count: 0,
      enhanced_graphics: false,
      help_docs_size: 0,
      help_menu_page: 0,
      help_menu_max_lines: 0,
      help_menu_offset: 0,
      home_scroll: 0,
      dialog: None,
      confirm: false,
      size: Rect::default(),
      api_error: String::new(),
      clis: vec![],
      kubeconfig: None,
      contexts: StatefulTable::new(),
      active_context: None,
      nodes: StatefulTable::new(),
      namespaces: StatefulTable::new(),
      pods: StatefulTable::new(),
      services: StatefulTable::new(),
      selected_ns: None,
      // TODO remove
      progress: 0.0,
    }
  }
}

impl App {
  pub fn new(io_tx: Sender<IoEvent>, enhanced_graphics: bool, tick_until_poll: u64) -> App {
    App {
      io_tx: Some(io_tx),
      enhanced_graphics,
      tick_until_poll: tick_until_poll,
      ..App::default()
    }
  }

  // TODO find a better way to do this
  pub fn reset(&mut self) {
    self.api_error = String::new();
    self.clis = vec![];
    self.kubeconfig = None;
    self.contexts = StatefulTable::new();
    self.active_context = None;
    self.nodes = StatefulTable::new();
    self.namespaces = StatefulTable::new();
    self.pods = StatefulTable::new();
    self.services = StatefulTable::new();
    self.selected_ns = None;
  }

  // Send a network event to the network thread
  pub fn dispatch(&mut self, action: IoEvent) {
    // `is_loading` will be set to false again after the async action has finished in network.rs
    self.is_loading = true;
    if let Some(io_tx) = &self.io_tx {
      if let Err(e) = io_tx.send(action) {
        self.is_loading = false;
        println!("Error from dispatch {}", e);
        self.handle_error(anyhow!(e));
      };
    }
  }

  pub fn set_contexts(&mut self, contexts: Vec<KubeContext>) {
    self.active_context =
      contexts
        .iter()
        .find_map(|it| if it.is_active { Some(it.clone()) } else { None });
    self.contexts.set_items(contexts);
  }

  pub fn handle_error(&mut self, e: anyhow::Error) {
    self.push_navigation_stack(RouteId::Error, ActiveBlock::Empty);
    self.api_error = e.to_string();
  }

  pub fn push_navigation_stack(&mut self, next_route_id: RouteId, next_active_block: ActiveBlock) {
    self.navigation_stack.push(Route {
      id: next_route_id,
      active_block: next_active_block,
    });
    self.is_routing = true;
  }

  pub fn pop_navigation_stack(&mut self) -> Option<Route> {
    self.is_routing = true;
    if self.navigation_stack.len() == 1 {
      None
    } else {
      self.navigation_stack.pop()
    }
  }

  pub fn get_current_route(&self) -> &Route {
    // if for some reason there is no route return the default
    self.navigation_stack.last().unwrap_or(&DEFAULT_ROUTE)
  }

  fn get_current_route_mut(&mut self) -> &mut Route {
    self.navigation_stack.last_mut().unwrap()
  }

  pub fn set_active_block(&mut self, active_block: Option<ActiveBlock>) {
    let mut current_route = self.get_current_route_mut();
    if let Some(active_block) = active_block {
      current_route.active_block = active_block;
    }
    self.is_routing = true;
  }

  pub fn calculate_help_menu_offset(&mut self) {
    let old_offset = self.help_menu_offset;

    if self.help_menu_max_lines < self.help_docs_size {
      self.help_menu_offset = self.help_menu_page * self.help_menu_max_lines;
    }
    if self.help_menu_offset > self.help_docs_size {
      self.help_menu_offset = old_offset;
      self.help_menu_page -= 1;
    }
  }

  pub fn route_home(&mut self) {
    self.main_tabs.set_index(0);
    self.push_navigation_stack(RouteId::Home, ActiveBlock::Pods);
  }

  pub fn route_contexts(&mut self) {
    self.main_tabs.set_index(1);
    self.push_navigation_stack(RouteId::Contexts, ActiveBlock::Contexts);
  }

  pub fn on_tick(&mut self, first_render: bool) {
    // Make one time requests on first render or refresh
    if self.refresh {
      if !first_render {
        self.dispatch(IoEvent::RefreshClient);
      }
      self.dispatch(IoEvent::GetCLIInfo);
      self.dispatch(IoEvent::GetKubeConfig);
    }
    // make network requests only in intervals to avoid hogging up the network
    if self.tick_count == 0 || self.is_routing {
      // make network calls based on active route and active block
      match self.get_current_route().id {
        RouteId::Home => {
          self.dispatch(IoEvent::GetNamespaces);

          match self.get_current_route().active_block {
            ActiveBlock::Pods => self.dispatch(IoEvent::GetPods),
            ActiveBlock::Services => self.dispatch(IoEvent::GetServices),
            ActiveBlock::Nodes => self.dispatch(IoEvent::GetNodes),
            _ => {}
          }
        }
        _ => {}
      }
      self.is_routing = false;
    }

    self.tick_count += 1;

    if self.tick_count > self.tick_until_poll {
      self.tick_count = 0; // reset ticks
    }

    // route to home after all network requests to avoid showing error again
    if self.refresh {
      if !first_render {
        self.route_home();
      }
      self.refresh = false;
    }
    // TODO remove temp code
    self.progress += 0.001;
    if self.progress > 1.0 {
      self.progress = 0.0;
    }
  }
}
