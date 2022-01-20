pub(crate) mod configmaps;
pub(crate) mod contexts;
pub(crate) mod cronjobs;
pub(crate) mod daemonsets;
pub(crate) mod deployments;
pub(crate) mod jobs;
pub(crate) mod key_binding;
pub(crate) mod metrics;
pub(crate) mod models;
pub(crate) mod nodes;
pub(crate) mod ns;
pub(crate) mod pods;
pub(crate) mod replicasets;
pub(crate) mod replication_controllers;
pub(crate) mod secrets;
pub(crate) mod statefulsets;
pub(crate) mod svcs;
mod utils;

use anyhow::anyhow;
use kube::config::Kubeconfig;
use kubectl_view_allocations::{GroupBy, QtyByQualifier};
use tokio::sync::mpsc::Sender;
use tui::layout::Rect;

use self::{
  configmaps::KubeConfigMap,
  contexts::KubeContext,
  cronjobs::KubeCronJob,
  daemonsets::KubeDaemonSet,
  deployments::KubeDeployment,
  jobs::KubeJob,
  key_binding::DEFAULT_KEYBINDING,
  metrics::KubeNodeMetrics,
  models::{LogsState, ScrollableTxt, StatefulList, StatefulTable, TabRoute, TabsState},
  nodes::KubeNode,
  ns::KubeNs,
  pods::{KubeContainer, KubePod},
  replicasets::KubeReplicaSet,
  replication_controllers::KubeReplicationController,
  secrets::KubeSecret,
  statefulsets::KubeStatefulSet,
  svcs::KubeSvc,
};
use super::{
  cmd::IoCmdEvent,
  network::{stream::IoStreamEvent, IoEvent},
};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveBlock {
  Help,
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
  Yaml,
  Contexts,
  Utilization,
  Jobs,
  DaemonSets,
  CronJobs,
  Secrets,
  RplCtrl,
  More,
}

#[derive(Clone, PartialEq, Debug)]
pub enum RouteId {
  Home,
  Contexts,
  Utilization,
  HelpMenu,
}

#[derive(Debug, Clone)]
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
  pub node_metrics: Vec<KubeNodeMetrics>,
  pub namespaces: StatefulTable<KubeNs>,
  pub pods: StatefulTable<KubePod>,
  pub containers: StatefulTable<KubeContainer>,
  pub services: StatefulTable<KubeSvc>,
  pub config_maps: StatefulTable<KubeConfigMap>,
  pub stateful_sets: StatefulTable<KubeStatefulSet>,
  pub replica_sets: StatefulTable<KubeReplicaSet>,
  pub deployments: StatefulTable<KubeDeployment>,
  pub jobs: StatefulTable<KubeJob>,
  pub daemon_sets: StatefulTable<KubeDaemonSet>,
  pub cronjobs: StatefulTable<KubeCronJob>,
  pub secrets: StatefulTable<KubeSecret>,
  pub rpl_ctrls: StatefulTable<KubeReplicationController>,
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
  pub more_resources_menu: StatefulList<(String, ActiveBlock)>,
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
  pub help_docs: StatefulTable<Vec<String>>,
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
      jobs: StatefulTable::new(),
      daemon_sets: StatefulTable::new(),
      cronjobs: StatefulTable::new(),
      secrets: StatefulTable::new(),
      rpl_ctrls: StatefulTable::new(),
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
        TabRoute {
          title: format!(
            "Active Context {}",
            DEFAULT_KEYBINDING.jump_to_current_context.key
          ),
          route: Route {
            active_block: ActiveBlock::Pods,
            id: RouteId::Home,
          },
        },
        TabRoute {
          title: format!(
            "All Contexts {}",
            DEFAULT_KEYBINDING.jump_to_all_context.key
          ),
          route: Route {
            active_block: ActiveBlock::Contexts,
            id: RouteId::Contexts,
          },
        },
        TabRoute {
          title: format!("Utilization {}", DEFAULT_KEYBINDING.jump_to_utilization.key),
          route: Route {
            active_block: ActiveBlock::Utilization,
            id: RouteId::Utilization,
          },
        },
      ]),
      context_tabs: TabsState::new(vec![
        TabRoute {
          title: format!("Pods {}", DEFAULT_KEYBINDING.jump_to_pods.key),
          route: Route {
            active_block: ActiveBlock::Pods,
            id: RouteId::Home,
          },
        },
        TabRoute {
          title: format!("Services {}", DEFAULT_KEYBINDING.jump_to_services.key),
          route: Route {
            active_block: ActiveBlock::Services,
            id: RouteId::Home,
          },
        },
        TabRoute {
          title: format!("Nodes {}", DEFAULT_KEYBINDING.jump_to_nodes.key),
          route: Route {
            active_block: ActiveBlock::Nodes,
            id: RouteId::Home,
          },
        },
        TabRoute {
          title: format!("ConfigMaps {}", DEFAULT_KEYBINDING.jump_to_configmaps.key),
          route: Route {
            active_block: ActiveBlock::ConfigMaps,
            id: RouteId::Home,
          },
        },
        TabRoute {
          title: format!(
            "StatefulSets {}",
            DEFAULT_KEYBINDING.jump_to_statefulsets.key
          ),
          route: Route {
            active_block: ActiveBlock::StatefulSets,
            id: RouteId::Home,
          },
        },
        TabRoute {
          title: format!("ReplicaSets {}", DEFAULT_KEYBINDING.jump_to_replicasets.key),
          route: Route {
            active_block: ActiveBlock::ReplicaSets,
            id: RouteId::Home,
          },
        },
        TabRoute {
          title: format!("Deployments {}", DEFAULT_KEYBINDING.jump_to_deployments.key),
          route: Route {
            active_block: ActiveBlock::Deployments,
            id: RouteId::Home,
          },
        },
        TabRoute {
          title: format!("Jobs {}", DEFAULT_KEYBINDING.jump_to_jobs.key),
          route: Route {
            active_block: ActiveBlock::Jobs,
            id: RouteId::Home,
          },
        },
        TabRoute {
          title: format!("DaemonSets {}", DEFAULT_KEYBINDING.jump_to_daemonsets.key),
          route: Route {
            active_block: ActiveBlock::DaemonSets,
            id: RouteId::Home,
          },
        },
        TabRoute {
          title: format!("More {}", DEFAULT_KEYBINDING.jump_to_more_resources.key),
          route: Route {
            active_block: ActiveBlock::More,
            id: RouteId::Home,
          },
        },
      ]),
      more_resources_menu: StatefulList::with_items(vec![
        ("CronJobs".into(), ActiveBlock::CronJobs),
        ("Secrets".into(), ActiveBlock::Secrets),
        ("Replication Controllers".into(), ActiveBlock::RplCtrl),
        // ("Persistent Volume Claims".into(), ActiveBlock::RplCtrl),
        // ("Persistent Volumes".into(), ActiveBlock::RplCtrl),
        // ("Storage Classes".into(), ActiveBlock::RplCtrl),
        // ("Roles".into(), ActiveBlock::RplCtrl),
        // ("Role Bindings".into(), ActiveBlock::RplCtrl),
        // ("Cluster Roles".into(), ActiveBlock::RplCtrl),
        // ("Cluster Role Bindings".into(), ActiveBlock::RplCtrl),
        // ("Service Accounts".into(), ActiveBlock::RplCtrl),
        // ("Ingresses".into(), ActiveBlock::RplCtrl),
        // ("Network Policies".into(), ActiveBlock::RplCtrl),
      ]),
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
        GroupBy::resource,
        GroupBy::node,
        GroupBy::namespace,
        GroupBy::pod,
      ],
      help_docs: StatefulTable::with_items(key_binding::get_help_docs()),
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
    self.tick_count = 0;
    self.api_error = String::new();
    self.data = Data::default();
    self.route_home();
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
    self.api_error = e.to_string();
  }

  pub fn push_navigation_stack(&mut self, id: RouteId, active_block: ActiveBlock) {
    self.push_navigation_route(Route { id, active_block });
  }

  pub fn push_navigation_route(&mut self, route: Route) {
    self.navigation_stack.push(route);
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
    self.get_nth_route_from_last(1)
  }

  pub fn get_nth_route_from_last(&self, index: usize) -> &Route {
    // get the previous route by index
    let index = self.navigation_stack.len().saturating_sub(index + 1);
    if index > 0 {
      &self.navigation_stack[index]
    } else {
      &self.navigation_stack[0]
    }
  }

  pub fn cycle_main_routes(&mut self) {
    self.main_tabs.next();
    let route = self.main_tabs.get_active_route().clone();
    self.push_navigation_route(route);
  }

  pub fn route_home(&mut self) {
    let route = self.main_tabs.set_index(0).route.clone();
    self.push_navigation_route(route);
  }

  pub fn route_contexts(&mut self) {
    let route = self.main_tabs.set_index(1).route.clone();
    self.push_navigation_route(route);
  }

  pub fn route_utilization(&mut self) {
    let route = self.main_tabs.set_index(2).route.clone();
    self.push_navigation_route(route);
  }

  pub async fn dispatch_container_logs(&mut self, id: String) {
    self.data.logs = LogsState::new(id);
    self.push_navigation_stack(RouteId::Home, ActiveBlock::Logs);
    self.dispatch_stream(IoStreamEvent::GetPodLogs(true)).await;
  }

  pub fn refresh(&mut self) {
    self.refresh = true;
  }

  pub async fn cache_all_resource_data(&mut self) {
    self.dispatch(IoEvent::GetNamespaces).await;
    self.dispatch(IoEvent::GetPods).await;
    self.dispatch(IoEvent::GetServices).await;
    self.dispatch(IoEvent::GetConfigMaps).await;
    self.dispatch(IoEvent::GetStatefulSets).await;
    self.dispatch(IoEvent::GetReplicaSets).await;
    self.dispatch(IoEvent::GetDeployments).await;
    self.dispatch(IoEvent::GetJobs).await;
    self.dispatch(IoEvent::GetDaemonSets).await;
    self.dispatch(IoEvent::GetCronJobs).await;
    self.dispatch(IoEvent::GetSecrets).await;
    self.dispatch(IoEvent::GetReplicationControllers).await;
    self.dispatch(IoEvent::GetMetrics).await;
  }

  pub async fn dispatch_by_active_block(&mut self, active_block: ActiveBlock) {
    match active_block {
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
      ActiveBlock::Jobs => {
        self.dispatch(IoEvent::GetJobs).await;
      }
      ActiveBlock::DaemonSets => {
        self.dispatch(IoEvent::GetDaemonSets).await;
      }
      ActiveBlock::CronJobs => {
        self.dispatch(IoEvent::GetCronJobs).await;
      }
      ActiveBlock::Secrets => {
        self.dispatch(IoEvent::GetSecrets).await;
      }
      ActiveBlock::RplCtrl => {
        self.dispatch(IoEvent::GetReplicationControllers).await;
      }
      ActiveBlock::Logs => {
        if !self.is_streaming {
          // do not tail to avoid duplicates
          self.dispatch_stream(IoStreamEvent::GetPodLogs(false)).await;
        }
      }
      _ => {}
    }
  }

  pub async fn on_tick(&mut self, first_render: bool) {
    // Make one time requests on first render or refresh
    if self.refresh {
      if !first_render {
        self.dispatch(IoEvent::RefreshClient).await;
        self.dispatch_stream(IoStreamEvent::RefreshClient).await;
      }
      self.dispatch(IoEvent::GetKubeConfig).await;
      // call these once to pre-load data
      self.cache_all_resource_data().await;
      self.refresh = false;
    }
    // make network requests only in intervals to avoid hogging up the network
    if self.tick_count % self.tick_until_poll == 0 || self.is_routing {
      // make periodic network calls based on active route and active block to avoid hogging
      match self.get_current_route().id {
        RouteId::Home => {
          if self.data.clis.is_empty() {
            self.dispatch_cmd(IoCmdEvent::GetCliInfo).await;
          }
          self.dispatch(IoEvent::GetNamespaces).await;
          self.dispatch(IoEvent::GetNodes).await;

          let active_block = self.get_current_route().active_block;
          if active_block == ActiveBlock::Namespaces {
            self
              .dispatch_by_active_block(self.get_prev_route().active_block)
              .await;
          } else {
            self.dispatch_by_active_block(active_block).await;
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
  }
}

/// utility methods for tests
#[cfg(test)]
#[macro_use]
mod test_utils {
  use std::{fmt, fs};

  use k8s_openapi::{
    apimachinery::pkg::apis::meta::v1::Time,
    chrono::{DateTime, TimeZone, Utc},
  };
  use kube::{api::ObjectList, Resource};
  use serde::{de::DeserializeOwned, Serialize};

  use super::models::KubeResource;

  pub fn convert_resource_from_file<K: Serialize, T>(filename: &str) -> (Vec<T>, Vec<K>)
  where
    <K as Resource>::DynamicType: Default,
    K: Clone + DeserializeOwned + fmt::Debug,
    K: Resource,
    T: KubeResource<K>,
  {
    let res_list = load_resource_from_file(filename);
    let original_res_list = res_list.items.clone();

    let resources: Vec<T> = res_list
      .iter()
      .map(|it| T::from_api(it))
      .collect::<Vec<_>>();

    (resources, original_res_list)
  }

  pub fn load_resource_from_file<K>(filename: &str) -> ObjectList<K>
  where
    <K as Resource>::DynamicType: Default,
    K: Clone + DeserializeOwned + fmt::Debug,
    K: Resource,
  {
    let yaml = fs::read_to_string(format!("./test_data/{}.yaml", filename))
      .expect("Something went wrong reading yaml file");
    assert_ne!(yaml, "".to_string());

    let res_list: serde_yaml::Result<ObjectList<K>> = serde_yaml::from_str(&*yaml);
    assert!(res_list.is_ok(), "{:?}", res_list.err());
    res_list.unwrap()
  }

  pub fn get_time(s: &str) -> Time {
    Time(to_utc(s))
  }

  fn to_utc(s: &str) -> DateTime<Utc> {
    Utc.datetime_from_str(s, "%Y-%m-%dT%H:%M:%SZ").unwrap()
  }

  #[macro_export]
  macro_rules! map_string_object {
    // map-like
    ($($k:expr => $v:expr),* $(,)?) => {
        std::iter::Iterator::collect(std::array::IntoIter::new([$(($k.to_string(), $v),)*]))
    };
  }
}

#[cfg(test)]
mod tests {
  use tokio::sync::mpsc;

  use super::*;

  #[tokio::test]
  async fn test_on_tick_first_render() {
    let (sync_io_tx, mut sync_io_rx) = mpsc::channel::<IoEvent>(500);
    let (sync_io_cmd_tx, mut sync_io_cmd_rx) = mpsc::channel::<IoCmdEvent>(500);

    let mut app = App {
      tick_until_poll: 2,
      io_tx: Some(sync_io_tx),
      io_cmd_tx: Some(sync_io_cmd_tx),
      ..App::default()
    };

    assert_eq!(app.tick_count, 0);
    // test first render
    app.on_tick(true).await;
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetKubeConfig);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNamespaces);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetPods);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetServices);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetConfigMaps);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetStatefulSets);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetReplicaSets);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetDeployments);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetJobs);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetDaemonSets);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetCronJobs);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetSecrets);
    assert_eq!(
      sync_io_rx.recv().await.unwrap(),
      IoEvent::GetReplicationControllers
    );
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetMetrics);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNamespaces);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNodes);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetPods);

    assert_eq!(sync_io_cmd_rx.recv().await.unwrap(), IoCmdEvent::GetCliInfo);

    assert!(!app.refresh);
    assert!(!app.is_routing);
    assert_eq!(app.tick_count, 1);
  }
  #[tokio::test]
  async fn test_on_tick_refresh_tick_limit() {
    let (sync_io_tx, mut sync_io_rx) = mpsc::channel::<IoEvent>(500);
    let (sync_io_stream_tx, mut sync_io_stream_rx) = mpsc::channel::<IoStreamEvent>(500);
    let (sync_io_cmd_tx, mut sync_io_cmd_rx) = mpsc::channel::<IoCmdEvent>(500);

    let mut app = App {
      tick_until_poll: 2,
      tick_count: 2,
      refresh: true,
      io_tx: Some(sync_io_tx),
      io_stream_tx: Some(sync_io_stream_tx),
      io_cmd_tx: Some(sync_io_cmd_tx),
      ..App::default()
    };

    assert_eq!(app.tick_count, 2);
    // test first render
    app.on_tick(false).await;
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::RefreshClient);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetKubeConfig);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNamespaces);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetPods);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetServices);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetConfigMaps);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetStatefulSets);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetReplicaSets);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetDeployments);

    assert_eq!(
      sync_io_stream_rx.recv().await.unwrap(),
      IoStreamEvent::RefreshClient
    );
    assert_eq!(sync_io_cmd_rx.recv().await.unwrap(), IoCmdEvent::GetCliInfo);

    assert!(!app.refresh);
    assert!(!app.is_routing);
    assert_eq!(app.tick_count, 3);
  }
  #[tokio::test]
  async fn test_on_tick_routing() {
    let (sync_io_tx, mut sync_io_rx) = mpsc::channel::<IoEvent>(500);
    let (sync_io_stream_tx, mut sync_io_stream_rx) = mpsc::channel::<IoStreamEvent>(500);

    let mut app = App {
      tick_until_poll: 2,
      tick_count: 2,
      is_routing: true,
      refresh: false,
      io_tx: Some(sync_io_tx),
      io_stream_tx: Some(sync_io_stream_tx),
      ..App::default()
    };

    app.push_navigation_stack(RouteId::Home, ActiveBlock::Logs);

    assert_eq!(app.tick_count, 2);
    // test first render
    app.on_tick(false).await;
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNamespaces);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNodes);

    assert_eq!(
      sync_io_stream_rx.recv().await.unwrap(),
      IoStreamEvent::GetPodLogs(false)
    );

    assert!(!app.refresh);
    assert!(!app.is_routing);
    assert_eq!(app.tick_count, 3);
  }
}
