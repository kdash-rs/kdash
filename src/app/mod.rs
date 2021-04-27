pub(crate) mod configmaps;
pub(crate) mod contexts;
pub(crate) mod deployments;
pub(crate) mod key_binding;
pub(crate) mod metrics;
pub(crate) mod models;
pub(crate) mod nodes;
pub(crate) mod ns;
pub(crate) mod pods;
pub(crate) mod replicasets;
pub(crate) mod statefulsets;
pub(crate) mod svcs;
mod utils;

use self::{
  configmaps::KubeConfigMap,
  contexts::KubeContext,
  deployments::KubeDeployments,
  key_binding::DEFAULT_KEYBINDING,
  metrics::{GroupBy, QtyByQualifier},
  models::{LogsState, ScrollableTxt, StatefulTable, TabsState},
  nodes::{KubeNode, NodeMetrics},
  ns::KubeNs,
  pods::{KubeContainer, KubePod},
  replicasets::KubeReplicaSet,
  statefulsets::KubeStatefulSet,
  svcs::KubeSvc,
};
use super::cmd::IoCmdEvent;
use super::network::{stream::IoStreamEvent, IoEvent};

use anyhow::anyhow;
use kube::config::Kubeconfig;
use tokio::sync::mpsc::Sender;
use tui::layout::Rect;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveBlock {
  Empty,
  Pods,
  Containers,
  Logs,
  Services,
  Nodes,
  Deployments,
  ConfigMaps,
  StatefulSets,
  ReplicaSets,
  Namespaces,
  Describe,
  Contexts,
  Utilization,
}

#[derive(Clone, PartialEq, Debug)]
pub enum RouteId {
  Error,
  Home,
  Contexts,
  Utilization,
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

/// Holds CLI version info
pub struct Cli {
  pub name: String,
  pub version: String,
  pub status: bool,
}

/// Holds data state for various views
pub struct Data {
  pub selected: Selected,
  pub clis: Vec<Cli>,
  pub kubeconfig: Option<Kubeconfig>,
  pub contexts: StatefulTable<KubeContext>,
  pub active_context: Option<KubeContext>,
  pub nodes: StatefulTable<KubeNode>,
  pub node_metrics: Vec<NodeMetrics>,
  pub namespaces: StatefulTable<KubeNs>,
  pub pods: StatefulTable<KubePod>,
  pub containers: StatefulTable<KubeContainer>,
  pub services: StatefulTable<KubeSvc>,
  pub config_maps: StatefulTable<KubeConfigMap>,
  pub stateful_sets: StatefulTable<KubeStatefulSet>,
  pub replica_sets: StatefulTable<KubeReplicaSet>,
  pub deployments: StatefulTable<KubeDeployments>,
  pub logs: LogsState,
  pub describe_out: ScrollableTxt,
  pub metrics: StatefulTable<(Vec<String>, Option<QtyByQualifier>)>,
}

/// selected data items
pub struct Selected {
  pub ns: Option<String>,
  pub pod: Option<String>,
  pub container: Option<String>,
  pub context: Option<String>,
}

/// Holds main application state
pub struct App {
  navigation_stack: Vec<Route>,
  io_tx: Option<Sender<IoEvent>>,
  io_stream_tx: Option<Sender<IoStreamEvent>>,
  io_cmd_tx: Option<Sender<IoCmdEvent>>,
  pub title: &'static str,
  pub should_quit: bool,
  pub main_tabs: TabsState,
  pub context_tabs: TabsState,
  pub show_info_bar: bool,
  pub is_loading: bool,
  pub is_streaming: bool,
  pub is_routing: bool,
  pub tick_until_poll: u64,
  pub tick_count: u64,
  pub enhanced_graphics: bool,
  pub table_cols: u16,
  pub size: Rect,
  pub api_error: String,
  pub dialog: Option<String>,
  pub confirm: bool,
  pub light_theme: bool,
  pub refresh: bool,
  pub log_auto_scroll: bool,
  pub utilization_group_by: Vec<GroupBy>,
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
      containers: StatefulTable::new(),
      services: StatefulTable::new(),
      config_maps: StatefulTable::new(),
      stateful_sets: StatefulTable::new(),
      replica_sets: StatefulTable::new(),
      deployments: StatefulTable::new(),
      selected: Selected {
        ns: None,
        pod: None,
        container: None,
        context: None,
      },
      logs: LogsState::new(String::default()),
      describe_out: ScrollableTxt::new(),
      metrics: StatefulTable::new(),
    }
  }
}

impl Default for App {
  fn default() -> Self {
    App {
      navigation_stack: vec![DEFAULT_ROUTE],
      io_tx: None,
      io_stream_tx: None,
      io_cmd_tx: None,
      title: " KDash - A simple Kubernetes dashboard ",
      should_quit: false,
      main_tabs: TabsState::new(vec![
        format!(
          "Active Context {}",
          DEFAULT_KEYBINDING.jump_to_current_context.key
        ),
        format!(
          "All Contexts {}",
          DEFAULT_KEYBINDING.jump_to_all_context.key
        ),
        format!("Utilization {}", DEFAULT_KEYBINDING.jump_to_utilization.key),
      ]),
      context_tabs: TabsState::with_active_blocks(
        vec![
          format!("Pods {}", DEFAULT_KEYBINDING.jump_to_pods.key),
          format!("Services {}", DEFAULT_KEYBINDING.jump_to_services.key),
          format!("Nodes {}", DEFAULT_KEYBINDING.jump_to_nodes.key),
          format!("ConfigMaps {}", DEFAULT_KEYBINDING.jump_to_configmaps.key),
          format!(
            "StatefulSets {}",
            DEFAULT_KEYBINDING.jump_to_statefulsets.key
          ),
          format!("ReplicaSets {}", DEFAULT_KEYBINDING.jump_to_replicasets.key),
          format!("Deployments {}", DEFAULT_KEYBINDING.jump_to_deployments.key),
        ],
        vec![
          ActiveBlock::Pods,
          ActiveBlock::Services,
          ActiveBlock::Nodes,
          ActiveBlock::ConfigMaps,
          ActiveBlock::StatefulSets,
          ActiveBlock::ReplicaSets,
          ActiveBlock::Deployments,
        ],
      ),
      show_info_bar: true,
      is_loading: false,
      is_streaming: false,
      is_routing: false,
      tick_until_poll: 0,
      tick_count: 0,
      enhanced_graphics: false,
      table_cols: 0,
      size: Rect::default(),
      api_error: String::new(),
      dialog: None,
      confirm: false,
      light_theme: false,
      refresh: true,
      log_auto_scroll: true,
      utilization_group_by: vec![
        GroupBy::Resource,
        GroupBy::Node,
        GroupBy::Namespace,
        GroupBy::Pod,
      ],
      data: Data::default(),
    }
  }
}

impl App {
  pub fn new(
    io_tx: Sender<IoEvent>,
    io_stream_tx: Sender<IoStreamEvent>,
    io_cmd_tx: Sender<IoCmdEvent>,
    enhanced_graphics: bool,
    tick_until_poll: u64,
  ) -> Self {
    App {
      io_tx: Some(io_tx),
      io_stream_tx: Some(io_stream_tx),
      io_cmd_tx: Some(io_cmd_tx),
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
  pub async fn dispatch(&mut self, action: IoEvent) {
    // `is_loading` will be set to false again after the async action has finished in network/mod.rs
    self.is_loading = true;
    if let Some(io_tx) = &self.io_tx {
      if let Err(e) = io_tx.send(action).await {
        self.is_loading = false;
        println!("Error from network dispatch {}", e);
        self.handle_error(anyhow!(e));
      };
    }
  }

  // Send a stream event to the stream network thread
  pub async fn dispatch_stream(&mut self, action: IoStreamEvent) {
    // `is_loading` will be set to false again after the async action has finished in network/stream.rs
    self.is_loading = true;
    if let Some(io_stream_tx) = &self.io_stream_tx {
      if let Err(e) = io_stream_tx.send(action).await {
        self.is_loading = false;
        println!("Error from stream dispatch {}", e);
        self.handle_error(anyhow!(e));
      };
    }
  }

  // Send a cmd event to the cmd runner thread
  pub async fn dispatch_cmd(&mut self, action: IoCmdEvent) {
    // `is_loading` will be set to false again after the async action has finished in network/stream.rs
    self.is_loading = true;
    if let Some(io_cmd_tx) = &self.io_cmd_tx {
      if let Err(e) = io_cmd_tx.send(action).await {
        self.is_loading = false;
        println!("Error from cmd dispatch {}", e);
        self.handle_error(anyhow!(e));
      };
    }
  }

  pub fn set_contexts(&mut self, contexts: Vec<KubeContext>) {
    self.data.active_context = contexts.iter().find_map(|ctx| {
      if ctx.is_active {
        Some(ctx.clone())
      } else {
        None
      }
    });
    self.data.contexts.set_items(contexts);
  }

  pub fn handle_error(&mut self, e: anyhow::Error) {
    self.push_navigation_stack(RouteId::Error, ActiveBlock::Empty);
    self.api_error = e.to_string();
  }

  pub fn push_navigation_stack(&mut self, id: RouteId, active_block: ActiveBlock) {
    self.navigation_stack.push(Route { id, active_block });
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

  pub fn get_prev_route(&self) -> &Route {
    // get the previous route
    &self.navigation_stack[self.navigation_stack.len() - 2]
  }

  pub fn route_home(&mut self) {
    self.main_tabs.set_index(0);
    self.push_navigation_stack(RouteId::Home, ActiveBlock::Pods);
  }

  pub fn route_contexts(&mut self) {
    self.main_tabs.set_index(1);
    self.push_navigation_stack(RouteId::Contexts, ActiveBlock::Contexts);
  }

  pub fn route_utilization(&mut self) {
    self.main_tabs.set_index(2);
    self.push_navigation_stack(RouteId::Utilization, ActiveBlock::Utilization);
  }

  pub async fn dispatch_container_logs(&mut self, id: String) {
    self.data.logs = LogsState::new(id);
    self.push_navigation_stack(RouteId::Home, ActiveBlock::Logs);
    self.dispatch_stream(IoStreamEvent::GetPodLogs(true)).await;
  }

  pub fn refresh(&mut self) {
    self.refresh = true
  }

  pub async fn on_tick(&mut self, first_render: bool) {
    // Make one time requests on first render or refresh
    if self.refresh {
      if !first_render {
        self.dispatch(IoEvent::RefreshClient).await;
        self.dispatch_stream(IoStreamEvent::RefreshClient).await;
      }
      self.dispatch_cmd(IoCmdEvent::GetCliInfo).await;
      self.dispatch(IoEvent::GetKubeConfig).await;
      // call these once  to pre-load data
      self.dispatch(IoEvent::GetPods).await;
      self.dispatch(IoEvent::GetServices).await;
      self.dispatch(IoEvent::GetConfigMaps).await;
      self.dispatch(IoEvent::GetStatefulSets).await;
      self.dispatch(IoEvent::GetReplicaSets).await;
      self.dispatch(IoEvent::GetDeployments).await;
    }
    // make network requests only in intervals to avoid hogging up the network
    if self.tick_count == 0 || self.is_routing {
      // make periodic network calls based on active route and active block to avoid hogging
      match self.get_current_route().id {
        RouteId::Home => {
          self.dispatch(IoEvent::GetNamespaces).await;
          self.dispatch(IoEvent::GetNodes).await;
          match self.get_current_route().active_block {
            ActiveBlock::Pods | ActiveBlock::Containers => {
              self.dispatch(IoEvent::GetPods).await;
            }
            ActiveBlock::Services => {
              self.dispatch(IoEvent::GetServices).await;
            }
            ActiveBlock::ConfigMaps => {
              self.dispatch(IoEvent::GetConfigMaps).await;
            }
            ActiveBlock::StatefulSets => {
              self.dispatch(IoEvent::GetStatefulSets).await;
            }
            ActiveBlock::ReplicaSets => {
              self.dispatch(IoEvent::GetReplicaSets).await;
            }
            ActiveBlock::Deployments => {
              self.dispatch(IoEvent::GetDeployments).await;
            }
            ActiveBlock::Logs => {
              if !self.is_streaming {
                self.dispatch_stream(IoStreamEvent::GetPodLogs(false)).await;
              }
            }
            _ => {}
          }
        }
        RouteId::Utilization => {
          self.dispatch(IoEvent::GetMetrics).await;
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
  }
}
