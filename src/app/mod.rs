pub(crate) mod configmaps;
pub(crate) mod contexts;
pub(crate) mod cronjobs;
pub(crate) mod daemonsets;
pub(crate) mod deployments;
pub(crate) mod dynamic;
pub(crate) mod events;
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
pub(crate) mod troubleshoot;
pub(crate) mod utils;

use anyhow::anyhow;
use chrono::Local;
use kube::config::Kubeconfig;
use kubectl_view_allocations::{GroupBy, QtyByQualifier};
use log::{error, info};
use ratatui::layout::Rect;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc::Sender, watch};

use self::{
  configmaps::KubeConfigMap,
  contexts::KubeContext,
  cronjobs::KubeCronJob,
  daemonsets::KubeDaemonSet,
  deployments::KubeDeployment,
  dynamic::{DynamicResourceCache, KubeDynamicKind, KubeDynamicResource},
  events::KubeEvent,
  ingress::KubeIngress,
  jobs::KubeJob,
  key_binding::DEFAULT_KEYBINDING,
  metrics::KubeNodeMetrics,
  models::{
    FilterableTable, LogsState, ScrollableTxt, StatefulList, StatefulTable, TabRoute, TabsState,
  },
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
  config::KdashConfig,
  network::{stream::IoStreamEvent, IoEvent},
};

const MAX_NAV_STACK: usize = 128;
const STATUS_MESSAGE_DURATION: Duration = Duration::from_secs(5);
pub const DEFAULT_LOG_TAIL_LINES: u32 = 100;
pub const MAX_ERROR_HISTORY: usize = 100;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorRecord {
  pub timestamp: String,
  pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingShellExec {
  pub namespace: String,
  pub pod: String,
  pub container: String,
}

#[derive(Clone, Debug)]
pub struct StatusMessage {
  pub text: String,
  pub duration: Duration,
  expires_at: Option<Instant>,
}

impl Default for StatusMessage {
  fn default() -> Self {
    Self {
      text: String::new(),
      duration: STATUS_MESSAGE_DURATION,
      expires_at: None,
    }
  }
}

impl StatusMessage {
  pub fn is_empty(&self) -> bool {
    self.text.is_empty()
  }

  pub fn text(&self) -> &str {
    &self.text
  }

  pub fn show(&mut self, message: impl Into<String>) {
    self.show_at(message, Instant::now());
  }

  pub fn show_at(&mut self, message: impl Into<String>, now: Instant) {
    self.text = message.into();
    self.expires_at = Some(now + self.duration);
  }

  pub fn clear(&mut self) {
    self.text.clear();
    self.expires_at = None;
  }

  pub fn clear_if_expired(&mut self, now: Instant) {
    if self.expires_at.is_some_and(|expires_at| now >= expires_at) {
      self.clear();
    }
  }
}

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
  Troubleshoot,
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
  Events,
  More,
  DynamicView,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum RouteId {
  Home,
  Contexts,
  Utilization,
  Troubleshoot,
  HelpMenu,
}

#[derive(Debug, Clone, Eq, PartialEq)]
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
  pub troubleshoot_findings: StatefulTable<troubleshoot::DisplayFinding>,
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
  pub events: StatefulTable<KubeEvent>,
  pub dynamic_kinds: Vec<KubeDynamicKind>,
  pub dynamic_resources: StatefulTable<KubeDynamicResource>,
  pub dynamic_resource_cache: DynamicResourceCache,
}

/// selected data items
pub struct Selected {
  pub ns: Option<String>,
  pub pod: Option<String>,
  pub container: Option<String>,
  pub context: Option<String>,
  pub dynamic_kind: Option<KubeDynamicKind>,
  /// Label selector for pod drill-down from workload resources
  pub pod_selector: Option<String>,
  /// Namespace for pod drill-down (the workload resource's namespace)
  pub pod_selector_ns: Option<String>,
  /// Parent resource name for display in drill-down title breadcrumbs
  pub pod_selector_resource: Option<String>,
}

/// Holds main application state
pub struct App {
  navigation_stack: Vec<Route>,
  io_tx: Option<Sender<IoEvent>>,
  io_stream_tx: Option<Sender<IoStreamEvent>>,
  io_cmd_tx: Option<Sender<IoCmdEvent>>,
  log_cancel_tx: watch::Sender<bool>,
  loading_counter: u32,
  background_cache_pending: bool,
  pub title: &'static str,
  pub should_quit: bool,
  pub main_tabs: TabsState,
  pub context_tabs: TabsState,
  pub more_resources_menu: StatefulList<(String, ActiveBlock)>,
  pub dynamic_resources_menu: StatefulList<(String, ActiveBlock)>,
  pub menu_filter: String,
  pub menu_filter_active: bool,
  pub ns_filter: String,
  pub ns_filter_active: bool,
  pub show_info_bar: bool,
  pub is_streaming: bool,
  pub is_routing: bool,
  pub tick_until_poll: u64,
  pub tick_count: u64,
  pub enhanced_graphics: bool,
  pub size: Rect,
  pub api_error: String,
  pub status_message: StatusMessage,
  pub light_theme: bool,
  pub refresh: bool,
  pub log_auto_scroll: bool,
  pub log_tail_lines: u32,
  pub utilization_group_by: Vec<GroupBy>,
  pub help_docs: StatefulTable<Vec<String>>,
  pub error_history: VecDeque<ErrorRecord>,
  pending_shell_exec: Option<PendingShellExec>,
  pub config: KdashConfig,
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
        ns: None,
        pod: None,
        container: None,
        context: None,
        dynamic_kind: None,
        pod_selector: None,
        pod_selector_ns: None,
        pod_selector_resource: None,
      },
      logs: LogsState::new(String::default()),
      describe_out: ScrollableTxt::new(),
      metrics: StatefulTable::new(),
      troubleshoot_findings: StatefulTable::new(),
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
      events: StatefulTable::new(),
      dynamic_kinds: vec![],
      dynamic_resources: StatefulTable::new(),
      dynamic_resource_cache: DynamicResourceCache::default(),
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
        TabRoute {
          title: format!(
            "Troubleshoot {}",
            DEFAULT_KEYBINDING.jump_to_troubleshoot.key
          ),
          route: Route {
            active_block: ActiveBlock::Troubleshoot,
            id: RouteId::Troubleshoot,
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
        ("Events".into(), ActiveBlock::Events),
        ("NetworkPolicies".into(), ActiveBlock::NetworkPolicies),
      ]),
      dynamic_resources_menu: StatefulList::new(),
      menu_filter: String::new(),
      menu_filter_active: false,
      ns_filter: String::new(),
      ns_filter_active: false,
      show_info_bar: true,
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
      status_message: StatusMessage::default(),
      light_theme: false,
      refresh: true,
      log_auto_scroll: true,
      log_tail_lines: DEFAULT_LOG_TAIL_LINES,
      utilization_group_by: Self::default_utilization_group_by(),
      help_docs: StatefulTable::with_items(key_binding::get_help_docs()),
      background_cache_pending: false,
      error_history: VecDeque::with_capacity(MAX_ERROR_HISTORY),
      pending_shell_exec: None,
      config: KdashConfig::default(),
      data: Data::default(),
    }
  }
}

impl App {
  fn default_utilization_group_by() -> Vec<GroupBy> {
    vec![
      GroupBy::resource,
      GroupBy::node,
      GroupBy::namespace,
      GroupBy::pod,
    ]
  }

  fn resource_block_for_context_tab(index: usize) -> Option<ActiveBlock> {
    match index {
      0 => Some(ActiveBlock::Pods),
      1 => Some(ActiveBlock::Services),
      2 => Some(ActiveBlock::Nodes),
      3 => Some(ActiveBlock::ConfigMaps),
      4 => Some(ActiveBlock::StatefulSets),
      5 => Some(ActiveBlock::ReplicaSets),
      6 => Some(ActiveBlock::Deployments),
      7 => Some(ActiveBlock::Jobs),
      8 => Some(ActiveBlock::DaemonSets),
      _ => None,
    }
  }

  pub fn resource_table(&self, block: ActiveBlock) -> Option<&dyn FilterableTable> {
    match block {
      ActiveBlock::Help => Some(&self.help_docs),
      ActiveBlock::Contexts => Some(&self.data.contexts),
      ActiveBlock::Utilization => Some(&self.data.metrics),
      ActiveBlock::Troubleshoot => Some(&self.data.troubleshoot_findings),
      ActiveBlock::Pods => Some(&self.data.pods),
      ActiveBlock::Services => Some(&self.data.services),
      ActiveBlock::Nodes => Some(&self.data.nodes),
      ActiveBlock::ConfigMaps => Some(&self.data.config_maps),
      ActiveBlock::StatefulSets => Some(&self.data.stateful_sets),
      ActiveBlock::ReplicaSets => Some(&self.data.replica_sets),
      ActiveBlock::Deployments => Some(&self.data.deployments),
      ActiveBlock::Jobs => Some(&self.data.jobs),
      ActiveBlock::DaemonSets => Some(&self.data.daemon_sets),
      ActiveBlock::CronJobs => Some(&self.data.cronjobs),
      ActiveBlock::Secrets => Some(&self.data.secrets),
      ActiveBlock::ReplicationControllers => Some(&self.data.replication_controllers),
      ActiveBlock::StorageClasses => Some(&self.data.storage_classes),
      ActiveBlock::Roles => Some(&self.data.roles),
      ActiveBlock::RoleBindings => Some(&self.data.role_bindings),
      ActiveBlock::ClusterRoles => Some(&self.data.cluster_roles),
      ActiveBlock::ClusterRoleBindings => Some(&self.data.cluster_role_bindings),
      ActiveBlock::Ingresses => Some(&self.data.ingress),
      ActiveBlock::PersistentVolumeClaims => Some(&self.data.persistent_volume_claims),
      ActiveBlock::PersistentVolumes => Some(&self.data.persistent_volumes),
      ActiveBlock::NetworkPolicies => Some(&self.data.network_policies),
      ActiveBlock::ServiceAccounts => Some(&self.data.service_accounts),
      ActiveBlock::DynamicResource => Some(&self.data.dynamic_resources),
      _ => None,
    }
  }

  pub fn resource_table_mut(&mut self, block: ActiveBlock) -> Option<&mut dyn FilterableTable> {
    match block {
      ActiveBlock::Help => Some(&mut self.help_docs),
      ActiveBlock::Contexts => Some(&mut self.data.contexts),
      ActiveBlock::Utilization => Some(&mut self.data.metrics),
      ActiveBlock::Troubleshoot => Some(&mut self.data.troubleshoot_findings),
      ActiveBlock::Pods => Some(&mut self.data.pods),
      ActiveBlock::Services => Some(&mut self.data.services),
      ActiveBlock::Nodes => Some(&mut self.data.nodes),
      ActiveBlock::ConfigMaps => Some(&mut self.data.config_maps),
      ActiveBlock::StatefulSets => Some(&mut self.data.stateful_sets),
      ActiveBlock::ReplicaSets => Some(&mut self.data.replica_sets),
      ActiveBlock::Deployments => Some(&mut self.data.deployments),
      ActiveBlock::Jobs => Some(&mut self.data.jobs),
      ActiveBlock::DaemonSets => Some(&mut self.data.daemon_sets),
      ActiveBlock::CronJobs => Some(&mut self.data.cronjobs),
      ActiveBlock::Secrets => Some(&mut self.data.secrets),
      ActiveBlock::ReplicationControllers => Some(&mut self.data.replication_controllers),
      ActiveBlock::StorageClasses => Some(&mut self.data.storage_classes),
      ActiveBlock::Roles => Some(&mut self.data.roles),
      ActiveBlock::RoleBindings => Some(&mut self.data.role_bindings),
      ActiveBlock::ClusterRoles => Some(&mut self.data.cluster_roles),
      ActiveBlock::ClusterRoleBindings => Some(&mut self.data.cluster_role_bindings),
      ActiveBlock::Ingresses => Some(&mut self.data.ingress),
      ActiveBlock::PersistentVolumeClaims => Some(&mut self.data.persistent_volume_claims),
      ActiveBlock::PersistentVolumes => Some(&mut self.data.persistent_volumes),
      ActiveBlock::NetworkPolicies => Some(&mut self.data.network_policies),
      ActiveBlock::ServiceAccounts => Some(&mut self.data.service_accounts),
      ActiveBlock::DynamicResource => Some(&mut self.data.dynamic_resources),
      _ => None,
    }
  }

  pub fn current_resource_table(&self) -> Option<&dyn FilterableTable> {
    self.resource_table(self.get_current_route().active_block)
  }

  pub fn current_or_selected_resource_table(&self) -> Option<&dyn FilterableTable> {
    self.current_resource_table().or_else(|| {
      Self::resource_block_for_context_tab(self.context_tabs.index)
        .and_then(|block| self.resource_table(block))
    })
  }

  pub fn context_tab_resource_table(&self, index: usize) -> Option<&dyn FilterableTable> {
    Self::resource_block_for_context_tab(index).and_then(|block| self.resource_table(block))
  }

  pub fn new(
    io_tx: Sender<IoEvent>,
    io_stream_tx: Sender<IoStreamEvent>,
    io_cmd_tx: Sender<IoCmdEvent>,
    enhanced_graphics: bool,
    tick_until_poll: u64,
    log_tail_lines: u32,
    config: KdashConfig,
  ) -> Self {
    App {
      io_tx: Some(io_tx),
      io_stream_tx: Some(io_stream_tx),
      io_cmd_tx: Some(io_cmd_tx),
      enhanced_graphics,
      tick_until_poll,
      log_tail_lines,
      config,
      ..App::default()
    }
  }

  pub fn is_menu_active(&self) -> bool {
    matches!(
      self.get_current_route().active_block,
      ActiveBlock::More | ActiveBlock::DynamicView
    )
  }

  pub fn current_resource_filter_mut(
    &mut self,
  ) -> Option<(&mut String, &mut bool, &mut ratatui::widgets::TableState)> {
    self
      .resource_table_mut(self.get_current_route().active_block)
      .map(FilterableTable::filter_parts_mut)
  }

  pub fn deactivate_current_resource_filter(&mut self) {
    if let Some((_, filter_active, _)) = self.current_resource_filter_mut() {
      *filter_active = false;
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

  pub fn initial_log_tail_lines(&self) -> i64 {
    i64::from(self.log_tail_lines)
  }

  pub fn reset(&mut self) {
    self.cancel_log_stream();
    self.loading_counter = 0;
    self.tick_count = 0;
    self.api_error = String::new();
    self.status_message.clear();
    self.utilization_group_by = Self::default_utilization_group_by();
    self.data = Data::default();
    self.route_home();
  }

  pub fn selected_dynamic_cache_key(&self) -> Option<String> {
    self
      .data
      .selected
      .dynamic_kind
      .as_ref()
      .map(|kind| dynamic::dynamic_cache_key(kind, self.data.selected.ns.as_deref()))
  }

  pub fn apply_cached_dynamic_resources(&mut self) -> bool {
    let Some(cache_key) = self.selected_dynamic_cache_key() else {
      return false;
    };

    let Some(items) = self.data.dynamic_resource_cache.get_cloned(&cache_key) else {
      return false;
    };

    self.data.dynamic_resources.set_items(items);
    true
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
    let message = crate::app::utils::sanitize_error_message(&e);
    self.record_error(message.clone());
    self.status_message.clear();
    self.api_error = message;
  }

  pub fn record_error(&mut self, message: String) {
    self.error_history.push_back(ErrorRecord {
      timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
      message,
    });

    while self.error_history.len() > MAX_ERROR_HISTORY {
      self.error_history.pop_front();
    }
  }

  pub fn set_status_message(&mut self, message: impl Into<String>) {
    self.api_error.clear();
    self.status_message.show(message);
  }

  pub fn queue_shell_exec(&mut self, request: PendingShellExec) {
    self.pending_shell_exec = Some(request);
  }

  pub fn take_pending_shell_exec(&mut self) -> Option<PendingShellExec> {
    self.pending_shell_exec.take()
  }

  #[cfg(test)]
  pub fn pending_shell_exec(&self) -> Option<&PendingShellExec> {
    self.pending_shell_exec.as_ref()
  }

  pub fn clear_status_message(&mut self) {
    self.status_message.clear();
  }

  fn clear_expired_status_message(&mut self, now: Instant) {
    self.status_message.clear_if_expired(now);
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

  pub fn route_troubleshoot(&mut self) {
    let route = self.main_tabs.set_index(3).route.clone();
    self.push_navigation_route(route);
  }

  /// Navigate from a node to its pods via field selector.
  pub async fn dispatch_node_pods(&mut self, node_name: String, route_id: RouteId) {
    self.data.selected.pod_selector = Some(node_name.clone());
    self.data.selected.pod_selector_ns = None;
    self.data.selected.pod_selector_resource = Some("node".into());
    self.dispatch(IoEvent::GetPodsByNode { node_name }).await;
    self.push_navigation_stack(route_id, ActiveBlock::Pods);
  }

  /// Navigate from a workload resource to its owned pods via label selector drill-down.
  pub async fn dispatch_resource_pods(
    &mut self,
    namespace: String,
    selector: String,
    resource_name: String,
    route_id: RouteId,
  ) {
    self.data.selected.pod_selector = Some(selector.clone());
    self.data.selected.pod_selector_ns = Some(namespace.clone());
    self.data.selected.pod_selector_resource = Some(resource_name);
    self
      .dispatch(IoEvent::GetPodsBySelector {
        namespace,
        selector,
      })
      .await;
    self.push_navigation_stack(route_id, ActiveBlock::Pods);
  }

  pub async fn dispatch_pod_logs(&mut self, pod_name: String, route_id: RouteId) {
    self.cancel_log_stream();
    self.data.logs = LogsState::new(format!("agg:{}", pod_name));
    self.push_navigation_stack(route_id, ActiveBlock::Logs);
    self
      .dispatch_stream(IoStreamEvent::GetPodAllContainerLogs)
      .await;
  }

  pub async fn dispatch_container_logs(&mut self, id: String, route_id: RouteId) {
    self.cancel_log_stream();
    self.data.logs = LogsState::new(id);
    self.push_navigation_stack(route_id, ActiveBlock::Logs);
    self.dispatch_stream(IoStreamEvent::GetPodLogs(true)).await;
  }

  /// Start aggregate log streaming from all pods matching a label selector.
  pub async fn dispatch_aggregate_logs(
    &mut self,
    name: String,
    namespace: String,
    selector: String,
    resource_name: String,
    route_id: RouteId,
  ) {
    self.cancel_log_stream();
    self.data.selected.pod_selector_resource = Some(resource_name);
    self.data.logs = LogsState::new(format!("agg:{}", name));
    self.push_navigation_stack(route_id, ActiveBlock::Logs);
    self
      .dispatch_stream(IoStreamEvent::GetAggregateLogs {
        namespace,
        selector,
      })
      .await;
  }

  pub fn refresh(&mut self) {
    self.refresh = true;
  }

  pub fn restore_route_state(
    &mut self,
    main_tab_index: usize,
    context_tab_index: usize,
    route: Route,
  ) {
    self.main_tabs.set_index(main_tab_index);
    self.context_tabs.set_index(context_tab_index);
    self.navigation_stack = vec![route];
    self.is_routing = true;
  }

  pub fn refresh_restore_route(&self) -> Route {
    match self.main_tabs.index {
      0 => self.context_tabs.get_active_route().clone(),
      _ => self.main_tabs.get_active_route().clone(),
    }
  }

  pub fn queue_background_resource_cache(&mut self) {
    self.background_cache_pending = true;
  }

  fn background_home_resource_events() -> &'static [IoEvent] {
    &[
      IoEvent::GetPods,
      IoEvent::GetServices,
      IoEvent::GetConfigMaps,
      IoEvent::GetStatefulSets,
      IoEvent::GetReplicaSets,
      IoEvent::GetDeployments,
      IoEvent::GetJobs,
      IoEvent::GetDaemonSets,
      IoEvent::GetCronJobs,
      IoEvent::GetSecrets,
      IoEvent::GetReplicationControllers,
      IoEvent::GetStorageClasses,
      IoEvent::GetRoles,
      IoEvent::GetRoleBindings,
      IoEvent::GetClusterRoles,
      IoEvent::GetClusterRoleBinding,
      IoEvent::GetIngress,
      IoEvent::GetPvcs,
      IoEvent::GetPvs,
      IoEvent::GetServiceAccounts,
      IoEvent::GetEvents,
      IoEvent::GetNetworkPolicies,
    ]
  }

  fn active_home_cache_block(&self) -> ActiveBlock {
    match self.get_current_route().active_block {
      ActiveBlock::Namespaces | ActiveBlock::Describe | ActiveBlock::Yaml | ActiveBlock::Logs => {
        self.get_prev_route().active_block
      }
      active_block => active_block,
    }
  }

  fn background_home_event_to_skip(&self) -> Option<IoEvent> {
    match self.active_home_cache_block() {
      ActiveBlock::Pods | ActiveBlock::Containers => Some(IoEvent::GetPods),
      ActiveBlock::Services => Some(IoEvent::GetServices),
      ActiveBlock::ConfigMaps => Some(IoEvent::GetConfigMaps),
      ActiveBlock::StatefulSets => Some(IoEvent::GetStatefulSets),
      ActiveBlock::ReplicaSets => Some(IoEvent::GetReplicaSets),
      ActiveBlock::Deployments => Some(IoEvent::GetDeployments),
      ActiveBlock::Jobs => Some(IoEvent::GetJobs),
      ActiveBlock::DaemonSets => Some(IoEvent::GetDaemonSets),
      ActiveBlock::CronJobs => Some(IoEvent::GetCronJobs),
      ActiveBlock::Secrets => Some(IoEvent::GetSecrets),
      ActiveBlock::ReplicationControllers => Some(IoEvent::GetReplicationControllers),
      ActiveBlock::StorageClasses => Some(IoEvent::GetStorageClasses),
      ActiveBlock::Roles => Some(IoEvent::GetRoles),
      ActiveBlock::RoleBindings => Some(IoEvent::GetRoleBindings),
      ActiveBlock::ClusterRoles => Some(IoEvent::GetClusterRoles),
      ActiveBlock::ClusterRoleBindings => Some(IoEvent::GetClusterRoleBinding),
      ActiveBlock::Ingresses => Some(IoEvent::GetIngress),
      ActiveBlock::PersistentVolumeClaims => Some(IoEvent::GetPvcs),
      ActiveBlock::PersistentVolumes => Some(IoEvent::GetPvs),
      ActiveBlock::ServiceAccounts => Some(IoEvent::GetServiceAccounts),
      ActiveBlock::Events => Some(IoEvent::GetEvents),
      ActiveBlock::NetworkPolicies => Some(IoEvent::GetNetworkPolicies),
      _ => None,
    }
  }

  pub async fn cache_essential_data(&mut self) {
    info!("Caching essential resource data");
    self.dispatch(IoEvent::GetNamespaces).await;
    self.dispatch(IoEvent::GetNodes).await;

    match self.get_current_route().id {
      RouteId::Home => {
        self
          .dispatch_by_active_block(self.active_home_cache_block())
          .await;
      }
      RouteId::Utilization => {
        self.dispatch(IoEvent::GetMetrics).await;
      }
      RouteId::Troubleshoot => {
        if self.get_current_route().active_block == ActiveBlock::Troubleshoot {
          self.dispatch(IoEvent::GetTroubleshootFindings).await;
        }
      }
      _ => {}
    }
  }

  pub async fn cache_background_resource_data(&mut self) {
    info!("Caching background resource data");
    self.dispatch(IoEvent::DiscoverDynamicRes).await;

    let skip_home_event = if self.get_current_route().id == RouteId::Home {
      self.background_home_event_to_skip()
    } else {
      None
    };

    for event in Self::background_home_resource_events() {
      if skip_home_event.as_ref() == Some(event) {
        continue;
      }
      self.dispatch(event.clone()).await;
    }

    if self.get_current_route().id != RouteId::Utilization {
      self.dispatch(IoEvent::GetMetrics).await;
    }
  }

  pub async fn dispatch_by_active_block(&mut self, active_block: ActiveBlock) {
    match active_block {
      ActiveBlock::Pods | ActiveBlock::Containers => {
        // If we're in a workload drill-down, refresh using the label selector
        if let (Some(selector), Some(namespace)) = (
          self.data.selected.pod_selector.clone(),
          self.data.selected.pod_selector_ns.clone(),
        ) {
          self
            .dispatch(IoEvent::GetPodsBySelector {
              namespace,
              selector,
            })
            .await;
        } else {
          self.dispatch(IoEvent::GetPods).await;
        }
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
      ActiveBlock::Events => {
        self.dispatch(IoEvent::GetEvents).await;
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
    self.clear_expired_status_message(Instant::now());

    // Make one time requests on first render or refresh
    let mut did_refresh = false;
    if self.refresh {
      if !first_render {
        self.dispatch(IoEvent::RefreshClient).await;
        self.dispatch_stream(IoStreamEvent::RefreshClient).await;
      }
      self.dispatch(IoEvent::GetKubeConfig).await;
      self.cache_essential_data().await;
      self.queue_background_resource_cache();
      self.refresh = false;
      did_refresh = true;
    }

    if self.background_cache_pending && !first_render && !did_refresh {
      self.cache_background_resource_data().await;
      self.background_cache_pending = false;
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
        RouteId::Troubleshoot => {
          if self.get_current_route().active_block == ActiveBlock::Troubleshoot {
            self.dispatch(IoEvent::GetTroubleshootFindings).await;
          }
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
  use anyhow::anyhow;
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
    // test first render — essential data loads immediately, background cache is deferred
    app.on_tick(true).await;
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetKubeConfig);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNamespaces);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNodes);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetPods);
    // periodic polling also fires (tick_count 0 % 2 == 0), fetching active tab data
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNamespaces);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNodes);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetPods);

    assert_eq!(sync_io_cmd_rx.recv().await.unwrap(), IoCmdEvent::GetCliInfo);

    assert!(!app.refresh);
    assert!(app.background_cache_pending);
    assert!(!app.is_routing);
    assert_eq!(app.tick_count, 1);
  }

  #[test]
  fn test_handle_error_preserves_only_last_100_errors() {
    let mut app = App::default();

    for i in 0..105 {
      app.handle_error(anyhow!("error {}", i));
    }

    assert_eq!(app.error_history.len(), MAX_ERROR_HISTORY);
    assert_eq!(app.error_history.front().unwrap().message, "error 5");
    assert_eq!(app.error_history.back().unwrap().message, "error 104");
    assert_eq!(app.api_error, "error 104");
  }

  #[test]
  fn test_handle_error_stores_unsanitized_history_but_sanitizes_ui_message() {
    let mut app = App::default();

    app.handle_error(anyhow!(
      "Failed to get namespaced resource kdash::app::pods::KubePod. timeout"
    ));

    assert_eq!(
      app.error_history.back().unwrap().message,
      "Failed to get namespaced resource Pod. timeout"
    );
    assert_eq!(
      app.api_error,
      "Failed to get namespaced resource Pod. timeout"
    );
  }

  #[test]
  fn test_status_message_expires_after_5_seconds() {
    let mut app = App::default();
    let now = Instant::now();
    app.status_message.duration = Duration::from_secs(5);
    app
      .status_message
      .show_at("Saved recent errors to /tmp/kdash-errors.log", now);

    app.clear_expired_status_message(now + Duration::from_secs(4));
    assert_eq!(
      app.status_message.text(),
      "Saved recent errors to /tmp/kdash-errors.log"
    );

    app.clear_expired_status_message(now + Duration::from_secs(5));
    assert!(app.status_message.is_empty());
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
    // test refresh (non-first-render) — essential data loads now, background cache waits
    app.on_tick(false).await;
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::RefreshClient);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetKubeConfig);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNamespaces);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNodes);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetPods);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNamespaces);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNodes);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetPods);

    assert_eq!(
      sync_io_stream_rx.recv().await.unwrap(),
      IoStreamEvent::RefreshClient
    );
    assert_eq!(sync_io_cmd_rx.recv().await.unwrap(), IoCmdEvent::GetCliInfo);

    assert!(!app.refresh);
    assert!(app.background_cache_pending);
    assert!(!app.is_routing);
    assert_eq!(app.tick_count, 3);
  }

  #[tokio::test]
  async fn test_on_tick_dispatches_background_cache_on_followup_tick() {
    let (sync_io_tx, mut sync_io_rx) = mpsc::channel::<IoEvent>(500);

    let mut app = App {
      tick_until_poll: 5,
      tick_count: 1,
      refresh: false,
      background_cache_pending: true,
      io_tx: Some(sync_io_tx),
      ..App::default()
    };

    app.on_tick(false).await;

    assert_eq!(
      sync_io_rx.recv().await.unwrap(),
      IoEvent::DiscoverDynamicRes
    );
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
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetEvents);
    assert_eq!(
      sync_io_rx.recv().await.unwrap(),
      IoEvent::GetNetworkPolicies
    );
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetMetrics);

    assert!(!app.background_cache_pending);
    assert_eq!(app.tick_count, 2);
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
  fn test_pop_navigation_stack_on_default_route_returns_none() {
    let mut app = App::default();

    assert_eq!(app.pop_navigation_stack(), None);
    assert!(app.is_routing);
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Pods);
  }

  #[test]
  fn test_get_prev_route_with_single_route_returns_default_route() {
    let app = App::default();

    assert_eq!(app.get_prev_route().active_block, ActiveBlock::Pods);
    assert_eq!(
      app.get_nth_route_from_last(99).active_block,
      ActiveBlock::Pods
    );
  }

  #[test]
  fn test_route_helpers_switch_main_tabs() {
    let mut app = App::default();

    app.route_contexts();
    assert_eq!(app.main_tabs.index, 1);
    assert_eq!(app.get_current_route().id, RouteId::Contexts);

    app.route_utilization();
    assert_eq!(app.main_tabs.index, 2);
    assert_eq!(app.get_current_route().id, RouteId::Utilization);

    app.route_troubleshoot();
    assert_eq!(app.main_tabs.index, 3);
    assert_eq!(app.get_current_route().id, RouteId::Troubleshoot);

    app.route_home();
    assert_eq!(app.main_tabs.index, 0);
    assert_eq!(app.get_current_route().id, RouteId::Home);
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Pods);
  }

  #[test]
  fn test_restore_route_state_replaces_stack_and_sets_indices() {
    let mut app = App::default();

    app.restore_route_state(
      2,
      6,
      Route {
        id: RouteId::Utilization,
        active_block: ActiveBlock::Utilization,
      },
    );

    assert_eq!(app.main_tabs.index, 2);
    assert_eq!(app.context_tabs.index, 6);
    assert_eq!(app.navigation_stack.len(), 1);
    assert_eq!(app.get_current_route().id, RouteId::Utilization);
    assert!(app.is_routing);
  }

  #[test]
  fn test_set_and_clear_status_message_manage_api_error() {
    let mut app = App {
      api_error: "boom".into(),
      ..App::default()
    };

    app.set_status_message("all good");
    assert!(app.api_error.is_empty());
    assert_eq!(app.status_message.text(), "all good");

    app.clear_status_message();
    assert!(app.status_message.is_empty());
  }

  #[test]
  fn test_active_home_cache_block_uses_previous_route_for_logs() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Events);
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Logs);

    assert_eq!(app.active_home_cache_block(), ActiveBlock::Events);
    assert_eq!(
      app.background_home_event_to_skip(),
      Some(IoEvent::GetEvents)
    );
  }

  #[test]
  fn test_background_home_event_to_skip_returns_none_for_non_resource_blocks() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::Home, ActiveBlock::More);

    assert_eq!(app.active_home_cache_block(), ActiveBlock::More);
    assert_eq!(app.background_home_event_to_skip(), None);
  }

  #[test]
  fn test_loading_counter_default() {
    let app = App::default();
    assert!(!app.is_loading());
  }

  #[test]
  fn test_refresh_restore_route_uses_parent_for_transient_home_views() {
    let mut app = App::default();
    app.context_tabs.set_index(1);
    app.push_navigation_route(app.context_tabs.get_active_route().clone());
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Describe);

    assert_eq!(
      app.refresh_restore_route(),
      Route {
        id: RouteId::Home,
        active_block: ActiveBlock::Services,
      }
    );
  }

  #[test]
  fn test_refresh_restore_route_uses_parent_for_help_menu() {
    let mut app = App::default();
    app.route_contexts();
    app.push_navigation_stack(RouteId::HelpMenu, ActiveBlock::Help);

    assert_eq!(
      app.refresh_restore_route(),
      Route {
        id: RouteId::Contexts,
        active_block: ActiveBlock::Contexts,
      }
    );
  }

  #[test]
  fn test_refresh_restore_route_uses_parent_for_filtered_pod_drilldown() {
    let mut app = App::default();
    app.context_tabs.set_index(6);
    app.push_navigation_route(app.context_tabs.get_active_route().clone());
    app.data.selected.pod_selector = Some("app=nginx".into());
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Pods);

    assert_eq!(
      app.refresh_restore_route(),
      Route {
        id: RouteId::Home,
        active_block: ActiveBlock::Deployments,
      }
    );
  }

  #[test]
  fn test_refresh_restore_route_uses_more_menu_for_more_resources() {
    let mut app = App::default();
    app.context_tabs.set_index(9);
    app.push_navigation_route(app.context_tabs.get_active_route().clone());
    app.push_navigation_stack(RouteId::Home, ActiveBlock::Secrets);

    assert_eq!(
      app.refresh_restore_route(),
      Route {
        id: RouteId::Home,
        active_block: ActiveBlock::More,
      }
    );
  }

  #[test]
  fn test_refresh_restore_route_uses_dynamic_menu_for_dynamic_resources() {
    let mut app = App::default();
    app.context_tabs.set_index(10);
    app.push_navigation_route(app.context_tabs.get_active_route().clone());
    app.push_navigation_stack(RouteId::Home, ActiveBlock::DynamicResource);

    assert_eq!(
      app.refresh_restore_route(),
      Route {
        id: RouteId::Home,
        active_block: ActiveBlock::DynamicView,
      }
    );
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

  #[tokio::test]
  async fn test_dispatch_pod_logs_enqueues_all_container_logs_and_routes_to_logs() {
    let (sync_io_stream_tx, mut sync_io_stream_rx) = mpsc::channel::<IoStreamEvent>(16);

    let mut app = App {
      io_stream_tx: Some(sync_io_stream_tx),
      ..App::default()
    };

    app.dispatch_pod_logs("pod-a".into(), RouteId::Home).await;

    assert_eq!(app.data.logs.id, "agg:pod-a");
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Logs);
    assert_eq!(
      sync_io_stream_rx.recv().await.unwrap(),
      IoStreamEvent::GetPodAllContainerLogs
    );
  }

  #[tokio::test]
  async fn test_dispatch_container_logs_enqueues_container_log_stream() {
    let (sync_io_stream_tx, mut sync_io_stream_rx) = mpsc::channel::<IoStreamEvent>(16);

    let mut app = App {
      io_stream_tx: Some(sync_io_stream_tx),
      ..App::default()
    };

    app
      .dispatch_container_logs("pod-a/container-1".into(), RouteId::Home)
      .await;

    assert_eq!(app.data.logs.id, "pod-a/container-1");
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Logs);
    assert_eq!(
      sync_io_stream_rx.recv().await.unwrap(),
      IoStreamEvent::GetPodLogs(true)
    );
  }

  #[tokio::test]
  async fn test_dispatch_by_active_block_uses_selector_for_pod_drilldown() {
    let (sync_io_tx, mut sync_io_rx) = mpsc::channel::<IoEvent>(16);

    let mut app = App {
      io_tx: Some(sync_io_tx),
      ..App::default()
    };
    app.data.selected.pod_selector = Some("app=nginx".into());
    app.data.selected.pod_selector_ns = Some("default".into());

    app.dispatch_by_active_block(ActiveBlock::Pods).await;

    assert_eq!(
      sync_io_rx.recv().await.unwrap(),
      IoEvent::GetPodsBySelector {
        namespace: "default".into(),
        selector: "app=nginx".into(),
      }
    );
  }

  #[tokio::test]
  async fn test_dispatch_by_active_block_logs_dispatches_only_when_not_streaming() {
    let (sync_io_stream_tx, mut sync_io_stream_rx) = mpsc::channel::<IoStreamEvent>(16);

    let mut app = App {
      io_stream_tx: Some(sync_io_stream_tx),
      is_streaming: false,
      ..App::default()
    };

    app.dispatch_by_active_block(ActiveBlock::Logs).await;

    assert_eq!(
      sync_io_stream_rx.recv().await.unwrap(),
      IoStreamEvent::GetPodLogs(false)
    );

    app.is_streaming = true;
    app.dispatch_by_active_block(ActiveBlock::Logs).await;

    assert!(sync_io_stream_rx.try_recv().is_err());
  }

  #[tokio::test]
  async fn test_cache_essential_data_on_utilization_route_fetches_metrics() {
    let (sync_io_tx, mut sync_io_rx) = mpsc::channel::<IoEvent>(16);

    let mut app = App {
      io_tx: Some(sync_io_tx),
      ..App::default()
    };
    app.route_utilization();

    app.cache_essential_data().await;

    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNamespaces);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNodes);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetMetrics);
  }

  #[tokio::test]
  async fn test_cache_essential_data_on_troubleshoot_route_fetches_findings() {
    let (sync_io_tx, mut sync_io_rx) = mpsc::channel::<IoEvent>(16);

    let mut app = App {
      io_tx: Some(sync_io_tx),
      ..App::default()
    };
    app.route_troubleshoot();

    app.cache_essential_data().await;

    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNamespaces);
    assert_eq!(sync_io_rx.recv().await.unwrap(), IoEvent::GetNodes);
    assert_eq!(
      sync_io_rx.recv().await.unwrap(),
      IoEvent::GetTroubleshootFindings
    );
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
