pub(crate) mod models;

use self::models::{StatefulTable, TabsState, DEFAULT_KEYBINDING};
use super::network::IoEvent;

use anyhow::anyhow;
use kube::config::Kubeconfig;
use std::sync::mpsc::Sender;
use tui::layout::Rect;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveBlock {
  Empty,
  Pods,
  Containers,
  Services,
  Nodes,
  Deployments,
  ConfigMaps,
  StatefulSets,
  ReplicaSets,
  Namespaces,
  Contexts,
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
  pub cpu_percent: f64,
  pub mem: String,
  pub mem_percent: f64,
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
  pub cpu_a: String,
  pub mem_a: String,
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
pub struct KubeContainers {
  pub name: String,
  pub image: String,
  pub ready: String,
  pub status: String,
  pub restarts: i32,
  pub liveliness_probe: bool,
  pub readiness_probe: bool,
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
  pub containers: StatefulTable<KubeContainers>,
}

pub struct Data {
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
  pub home_scroll: u16,
  pub table_cols: u16,
  pub size: Rect,
  pub api_error: String,
  pub dialog: Option<String>,
  pub confirm: bool,
  pub light_theme: bool,
  pub refresh: bool,
  pub data: Data,
}

impl Default for Data {
  fn default() -> Self {
    Data {
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

impl Default for App {
  fn default() -> Self {
    App {
      navigation_stack: vec![DEFAULT_ROUTE],
      io_tx: None,
      title: " KDash - A simple Kubernetes dashboard ",
      should_quit: false,
      main_tabs: TabsState::new(vec![
        format!(
          "Active Context {}",
          DEFAULT_KEYBINDING.jump_to_current_context
        ),
        format!("All Contexts {}", DEFAULT_KEYBINDING.jump_to_all_context),
      ]),
      context_tabs: TabsState::with_active_blocks(
        vec![
          format!("Pods {}", DEFAULT_KEYBINDING.jump_to_pods),
          format!("Services {}", DEFAULT_KEYBINDING.jump_to_services),
          format!("Nodes {}", DEFAULT_KEYBINDING.jump_to_nodes),
          format!("Deployments {}", DEFAULT_KEYBINDING.jump_to_deployments),
          format!("ConfigMaps {}", DEFAULT_KEYBINDING.jump_to_configmaps),
          format!("StatefulSets {}", DEFAULT_KEYBINDING.jump_to_statefulsets),
          format!("ReplicaSets {}", DEFAULT_KEYBINDING.jump_to_replicasets),
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
      tick_until_poll: 0,
      tick_count: 0,
      enhanced_graphics: false,
      home_scroll: 0,
      table_cols: 0,
      size: Rect::default(),
      api_error: String::new(),
      dialog: None,
      confirm: false,
      light_theme: false,
      refresh: true,
      data: Data::default(),
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

  pub fn reset(&mut self) {
    self.api_error = String::new();
    self.data = Data::default();
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
    self.data.active_context =
      contexts
        .iter()
        .find_map(|it| if it.is_active { Some(it.clone()) } else { None });
    self.data.contexts.set_items(contexts);
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
      // call these once as well to pre-load data
      self.dispatch(IoEvent::GetPods);
      self.dispatch(IoEvent::GetServices);
    }
    // make network requests only in intervals to avoid hogging up the network
    if self.tick_count == 0 || self.is_routing {
      // make periodic network calls based on active route and active block to avoid hogging
      if self.get_current_route().id == RouteId::Home {
        self.dispatch(IoEvent::GetNamespaces);
        self.dispatch(IoEvent::GetNodes);
        match self.get_current_route().active_block {
          ActiveBlock::Pods => self.dispatch(IoEvent::GetPods),
          ActiveBlock::Services => self.dispatch(IoEvent::GetServices),
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
