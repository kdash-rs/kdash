pub(crate) mod configmaps;
pub(crate) mod contexts;
pub(crate) mod cronjobs;
pub(crate) mod daemonsets;
pub(crate) mod deployments;
pub(crate) mod dynamic;
pub(crate) mod ingress;
pub(crate) mod jobs;
pub(crate) mod key_binding;
pub(crate) mod metrics;
pub(crate) mod models;
pub(crate) mod network_policies;
pub(crate) mod nodes;
pub(crate) mod ns;
pub(crate) mod pods;
pub(crate) mod pvcs;
pub(crate) mod pvs;
pub(crate) mod replicasets;
pub(crate) mod replication_controllers;
pub(crate) mod roles;
pub(crate) mod secrets;
pub(crate) mod serviceaccounts;
pub(crate) mod statefulsets;
pub(crate) mod storageclass;
pub(crate) mod svcs;
pub(crate) mod utils;

use anyhow::anyhow;
use kube::config::Kubeconfig;
use kubectl_view_allocations::{GroupBy, QtyByQualifier};
use log::{error, info};
use ratatui::layout::Rect;
use tokio::sync::{mpsc::Sender, watch};
use tui_input::Input;

use self::{
  configmaps::KubeConfigMap,
  contexts::KubeContext,
  cronjobs::KubeCronJob,
  daemonsets::KubeDaemonSet,
  deployments::KubeDeployment,
  dynamic::{KubeDynamicKind, KubeDynamicResource},
  ingress::KubeIngress,
  jobs::KubeJob,
  key_binding::DEFAULT_KEYBINDING,
  metrics::KubeNodeMetrics,
  models::{LogsState, ScrollableTxt, StatefulList, StatefulTable, TabRoute, TabsState},
  network_policies::KubeNetworkPolicy,
  nodes::KubeNode,
  ns::KubeNs,
  pods::{KubeContainer, KubePod},
  pvcs::KubePVC,
  pvs::KubePV,
  replicasets::KubeReplicaSet,
  replication_controllers::KubeReplicationController,
  roles::{KubeClusterRole, KubeClusterRoleBinding, KubeRole, KubeRoleBinding},
  secrets::KubeSecret,
  serviceaccounts::KubeSvcAcct,
  statefulsets::KubeStatefulSet,
  storageclass::KubeStorageClass,
  svcs::KubeSvc,
};
use super::{
  cmd::IoCmdEvent,
  network::{stream::IoStreamEvent, IoEvent},
};

const MAX_NAV_STACK: usize = 128;

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
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
  DynamicResource,
  Secrets,
  ReplicationControllers,
  StorageClasses,
  Roles,
  RoleBindings,
  ClusterRoles,
  ClusterRoleBindings,
  Ingresses,
  PersistentVolumeClaims,
  PersistentVolumes,
  NetworkPolicies,
  ServiceAccounts,
  More,
  DynamicView,
}

#[derive(Clone, Eq, PartialEq, Debug)]
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
  pub node_metrics: Vec<KubeNodeMetrics>,
  pub logs: LogsState,
  pub describe_out: ScrollableTxt,
  pub metrics: StatefulTable<(Vec<String>, Option<QtyByQualifier>)>,
  pub namespaces: StatefulTable<KubeNs>,
  pub nodes: StatefulTable<KubeNode>,
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
  pub replication_controllers: StatefulTable<KubeReplicationController>,
  pub storage_classes: StatefulTable<KubeStorageClass>,
  pub roles: StatefulTable<KubeRole>,
  pub role_bindings: StatefulTable<KubeRoleBinding>,
  pub cluster_roles: StatefulTable<KubeClusterRole>,
  pub cluster_role_bindings: StatefulTable<KubeClusterRoleBinding>,
  pub ingress: StatefulTable<KubeIngress>,
  pub persistent_volume_claims: StatefulTable<KubePVC>,
  pub persistent_volumes: StatefulTable<KubePV>,
  pub network_policies: StatefulTable<KubeNetworkPolicy>,
  pub service_accounts: StatefulTable<KubeSvcAcct>,
  pub dynamic_kinds: Vec<KubeDynamicKind>,
  pub dynamic_resources: StatefulTable<KubeDynamicResource>,
}

/// selected data items
pub struct Selected {
  pub ns: Option<String>,
  pub filter: Option<String>,
  pub pod: Option<String>,
  pub container: Option<String>,
  pub context: Option<String>,
  pub dynamic_kind: Option<KubeDynamicKind>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum InputMode {
  Normal,
  Editing,
}
pub struct AppInput {
  /// Current value of the input box
  pub input: Input,
  /// Current input mode
  pub input_mode: InputMode,
}

/// Holds main application state
pub struct App {
  navigation_stack: Vec<Route>,
  io_tx: Option<Sender<IoEvent>>,
  io_stream_tx: Option<Sender<IoStreamEvent>>,
  io_cmd_tx: Option<Sender<IoCmdEvent>>,
  log_cancel_tx: watch::Sender<bool>,
  loading_counter: u32,
  pub title: &'static str,
  pub should_quit: bool,
  pub main_tabs: TabsState,
  pub context_tabs: TabsState,
  pub more_resources_menu: StatefulList<(String, ActiveBlock)>,
  pub dynamic_resources_menu: StatefulList<(String, ActiveBlock)>,
  pub show_info_bar: bool,
  pub show_filter_bar: bool,
  pub is_streaming: bool,
  pub is_routing: bool,
  pub tick_until_poll: u64,
  pub tick_count: u64,
  pub enhanced_graphics: bool,
  pub size: Rect,
  pub api_error: String,
  pub app_input: AppInput,
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
      node_metrics: vec![],
      namespaces: StatefulTable::new(),
      selected: Selected {
        filter: None,
        ns: None,
        pod: None,
        container: None,
        context: None,
        dynamic_kind: None,
      },
      logs: LogsState::new(String::default()),
      describe_out: ScrollableTxt::new(),
      metrics: StatefulTable::new(),
      nodes: StatefulTable::new(),
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
      replication_controllers: StatefulTable::new(),
      storage_classes: StatefulTable::new(),
      roles: StatefulTable::new(),
      role_bindings: StatefulTable::new(),
      cluster_roles: StatefulTable::new(),
      cluster_role_bindings: StatefulTable::new(),
      ingress: StatefulTable::new(),
      persistent_volume_claims: StatefulTable::new(),
      persistent_volumes: StatefulTable::new(),
      network_policies: StatefulTable::new(),
      service_accounts: StatefulTable::new(),
      dynamic_kinds: vec![],
      dynamic_resources: StatefulTable::new(),
    }
  }
}

impl Default for App {
  fn default() -> Self {
    let (log_cancel_tx, _) = watch::channel(false);
    App {
      navigation_stack: vec![DEFAULT_ROUTE],
      io_tx: None,
      io_stream_tx: None,
      io_cmd_tx: None,
      log_cancel_tx,
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
        TabRoute {
          title: format!(
            "Dynamic {}",
            DEFAULT_KEYBINDING.jump_to_dynamic_resources.key
          ),
          route: Route {
            active_block: ActiveBlock::DynamicView,
            id: RouteId::Home,
          },
        },
      ]),
      more_resources_menu: StatefulList::with_items(vec![
        ("CronJobs".into(), ActiveBlock::CronJobs),
        ("Secrets".into(), ActiveBlock::Secrets),
        (
          "ReplicationControllers".into(),
          ActiveBlock::ReplicationControllers,
        ),
        (
          "PersistentVolumeClaims".into(),
          ActiveBlock::PersistentVolumeClaims,
        ),
        ("PersistentVolumes".into(), ActiveBlock::PersistentVolumes),
        ("StorageClasses".into(), ActiveBlock::StorageClasses),
        ("Roles".into(), ActiveBlock::Roles),
        ("RoleBindings".into(), ActiveBlock::RoleBindings),
        ("ClusterRoles".into(), ActiveBlock::ClusterRoles),
        (
          "ClusterRoleBinding".into(),
          ActiveBlock::ClusterRoleBindings,
        ),
        ("ServiceAccounts".into(), ActiveBlock::ServiceAccounts),
        ("Ingresses".into(), ActiveBlock::Ingresses),
        ("NetworkPolicies".into(), ActiveBlock::NetworkPolicies),
      ]),
      dynamic_resources_menu: StatefulList::new(),
      show_info_bar: true,
      show_filter_bar: false,
      loading_counter: 0,
      is_streaming: false,
      is_routing: false,
      tick_until_poll: 0,
      tick_count: 0,
      enhanced_graphics: false,
      //   table_cols: 0,
      //   dialog: None,
      //   confirm: false,
      size: Rect::default(),
      api_error: String::new(),
      app_input: AppInput {
        input: Input::default(),
        input_mode: InputMode::Normal,
      },
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

  pub fn is_loading(&self) -> bool {
    self.loading_counter > 0
  }

  pub fn loading_complete(&mut self) {
    self.loading_counter = self.loading_counter.saturating_sub(1);
  }

  /// Signal any active log stream to stop
  pub fn cancel_log_stream(&self) {
    let _ = self.log_cancel_tx.send(true);
  }

  /// Get a new receiver for log cancellation.
  /// Resets the channel so the next stream starts clean.
  pub fn new_log_cancel_rx(&self) -> watch::Receiver<bool> {
    let _ = self.log_cancel_tx.send(false);
    self.log_cancel_tx.subscribe()
  }

  pub fn reset(&mut self) {
    self.cancel_log_stream();
    self.loading_counter = 0;
    self.tick_count = 0;
    self.api_error = String::new();
    self.data = Data::default();
    self.route_home();
  }

  // Send a network event to the network thread
  pub async fn dispatch(&mut self, action: IoEvent) {
    // `loading_counter` will be decremented after the async action has finished in network/mod.rs
    if let Some(io_tx) = &self.io_tx {
      self.loading_counter += 1;
      if let Err(e) = io_tx.send(action).await {
        self.loading_counter = self.loading_counter.saturating_sub(1);
        self.handle_error(anyhow!(e));
      };
    }
  }

  // Send a stream event to the stream network thread
  pub async fn dispatch_stream(&mut self, action: IoStreamEvent) {
    // `loading_counter` will be decremented after the async action has finished in network/stream.rs
    if let Some(io_stream_tx) = &self.io_stream_tx {
      self.loading_counter += 1;
      if let Err(e) = io_stream_tx.send(action).await {
        self.loading_counter = self.loading_counter.saturating_sub(1);
        self.handle_error(anyhow!(e));
      };
    }
  }

  // Send a cmd event to the cmd runner thread
  pub async fn dispatch_cmd(&mut self, action: IoCmdEvent) {
    // `loading_counter` will be decremented after the async action has finished in cmd/mod.rs
    if let Some(io_cmd_tx) = &self.io_cmd_tx {
      self.loading_counter += 1;
      if let Err(e) = io_cmd_tx.send(action).await {
        self.loading_counter = self.loading_counter.saturating_sub(1);
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
    // Log the full debug output for diagnostics
    error!("{:?}", e);
    // Show a cleaned-up message in the UI
    self.api_error = crate::app::utils::sanitize_error_message(&e);
  }

  pub fn push_navigation_stack(&mut self, id: RouteId, active_block: ActiveBlock) {
    self.push_navigation_route(Route { id, active_block });
  }

  pub fn push_navigation_route(&mut self, route: Route) {
    self.navigation_stack.push(route);
    if self.navigation_stack.len() > MAX_NAV_STACK {
      self
        .navigation_stack
        .drain(..self.navigation_stack.len() - MAX_NAV_STACK);
    }
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
    self.cancel_log_stream();
    self.data.logs = LogsState::new(id);
    self.push_navigation_stack(RouteId::Home, ActiveBlock::Logs);
    self.dispatch_stream(IoStreamEvent::GetPodLogs(true)).await;
  }

  pub fn refresh(&mut self) {
    self.refresh = true;
  }

  pub async fn cache_all_resource_data(&mut self) {
    info!("Caching all resource data");
    self.dispatch(IoEvent::GetNamespaces).await;
    self.dispatch(IoEvent::GetPods).await;
    self.dispatch(IoEvent::DiscoverDynamicRes).await;
    self.dispatch(IoEvent::GetServices).await;
    self.dispatch(IoEvent::GetNodes).await;
    self.dispatch(IoEvent::GetConfigMaps).await;
    self.dispatch(IoEvent::GetStatefulSets).await;
    self.dispatch(IoEvent::GetReplicaSets).await;
    self.dispatch(IoEvent::GetDeployments).await;
    self.dispatch(IoEvent::GetJobs).await;
    self.dispatch(IoEvent::GetDaemonSets).await;
    self.dispatch(IoEvent::GetCronJobs).await;
    self.dispatch(IoEvent::GetSecrets).await;
    self.dispatch(IoEvent::GetReplicationControllers).await;
    self.dispatch(IoEvent::GetStorageClasses).await;
    self.dispatch(IoEvent::GetRoles).await;
    self.dispatch(IoEvent::GetRoleBindings).await;
    self.dispatch(IoEvent::GetClusterRoles).await;
    self.dispatch(IoEvent::GetClusterRoleBinding).await;
    self.dispatch(IoEvent::GetIngress).await;
    self.dispatch(IoEvent::GetPvcs).await;
    self.dispatch(IoEvent::GetPvs).await;
    self.dispatch(IoEvent::GetServiceAccounts).await;
    self.dispatch(IoEvent::GetNetworkPolicies).await;
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
      ActiveBlock::ReplicationControllers => {
        self.dispatch(IoEvent::GetReplicationControllers).await;
      }
      ActiveBlock::StorageClasses => {
        self.dispatch(IoEvent::GetStorageClasses).await;
      }
      ActiveBlock::Roles => {
        self.dispatch(IoEvent::GetRoles).await;
      }
      ActiveBlock::RoleBindings => {
        self.dispatch(IoEvent::GetRoleBindings).await;
      }
      ActiveBlock::ClusterRoles => {
        self.dispatch(IoEvent::GetClusterRoles).await;
      }
      ActiveBlock::ClusterRoleBindings => {
        self.dispatch(IoEvent::GetClusterRoleBinding).await;
      }
      ActiveBlock::Ingresses => {
        self.dispatch(IoEvent::GetIngress).await;
      }
      ActiveBlock::PersistentVolumeClaims => {
        self.dispatch(IoEvent::GetPvcs).await;
      }
      ActiveBlock::PersistentVolumes => {
        self.dispatch(IoEvent::GetPvs).await;
      }
      ActiveBlock::ServiceAccounts => {
        self.dispatch(IoEvent::GetServiceAccounts).await;
      }
      ActiveBlock::DynamicResource => {
        self.dispatch(IoEvent::GetDynamicRes).await;
      }
      ActiveBlock::Logs => {
        if !self.is_streaming {
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
    if self.tick_count.is_multiple_of(self.tick_until_poll) || self.is_routing {
      // Safety-net kubeconfig reload (~60s) in case the file watcher misses an event
      if self.tick_until_poll > 0
        && self.tick_count > 0
        && self.tick_count.is_multiple_of(self.tick_until_poll * 12)
      {
        self.dispatch(IoEvent::GetKubeConfig).await;
      }
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

  use chrono::{DateTime, Utc};
  use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
  use kube::{api::ObjectList, Resource};
  use serde::{de::DeserializeOwned, Serialize};

  use super::models::KubeResource;

  pub fn convert_resource_from_file<K, T>(filename: &str) -> (Vec<T>, Vec<K>)
  where
    K: Serialize + Resource + Clone + DeserializeOwned + fmt::Debug,
    T: KubeResource<K> + From<K>,
  {
    let res_list = load_resource_from_file(filename);
    let original_res_list = res_list.items.clone();

    let resources: Vec<T> = res_list.into_iter().map(K::into).collect::<Vec<_>>();

    (resources, original_res_list)
  }

  pub fn load_resource_from_file<K>(filename: &str) -> ObjectList<K>
  where
    K: Clone + DeserializeOwned + fmt::Debug,
    K: Resource,
  {
    let yaml = fs::read_to_string(format!("./test_data/{}.yaml", filename))
      .expect("Something went wrong reading yaml file");
    assert_ne!(yaml, "".to_string());

    let res_list: serde_yaml::Result<ObjectList<K>> = serde_yaml::from_str(&yaml);
    assert!(res_list.is_ok(), "{:?}", res_list.err());
    res_list.unwrap()
  }

  pub fn get_time(s: &str) -> Time {
    let dt = to_utc(s);
    Time(k8s_openapi::jiff::Timestamp::from_second(dt.timestamp()).unwrap())
  }

  fn to_utc(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_str(&format!("{} +0000", s), "%Y-%m-%dT%H:%M:%SZ %z")
      .unwrap()
      .into()
  }

  #[macro_export]
  macro_rules! map_string_object {
    // map-like
    ($($k:expr => $v:expr),* $(,)?) => {
        std::iter::Iterator::collect(IntoIterator::into_iter([$(($k.to_string(), $v),)*]))
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
    // test first render — cache_all_resource_data pre-loads everything
    app.on_tick(true).await;
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetKubeConfig);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNamespaces);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetPods);
    assert_eq!(
      sync_io_rx.recv().await.unwrap(),
      IoEvent::DiscoverDynamicRes
    );
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetServices);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNodes);
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
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetStorageClasses);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetRoles);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetRoleBindings);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetClusterRoles);
    assert_eq!(
      sync_io_rx.recv().await.unwrap(),
      IoEvent::GetClusterRoleBinding
    );
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetIngress);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetPvcs);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetPvs);
    assert_eq!(
      sync_io_rx.recv().await.unwrap(),
      IoEvent::GetServiceAccounts
    );
    assert_eq!(
      sync_io_rx.recv().await.unwrap(),
      IoEvent::GetNetworkPolicies
    );
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetMetrics);
    // periodic polling also fires (tick_count 0 % 2 == 0), fetching active tab data
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
    // test refresh (non-first-render) — cache_all_resource_data pre-loads everything
    app.on_tick(false).await;
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::RefreshClient);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetKubeConfig);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNamespaces);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetPods);
    assert_eq!(
      sync_io_rx.recv().await.unwrap(),
      IoEvent::DiscoverDynamicRes
    );
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetServices);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNodes);
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

  #[tokio::test]
  async fn test_on_tick_no_poll_non_refresh() {
    // When tick_count is not a multiple of tick_until_poll and refresh=false,
    // no IO events should be dispatched (lazy loading: only fetch when needed)
    let (sync_io_tx, mut sync_io_rx) = mpsc::channel::<IoEvent>(500);

    let mut app = App {
      tick_until_poll: 5,
      tick_count: 3, // 3 % 5 != 0, so no polling
      refresh: false,
      is_routing: false,
      io_tx: Some(sync_io_tx),
      ..App::default()
    };

    app.on_tick(false).await;

    // No IO events should have been dispatched
    assert!(sync_io_rx.try_recv().is_err());
    assert_eq!(app.tick_count, 4);
  }

  #[tokio::test]
  async fn test_on_tick_dispatch_by_active_block() {
    // Verify that on polling tick, the active block's resource is fetched
    let (sync_io_tx, mut sync_io_rx) = mpsc::channel::<IoEvent>(500);
    let (sync_io_cmd_tx, mut sync_io_cmd_rx) = mpsc::channel::<IoCmdEvent>(500);

    let mut app = App {
      tick_until_poll: 1, // poll every tick
      tick_count: 0,
      refresh: false,
      io_tx: Some(sync_io_tx),
      io_cmd_tx: Some(sync_io_cmd_tx),
      ..App::default()
    };

    // Navigate to Services tab
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Services);

    app.on_tick(false).await;

    // Should dispatch: GetNamespaces, GetNodes (always), then GetServices (active block)
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNamespaces);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNodes);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetServices);
    assert_eq!(sync_io_cmd_rx.recv().await.unwrap(), IoCmdEvent::GetCliInfo);
  }

  #[test]
  fn test_navigation_stack_cap() {
    let mut app = App::default();
    // Push more than MAX_NAV_STACK routes
    for _i in 0..150 {
      app.push_navigation_stack(RouteId::Home, ActiveBlock::Pods);
    }
    // Stack should be capped at MAX_NAV_STACK
    assert!(app.navigation_stack.len() <= MAX_NAV_STACK);
    // Current route should still be the most recently pushed
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Pods);
  }

  #[test]
  fn test_navigation_stack_within_cap() {
    let mut app = App::default();
    // Push fewer than cap - default already has 1 route
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Services);
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Pods);
    assert_eq!(app.navigation_stack.len(), 3); // 1 default + 2 pushed
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Pods);
    // Pop should work normally
    let popped = app.pop_navigation_stack();
    assert!(popped.is_some());
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Services);
  }

  #[test]
  fn test_loading_counter_default() {
    let app = App::default();
    assert!(!app.is_loading());
  }

  #[tokio::test]
  async fn test_dispatch_without_sender_does_not_set_loading() {
    let mut app = App::default();

    app.dispatch(IoEvent::GetNamespaces).await;

    assert!(!app.is_loading());
  }

  #[tokio::test]
  async fn test_dispatch_stream_without_sender_does_not_set_loading() {
    let mut app = App::default();

    app.dispatch_stream(IoStreamEvent::GetPodLogs(false)).await;

    assert!(!app.is_loading());
  }

  #[tokio::test]
  async fn test_dispatch_cmd_without_sender_does_not_set_loading() {
    let mut app = App::default();

    app.dispatch_cmd(IoCmdEvent::GetCliInfo).await;

    assert!(!app.is_loading());
  }

  #[test]
  fn test_set_contexts_tracks_active_context() {
    use crate::app::contexts::KubeContext;

    let mut app = App::default();
    let contexts = vec![
      KubeContext {
        name: "ctx-a".into(),
        namespace: Some("ns-a".into()),
        is_active: false,
        ..Default::default()
      },
      KubeContext {
        name: "ctx-b".into(),
        namespace: Some("ns-b".into()),
        is_active: true,
        ..Default::default()
      },
    ];

    app.set_contexts(contexts);

    let active = app
      .data
      .active_context
      .as_ref()
      .expect("should have active context");
    assert_eq!(active.name, "ctx-b");
    assert_eq!(active.namespace, Some("ns-b".into()));
    assert_eq!(app.data.contexts.items.len(), 2);
  }

  #[test]
  fn test_set_contexts_no_active() {
    use crate::app::contexts::KubeContext;

    let mut app = App::default();
    let contexts = vec![KubeContext {
      name: "ctx-a".into(),
      is_active: false,
      ..Default::default()
    }];

    app.set_contexts(contexts);

    assert!(app.data.active_context.is_none());
  }
}
