use crate::{event::Key, network::IoEvent};
use anyhow::anyhow;
use duct::cmd;
use k8s_openapi::api::core::v1::{Event, Node};
use kube::{
  api::{Api, ListParams},
  config::{AuthInfo, Cluster, Context, Kubeconfig},
};
use std::{io, str::FromStr, sync::mpsc::Sender};
use tui::{
  layout::Rect,
  widgets::{ListState, TableState},
};

#[derive(Clone)]
pub struct KeyBindings {
  pub esc: Key,
  pub quit: Key,
  pub next_tab: Key,
  pub previous_tab: Key,
  pub up: Key,
  pub down: Key,
  pub jump_to_all_context: Key,
  pub jump_to_current_context: Key,
  pub jump_to_namespace: Key,
  pub jump_to_pods: Key,
  pub jump_to_services: Key,
  pub help: Key,
  pub submit: Key,
}

pub const DEFAULT_KEYBINDING: KeyBindings = KeyBindings {
  esc: Key::Esc,
  quit: Key::Char('q'),
  next_tab: Key::Left,
  previous_tab: Key::Right,
  up: Key::Up,
  down: Key::Down,
  jump_to_all_context: Key::Char('a'),
  jump_to_current_context: Key::Char('c'),
  jump_to_namespace: Key::Char('n'),
  jump_to_pods: Key::Char('p'),
  jump_to_services: Key::Char('s'),
  help: Key::Char('?'),
  submit: Key::Enter,
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

  pub fn with_items(items: Vec<T>) -> StatefulTable<T> {
    StatefulTable {
      state: TableState::default(),
      items,
    }
  }

  pub fn next(&mut self) {
    let i = match self.state.selected() {
      Some(i) => {
        if i >= self.items.len() - 1 {
          0
        } else {
          i + 1
        }
      }
      None => 0,
    };
    self.state.select(Some(i));
  }

  pub fn previous(&mut self) {
    let i = match self.state.selected() {
      Some(i) => {
        if i == 0 {
          self.items.len() - 1
        } else {
          i - 1
        }
      }
      None => 0,
    };
    self.state.select(Some(i));
  }

  pub fn unselect(&mut self) {
    self.state.select(None);
  }
}

pub struct CLI {
  pub name: String,
  pub version: String,
  pub status: bool,
}

pub struct TabsState {
  pub titles: Vec<&'static str>,
  pub index: usize,
}

impl TabsState {
  pub fn new(titles: Vec<&'static str>) -> TabsState {
    TabsState { titles, index: 0 }
  }
  pub fn next(&mut self) {
    self.index = (self.index + 1) % self.titles.len();
  }
  pub fn set_index(&mut self, index: usize) {
    self.index = index;
  }

  pub fn previous(&mut self) {
    if self.index > 0 {
      self.index -= 1;
    } else {
      self.index = self.titles.len() - 1;
    }
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveBlock {
  Empty,
  Pods,
  Services,
  Nodes,
  Namespaces,
  Contexts,
}

#[derive(Clone, PartialEq, Debug)]
pub enum RouteId {
  BasicView,
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

#[derive(Clone, PartialEq)]
pub struct KubeContext {
  pub name: String,
  pub cluster: String,
  pub user: String,
  pub namespace: Option<String>,
  pub is_active: bool,
}

pub struct KubeNode {
  pub name: String,
  pub status: String,
  pub cpu: u8,
  pub mem: u8,
}

pub struct KubeNs {
  pub name: String,
  pub status: String,
}
pub struct KubeSvs {
  pub name: String,
  pub type_: String,
}

pub struct KubePods {
  pub name: String,
  pub namespace: String,
  pub ready: String,
  pub restarts: u8,
  pub status: String,
  pub cpu: String,
  pub mem: String,
}

pub struct App {
  navigation_stack: Vec<Route>,
  io_tx: Option<Sender<IoEvent>>,
  pub title: &'static str,
  pub should_quit: bool,
  pub main_tabs: TabsState,
  pub context_tabs: TabsState,
  pub show_chart: bool,
  pub is_loading: bool,
  pub poll_tick_count: u64,
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
  pub clis: Vec<CLI>,
  pub kubeconfig: Option<Kubeconfig>,
  pub contexts: StatefulTable<KubeContext>,
  pub active_context: Option<KubeContext>,
  pub nodes: Vec<KubeNode>,
  pub namespaces: Vec<KubeNs>,
  pub pods: Vec<KubePods>,
  pub services: Vec<KubeSvs>,

  // TODO useless remove
  pub progress: f64,
}

impl Default for App {
  fn default() -> Self {
    App {
      title: " KDash - The only Kubernetes dashboard you will ever need! ",
      main_tabs: TabsState::new(vec!["Active Context (c)", "All Contexts (a)"]),
      context_tabs: TabsState::new(vec!["Pods (p)", "Services (s)", "Nodes"]),
      should_quit: false,
      poll_tick_count: 0,
      tick_count: 0,
      show_chart: true,
      is_loading: false,
      confirm: false,
      enhanced_graphics: false,
      home_scroll: 0,
      help_docs_size: 0,
      help_menu_page: 0,
      help_menu_max_lines: 0,
      help_menu_offset: 0,
      api_error: String::new(),
      io_tx: None,
      dialog: None,
      size: Rect::default(),
      navigation_stack: vec![DEFAULT_ROUTE],
      clis: vec![],
      kubeconfig: None,
      contexts: StatefulTable::new(),
      active_context: None,
      nodes: vec![],
      namespaces: vec![],
      pods: vec![],
      services: vec![],
      // TODO remove
      progress: 0.0,
    }
  }
}

impl App {
  pub fn new(io_tx: Sender<IoEvent>, enhanced_graphics: bool, poll_tick_count: u64) -> App {
    App {
      io_tx: Some(io_tx),
      enhanced_graphics,
      poll_tick_count,
      ..App::default()
    }
  }

  // Send a network event to the network thread
  pub fn dispatch(&mut self, action: IoEvent) {
    // `is_loading` will be set to false again after the async action has finished in network.rs
    self.is_loading = true;
    if let Some(io_tx) = &self.io_tx {
      if let Err(e) = io_tx.send(action) {
        self.is_loading = false;
        println!("Error from dispatch {}", e);
        // TODO: handle error
      };
    }
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
  }

  pub fn pop_navigation_stack(&mut self) -> Option<Route> {
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

  pub fn on_up(&mut self) {
    self.contexts.previous();
  }

  pub fn on_down(&mut self) {
    self.contexts.next();
  }

  pub fn on_right(&mut self) {
    self.context_tabs.next();
  }

  pub fn on_left(&mut self) {
    self.context_tabs.previous();
  }

  pub fn on_key(&mut self, c: Key) {
    if c == DEFAULT_KEYBINDING.quit {
      self.should_quit = true;
    } else if c == DEFAULT_KEYBINDING.esc {
      self.route_home();
    } else if c == DEFAULT_KEYBINDING.next_tab {
      self.on_left();
    } else if c == DEFAULT_KEYBINDING.previous_tab {
      self.on_right();
    } else if c == DEFAULT_KEYBINDING.up {
      self.on_up();
    } else if c == DEFAULT_KEYBINDING.down {
      self.on_down();
    } else if c == DEFAULT_KEYBINDING.help {
      self.push_navigation_stack(RouteId::HelpMenu, ActiveBlock::Empty)
    } else if c == DEFAULT_KEYBINDING.submit {
      // todo
    } else if c == DEFAULT_KEYBINDING.jump_to_all_context {
      self.route_contexts();
    } else if c == DEFAULT_KEYBINDING.jump_to_current_context {
      self.route_home();
    } else if c == DEFAULT_KEYBINDING.jump_to_namespace {
      self.set_active_block(Some(ActiveBlock::Namespaces))
    } else if c == DEFAULT_KEYBINDING.jump_to_pods {
      self.context_tabs.set_index(0)
    } else if c == DEFAULT_KEYBINDING.jump_to_services {
      self.context_tabs.set_index(1)
    }
  }

  pub fn on_tick(&mut self) {
    if self.tick_count == 0 {
      self.dispatch(IoEvent::GetNodes);
      self.dispatch(IoEvent::GetNamespaces);
      self.dispatch(IoEvent::GetPods);
      self.dispatch(IoEvent::GetServices);
    } else if self.tick_count == self.poll_tick_count {
      self.tick_count = 0;
    }
    self.tick_count += 1;

    // TODO remove temp code
    self.progress += 0.001;
    if self.progress > 1.0 {
      self.progress = 0.0;
    }
  }
}
