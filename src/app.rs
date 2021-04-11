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
  Error,
  HelpMenu,
  Home,
  BasicView,
}

#[derive(Clone, PartialEq, Debug)]
pub enum RouteId {
  BasicView,
  Error,
  Home,
}

#[derive(Debug)]
pub struct Route {
  pub id: RouteId,
  pub active_block: ActiveBlock,
  pub hovered_block: ActiveBlock,
}

const DEFAULT_ROUTE: Route = Route {
  id: RouteId::Home,
  active_block: ActiveBlock::Empty,
  hovered_block: ActiveBlock::Home,
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
}

pub struct App {
  navigation_stack: Vec<Route>,
  io_tx: Option<Sender<IoEvent>>,
  pub title: &'static str,
  pub should_quit: bool,
  pub tabs: TabsState,
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
  //   pub cluster_metrics:
  pub nodes: Vec<KubeNode>,

  // TODO useless
  pub progress: f64,
}

impl Default for App {
  fn default() -> Self {
    App {
      title: " KDash - The only Kubernetes dashboard you will ever need! ",
      tabs: TabsState::new(vec!["Overview", "Logs"]),
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
      // todo remove
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
    self.push_navigation_stack(RouteId::Error, ActiveBlock::Error);
    self.api_error = e.to_string();
  }

  // The navigation_stack actually only controls the large block to the right of `library` and
  // `playlists`
  pub fn push_navigation_stack(&mut self, next_route_id: RouteId, next_active_block: ActiveBlock) {
    self.navigation_stack.push(Route {
      id: next_route_id,
      active_block: next_active_block,
      hovered_block: next_active_block,
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

  pub fn set_current_route_state(
    &mut self,
    active_block: Option<ActiveBlock>,
    hovered_block: Option<ActiveBlock>,
  ) {
    let mut current_route = self.get_current_route_mut();
    if let Some(active_block) = active_block {
      current_route.active_block = active_block;
    }
    if let Some(hovered_block) = hovered_block {
      current_route.hovered_block = hovered_block;
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

  pub fn on_up(&mut self) {
    self.contexts.previous();
  }

  pub fn on_down(&mut self) {
    self.contexts.next();
  }

  pub fn on_right(&mut self) {
    self.tabs.next();
  }

  pub fn on_left(&mut self) {
    self.tabs.previous();
  }

  pub fn on_key(&mut self, c: Key) {
    match c {
      Key::Char('q') => {
        self.should_quit = true;
      }
      Key::Char('t') => {
        self.show_chart = !self.show_chart;
      }
      Key::Char('?') => {
        // TODO show help
      }
      Key::Left => self.on_left(),
      Key::Right => self.on_right(),
      Key::Up => self.on_up(),
      Key::Down => self.on_down(),
      _ => (),
    }
  }

  pub fn on_tick(&mut self) {
    self.tick_count = self.tick_count + 1;

    if self.tick_count == self.poll_tick_count {
      self.dispatch(IoEvent::GetNodes);
      self.tick_count = 0;
    }

    // TODO remove temp code
    self.progress += 0.001;
    if self.progress > 1.0 {
      self.progress = 0.0;
    }
  }
}
