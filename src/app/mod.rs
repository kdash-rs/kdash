pub(crate) mod models;

use self::models::{StatefulTable, TabsState};
use super::network::IoEvent;

use anyhow::anyhow;
use kube::config::Kubeconfig;
use std::{sync::mpsc::Sender, u64};
use tui::layout::Rect;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveBlock {
  Empty,
  Pods,
  Services,
  Nodes,
  Deployments,
  ConfigMaps,
  StatefulSets,
  ReplicaSets,
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

pub struct Cli {
  pub name: String,
  pub version: String,
  pub status: bool,
}

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
pub struct NodeMetrics {
  pub name: String,
  pub cpu: String,
  pub cpu_percent: String,
  pub cpu_percent_i: f64,
  pub mem: String,
  pub mem_percent: String,
  pub mem_percent_i: f64,
}

#[derive(Clone)]
pub struct KubeNode {
  pub name: String,
  pub status: String,
  pub role: String,
  pub version: String,
  pub pods: i32,
  pub cpu: String,
  pub mem: String,
  pub cpu_percent: String,
  pub mem_percent: String,
  pub age: String,
}

#[derive(Clone)]
pub struct KubeNs {
  pub name: String,
  pub status: String,
}

#[derive(Clone)]
pub struct KubeSvs {
  pub namespace: String,
  pub name: String,
  pub type_: String,
  pub cluster_ip: String,
  pub external_ip: String,
  pub ports: String,
  pub age: String,
}

#[derive(Clone)]
pub struct KubePods {
  pub namespace: String,
  pub name: String,
  pub ready: String,
  pub status: String,
  pub restarts: i32,
  pub cpu: String,
  pub mem: String,
  pub age: String,
}

// main app state
pub struct App {
  navigation_stack: Vec<Route>,
  pub io_tx: Option<Sender<IoEvent>>,
  pub title: &'static str,
  pub should_quit: bool,
  pub main_tabs: TabsState,
  pub context_tabs: TabsState,
  pub show_info_bar: bool,
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
  pub clis: Vec<Cli>,
  pub kubeconfig: Option<Kubeconfig>,
  pub contexts: StatefulTable<KubeContext>,
  pub active_context: Option<KubeContext>,
  pub nodes: StatefulTable<KubeNode>,
  pub node_metrics: Vec<NodeMetrics>,
  pub namespaces: StatefulTable<KubeNs>,
  pub pods: StatefulTable<KubePods>,
  pub services: StatefulTable<KubeSvs>,
  pub selected_ns: Option<String>,
}

impl Default for App {
  fn default() -> Self {
    App {
      navigation_stack: vec![DEFAULT_ROUTE],
      io_tx: None,
      title: " KDash - A simple Kubernetes dashboard ",
      should_quit: false,
      main_tabs: TabsState::new(vec!["Active Context <a>", "All Contexts <c>"]),
      context_tabs: TabsState::with_active_blocks(
        vec![
          "Pods <p>",
          "Services <s>",
          "Nodes <N>",
          "Deployments <D>",
          "ConfigMaps <C>",
          "StatefulSets <S>",
          "ReplicaSets <R>",
        ],
        vec![
          ActiveBlock::Pods,
          ActiveBlock::Services,
          ActiveBlock::Nodes,
          ActiveBlock::Deployments,
          ActiveBlock::ConfigMaps,
          ActiveBlock::StatefulSets,
          ActiveBlock::ReplicaSets,
        ],
      ),
      show_info_bar: true,
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
      node_metrics: vec![],
      namespaces: StatefulTable::new(),
      pods: StatefulTable::new(),
      services: StatefulTable::new(),
      selected_ns: None,
    }
  }
}

impl App {
  pub fn new(io_tx: Sender<IoEvent>, enhanced_graphics: bool, tick_until_poll: u64) -> Self {
    App {
      io_tx: Some(io_tx),
      enhanced_graphics,
      tick_until_poll,
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
      self.dispatch(IoEvent::GetCliInfo);
      self.dispatch(IoEvent::GetKubeConfig);
    }
    // make network requests only in intervals to avoid hogging up the network
    if self.tick_count == 0 || self.is_routing {
      // make network calls based on active route and active block
      if self.get_current_route().id == RouteId::Home {
        self.dispatch(IoEvent::GetNamespaces);
        self.dispatch(IoEvent::GetTopNodes);
        match self.get_current_route().active_block {
          ActiveBlock::Pods => self.dispatch(IoEvent::GetPods),
          ActiveBlock::Services => self.dispatch(IoEvent::GetServices),
          ActiveBlock::Nodes => self.dispatch(IoEvent::GetNodes),
          _ => {}
        }
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
  }
}
