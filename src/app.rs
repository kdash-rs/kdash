use crate::network::IoEvent;
use crate::util::{RandomSignal, SinSignal, StatefulList};
use anyhow::anyhow;
use duct::cmd;
use kube::config::{AuthInfo, Cluster, Context, Kubeconfig};
use std::{io, str::FromStr, sync::mpsc::Sender};
use tui::{
  layout::Rect,
  widgets::{ListState, TableState},
};

const TASKS: [&str; 24] = [
  "Item1", "Item2", "Item3", "Item4", "Item5", "Item6", "Item7", "Item8", "Item9", "Item10",
  "Item11", "Item12", "Item13", "Item14", "Item15", "Item16", "Item17", "Item18", "Item19",
  "Item20", "Item21", "Item22", "Item23", "Item24",
];

const LOGS: [(&str, &str); 3] = [
  ("Event1", "INFO"),
  ("Event2", "INFO"),
  ("Event3", "CRITICAL"),
];

const EVENTS: [(&str, u64); 3] = [("B1", 9), ("B2", 12), ("B3", 5)];

pub struct Signal<S: Iterator> {
  source: S,
  pub points: Vec<S::Item>,
  tick_rate: usize,
}

impl<S> Signal<S>
where
  S: Iterator,
{
  fn on_tick(&mut self) {
    for _ in 0..self.tick_rate {
      self.points.remove(0);
    }
    self
      .points
      .extend(self.source.by_ref().take(self.tick_rate));
  }
}

pub struct Signals {
  pub sin1: Signal<SinSignal>,
  pub sin2: Signal<SinSignal>,
  pub window: [f64; 2],
}

impl Signals {
  fn on_tick(&mut self) {
    self.sin1.on_tick();
    self.sin2.on_tick();
    self.window[0] += 1.0;
    self.window[1] += 1.0;
  }
}
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

pub struct Node {
  pub value: String,
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

pub struct App {
  navigation_stack: Vec<Route>,
  pub title: &'static str,
  pub should_quit: bool,
  pub tabs: TabsState,
  pub show_chart: bool,
  pub is_loading: bool,
  pub enhanced_graphics: bool,
  io_tx: Option<Sender<IoEvent>>,
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

  // TODO useless
  pub progress: f64,
  pub tasks: StatefulList<&'static str>,
  pub logs: StatefulList<(&'static str, &'static str)>,
  pub signals: Signals,
  pub barchart: Vec<(&'static str, u64)>,
}

impl Default for App {
  fn default() -> Self {
    let mut sin_signal = SinSignal::new(0.2, 3.0, 18.0);
    let sin1_points = sin_signal.by_ref().take(100).collect();
    let mut sin_signal2 = SinSignal::new(0.1, 2.0, 10.0);
    let sin2_points = sin_signal2.by_ref().take(200).collect();

    App {
      title: " KDash - The only Kubernetes dashboard you will ever need! ",
      should_quit: false,
      tabs: TabsState::new(vec!["Overview", "Logs"]),
      show_chart: true,
      enhanced_graphics: false,
      home_scroll: 0,
      api_error: String::new(),
      help_docs_size: 0,
      help_menu_page: 0,
      help_menu_max_lines: 0,
      help_menu_offset: 0,
      is_loading: false,
      io_tx: None,
      dialog: None,
      confirm: false,
      size: Rect::default(),
      navigation_stack: vec![DEFAULT_ROUTE],
      clis: vec![],
      kubeconfig: None,
      contexts: StatefulTable::new(),
      active_context: None,
      // todo remove
      progress: 0.0,
      tasks: StatefulList::with_items(TASKS.to_vec()),
      logs: StatefulList::with_items(LOGS.to_vec()),
      signals: Signals {
        sin1: Signal {
          source: sin_signal,
          points: sin1_points,
          tick_rate: 5,
        },
        sin2: Signal {
          source: sin_signal2,
          points: sin2_points,
          tick_rate: 10,
        },
        window: [0.0, 20.0],
      },
      barchart: EVENTS.to_vec(),
    }
  }
}

impl App {
  pub fn new(io_tx: Sender<IoEvent>, enhanced_graphics: bool) -> App {
    App {
      io_tx: Some(io_tx),
      enhanced_graphics,
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

  pub fn on_key(&mut self, c: char) {
    match c {
      'q' => {
        self.should_quit = true;
      }
      't' => {
        self.show_chart = !self.show_chart;
      }
      _ => {}
    }
  }

  pub fn on_tick(&mut self) {
    // Update progress
    self.progress += 0.001;
    if self.progress > 1.0 {
      self.progress = 0.0;
    }

    self.signals.on_tick();

    let log = self.logs.items.pop().unwrap();
    self.logs.items.insert(0, log);

    let event = self.barchart.pop().unwrap();
    self.barchart.insert(0, event);
  }
}
