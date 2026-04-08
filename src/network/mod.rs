pub(crate) mod stream;

use core::convert::TryFrom;
use std::{
  env, fmt,
  io::ErrorKind,
  path::{Path, PathBuf},
  process::Stdio,
  sync::Arc,
  time::Duration,
};

use anyhow::{anyhow, Context, Result};
use k8s_openapi::{
  api::core::v1::Pod, apimachinery::pkg::apis::meta::v1::APIGroup as DiscoveryApiGroup,
  NamespaceResourceScope,
};
use kube::{
  api::ListParams,
  config::{KubeConfigOptions, Kubeconfig},
  core::{DynamicObject, GroupVersion},
  discovery::{pinned_group, verbs, Scope},
  Api, Client, Resource as ApiResource,
};
use log::{debug, error, info, warn};
use serde::de::DeserializeOwned;
use tokio::{process::Command, sync::Mutex, time::timeout};

use crate::app::{
  configmaps::ConfigMapResource,
  contexts,
  cronjobs::CronJobResource,
  daemonsets::DaemonSetResource,
  deployments::DeploymentResource,
  dynamic::{DynamicResource, KubeDynamicKind},
  ingress::IngressResource,
  jobs::JobResource,
  metrics::UtilizationResource,
  models::{AppResource, StatefulList},
  network_policies::NetworkPolicyResource,
  nodes::NodeResource,
  ns::NamespaceResource,
  pods::{KubePod, PodResource},
  pvcs::PvcResource,
  pvs::PvResource,
  replicasets::ReplicaSetResource,
  replication_controllers::ReplicationControllerResource,
  roles::{ClusterRoleBindingResource, ClusterRoleResource, RoleBindingResource, RoleResource},
  secrets::SecretResource,
  serviceaccounts::SvcAcctResource,
  statefulsets::StatefulSetResource,
  storageclass::StorageClassResource,
  svcs::SvcResource,
  troubleshoot::TroubleshootResource,
  ActiveBlock, App,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IoEvent {
  GetKubeConfig,
  GetNodes,
  GetNamespaces,
  GetPods,
  GetServices,
  GetConfigMaps,
  GetStatefulSets,
  GetReplicaSets,
  GetDeployments,
  GetJobs,
  GetDaemonSets,
  GetCronJobs,
  GetSecrets,
  GetReplicationControllers,
  GetStorageClasses,
  GetRoles,
  GetRoleBindings,
  GetClusterRoles,
  GetClusterRoleBinding,
  GetIngress,
  GetPvcs,
  GetPvs,
  GetServiceAccounts,
  GetMetrics,
  GetTroubleshootFindings,
  RefreshClient,
  DiscoverDynamicRes,
  GetDynamicRes,
  GetNetworkPolicies,
  GetPodsBySelector { namespace: String, selector: String },
  GetPodsByNode { node_name: String },
}

async fn refresh_kube_config(context: &Option<String>) -> Result<kube::Client> {
  match get_client(context.clone()).await {
    Ok(client) => Ok(client),
    Err(err) if should_retry_kubectl_refresh(&err) => {
      warn!(
        "Initial client refresh failed with auth-related error, retrying after `kubectl cluster-info`: {:#}",
        err
      );
      run_kubectl_cluster_info(context, Duration::from_secs(3)).await?;
      get_client(context.clone()).await
    }
    Err(err) => Err(err),
  }
}

fn should_retry_kubectl_refresh(error: &anyhow::Error) -> bool {
  error.chain().any(|cause| {
    cause
      .downcast_ref::<kube::Error>()
      .is_some_and(|error| match error {
        kube::Error::Auth(_) => true,
        kube::Error::Api(status) => status.code == 401 || status.reason == "Unauthorized",
        _ => false,
      })
      || {
        let message = cause.to_string().to_lowercase();
        message.contains("auth exec")
          || message.contains("failed exec auth")
          || message.contains("exec-plugin")
          || message.contains("oidc")
          || message.contains("oauth")
          || message.contains("unauthorized")
      }
  })
}

async fn run_kubectl_cluster_info(context: &Option<String>, max_wait: Duration) -> Result<()> {
  let mut command = Command::new("kubectl");
  command
    .arg("cluster-info")
    .stderr(Stdio::null())
    .stdout(Stdio::null())
    .kill_on_drop(true);

  if let Some(context) = context {
    command.arg("--context").arg(context);
  }

  let mut child = command
    .spawn()
    .context("Failed to start `kubectl cluster-info`")?;
  match timeout(max_wait, child.wait()).await {
    Ok(Ok(status)) if status.success() => Ok(()),
    Ok(Ok(status)) => Err(anyhow!(
      "`kubectl cluster-info` exited with status {}",
      status
    )),
    Ok(Err(err)) => Err(anyhow!(
      "Failed to wait for `kubectl cluster-info`. {}",
      err
    )),
    Err(_) => {
      let _ = child.kill().await;
      let _ = child.wait().await;
      Err(anyhow!(
        "`kubectl cluster-info` timed out after {} seconds",
        max_wait.as_secs()
      ))
    }
  }
}

fn is_blank_kubeconfig(config: &Kubeconfig) -> bool {
  config.current_context.is_none()
    && config.clusters.is_empty()
    && config.auth_infos.is_empty()
    && config.contexts.is_empty()
}

fn format_invalid_kubeconfig_error(source: &str, problems: &[String]) -> anyhow::Error {
  anyhow!(
    "Failed to load Kubernetes config from {}: {}",
    source,
    problems.join("; ")
  )
}

fn load_kubeconfig_path(path: &Path) -> std::result::Result<Kubeconfig, String> {
  match Kubeconfig::read_from(path) {
    Ok(config) => {
      if is_blank_kubeconfig(&config) {
        Err(format!("ignored blank kubeconfig {:?}", path))
      } else {
        Ok(config)
      }
    }
    Err(kube::config::KubeconfigError::ReadConfig(err, path))
      if err.kind() == ErrorKind::NotFound =>
    {
      Err(format!("ignored missing kubeconfig {:?}", path))
    }
    Err(e) => Err(format!("ignored invalid kubeconfig {:?}: {}", path, e)),
  }
}

fn load_kubeconfig_from_paths(paths: &[PathBuf]) -> Result<Kubeconfig> {
  let mut config = Kubeconfig::default();
  let mut loaded = false;
  let mut problems = vec![];

  for path in paths {
    match load_kubeconfig_path(path) {
      Ok(next) => {
        config = config.merge(next)?;
        loaded = true;
      }
      Err(problem) => {
        warn!("{}", problem);
        problems.push(problem);
      }
    }
  }

  if loaded {
    Ok(config)
  } else {
    Err(format_invalid_kubeconfig_error("KUBECONFIG", &problems))
  }
}

fn load_local_kubeconfig() -> Result<Option<Kubeconfig>> {
  match env::var_os("KUBECONFIG") {
    Some(value) => {
      let paths = env::split_paths(&value)
        .filter(|path| !path.as_os_str().is_empty())
        .collect::<Vec<_>>();

      if paths.is_empty() {
        return Ok(None);
      }

      load_kubeconfig_from_paths(&paths).map(Some)
    }
    None => match Kubeconfig::read() {
      Ok(config) => {
        if is_blank_kubeconfig(&config) {
          Err(anyhow!(
            "Failed to load Kubernetes config from default kubeconfig: kubeconfig file is blank"
          ))
        } else {
          Ok(Some(config))
        }
      }
      Err(kube::config::KubeconfigError::FindPath) => Ok(None),
      Err(kube::config::KubeconfigError::ReadConfig(err, _))
        if err.kind() == ErrorKind::NotFound =>
      {
        Ok(None)
      }
      Err(e) => Err(anyhow!("Failed to load Kubernetes config. {}", e)),
    },
  }
}

async fn load_client_config(context: Option<String>) -> Result<kube::Config> {
  let options = KubeConfigOptions {
    context: context.clone(),
    ..Default::default()
  };

  if let Some(kubeconfig) = load_local_kubeconfig()? {
    return load_client_config_from_kubeconfig(kubeconfig, options).await;
  }

  if let Some(context) = context {
    Err(anyhow!(
      "Failed to load Kubernetes config: no valid kubeconfig was found for context {}",
      context
    ))
  } else {
    kube::Config::incluster().context("Failed to load Kubernetes config")
  }
}

async fn load_client_config_from_kubeconfig(
  kubeconfig: Kubeconfig,
  options: KubeConfigOptions,
) -> Result<kube::Config> {
  let mut config = kube::Config::from_custom_kubeconfig(kubeconfig, &options)
    .await
    .context("Failed to load Kubernetes config")?;

  if config.proxy_url.is_none() {
    if let Some(proxy_url) = env_https_proxy_url() {
      match proxy_url.parse() {
        Ok(parsed) => config.proxy_url = Some(parsed),
        Err(error) => warn!(
          "Ignoring invalid HTTPS proxy URL {:?}: {}",
          proxy_url, error
        ),
      }
    }
  }

  Ok(config)
}

fn env_https_proxy_url() -> Option<String> {
  env::vars_os().find_map(|(key, value)| {
    let normalized = key.to_string_lossy();
    if normalized.eq_ignore_ascii_case("HTTPS_PROXY") {
      Some(value.to_string_lossy().into_owned())
    } else {
      None
    }
  })
}

pub async fn get_client(context: Option<String>) -> Result<kube::Client> {
  debug!("env KUBECONFIG: {:?}", env::var_os("KUBECONFIG"));
  let client_config = match context.as_ref() {
    Some(context) => {
      info!("Getting kubernetes client. Context: {}", context);
      load_client_config(Some(context.to_owned())).await?
    }
    None => {
      warn!("Getting kubernetes client by inference. No context given");
      load_client_config(None).await?
    }
  };
  debug!("Kubernetes client config: {:?}", client_config);
  info!("Kubernetes client connected");
  kube::Client::try_from(client_config).context("Failed to create Kubernetes client")
}

#[derive(Clone)]
pub struct Network<'a> {
  pub client: Client,
  pub app: &'a Arc<Mutex<App>>,
}

impl<'a> Network<'a> {
  pub fn new(client: Client, app: &'a Arc<Mutex<App>>) -> Self {
    Network { client, app }
  }

  pub async fn refresh_client(&mut self) {
    let (context, ns, main_tab_index, context_tab_index, route) = {
      let mut app = self.app.lock().await;
      let context = app.data.selected.context.clone();
      let ns = app.data.selected.ns.clone();
      let main_tab_index = app.main_tabs.index;
      let context_tab_index = app.context_tabs.index;
      let route = app.refresh_restore_route();
      // so that if refresh fails we dont see mixed results
      app.data.selected.context = None;
      (context, ns, main_tab_index, context_tab_index, route)
    };

    match refresh_kube_config(&context).await {
      Ok(client) => {
        self.client = client;
        let mut app = self.app.lock().await;
        app.reset();
        app.data.selected.context = context;
        app.data.selected.ns = ns;
        app.restore_route_state(main_tab_index, context_tab_index, route);
        app.status_message.show("Refresh complete!");
      }
      Err(e) => {
        self
          .handle_error(anyhow!(
            "Failed to refresh client. {}. Loading default context.",
            e
          ))
          .await;
      }
    }
  }

  #[allow(clippy::cognitive_complexity)]
  pub async fn handle_network_event(&mut self, io_event: IoEvent) {
    match io_event {
      IoEvent::RefreshClient => {
        self.refresh_client().await;
      }
      IoEvent::GetKubeConfig => {
        self.get_kube_config().await;
      }
      IoEvent::GetNodes => {
        NodeResource::get_resource(self).await;
      }
      IoEvent::GetNamespaces => {
        NamespaceResource::get_resource(self).await;
      }
      IoEvent::GetPods => {
        PodResource::get_resource(self).await;
      }
      IoEvent::GetServices => {
        SvcResource::get_resource(self).await;
      }
      IoEvent::GetConfigMaps => {
        ConfigMapResource::get_resource(self).await;
      }
      IoEvent::GetStatefulSets => {
        StatefulSetResource::get_resource(self).await;
      }
      IoEvent::GetReplicaSets => {
        ReplicaSetResource::get_resource(self).await;
      }
      IoEvent::GetJobs => {
        JobResource::get_resource(self).await;
      }
      IoEvent::GetDaemonSets => {
        DaemonSetResource::get_resource(self).await;
      }
      IoEvent::GetCronJobs => {
        CronJobResource::get_resource(self).await;
      }
      IoEvent::GetSecrets => {
        SecretResource::get_resource(self).await;
      }
      IoEvent::GetDeployments => {
        DeploymentResource::get_resource(self).await;
      }
      IoEvent::GetReplicationControllers => {
        ReplicationControllerResource::get_resource(self).await;
      }
      IoEvent::GetMetrics => {
        UtilizationResource::get_resource(self).await;
      }
      IoEvent::GetTroubleshootFindings => {
        TroubleshootResource::get_resource(self).await;
      }
      IoEvent::GetStorageClasses => {
        StorageClassResource::get_resource(self).await;
      }
      IoEvent::GetRoles => {
        RoleResource::get_resource(self).await;
      }
      IoEvent::GetRoleBindings => {
        RoleBindingResource::get_resource(self).await;
      }
      IoEvent::GetClusterRoles => {
        ClusterRoleResource::get_resource(self).await;
      }
      IoEvent::GetClusterRoleBinding => {
        ClusterRoleBindingResource::get_resource(self).await;
      }
      IoEvent::GetIngress => {
        IngressResource::get_resource(self).await;
      }
      IoEvent::GetPvcs => {
        PvcResource::get_resource(self).await;
      }
      IoEvent::GetPvs => {
        PvResource::get_resource(self).await;
      }
      IoEvent::GetServiceAccounts => {
        SvcAcctResource::get_resource(self).await;
      }
      IoEvent::GetNetworkPolicies => {
        NetworkPolicyResource::get_resource(self).await;
      }
      IoEvent::DiscoverDynamicRes => {
        self.discover_dynamic_resources().await;
      }
      IoEvent::GetDynamicRes => {
        DynamicResource::get_resource(self).await;
      }
      IoEvent::GetPodsBySelector {
        namespace,
        selector,
      } => {
        self.get_pods_by_selector(&namespace, &selector).await;
      }
      IoEvent::GetPodsByNode { node_name } => {
        self.get_pods_by_node(&node_name).await;
      }
    };

    let mut app = self.app.lock().await;
    app.loading_complete();
  }

  pub async fn handle_error(&self, e: anyhow::Error) {
    error!("{:?}", e);
    let mut app = self.app.lock().await;
    app.handle_error(e);
  }

  pub async fn get_kube_config(&self) {
    match load_local_kubeconfig() {
      Ok(Some(config)) => {
        info!("Using Kubeconfig");
        debug!("Kubeconfig: {:?}", config);
        let mut app = self.app.lock().await;
        let selected_ctx = app.data.selected.context.to_owned();

        // Detect external context change (#315): if the user hasn't manually
        // selected a context (selected.context is None) and the kubeconfig's
        // current_context differs from what we had, trigger a refresh.
        if selected_ctx.is_none() {
          let prev_ctx = app.data.active_context.as_ref().map(|c| c.name.clone());
          if prev_ctx.is_some() && prev_ctx != config.current_context {
            info!(
              "External context change detected: {:?} -> {:?}",
              prev_ctx, config.current_context
            );
            app.set_contexts(contexts::get_contexts(&config, None));
            app.data.kubeconfig = Some(config);
            app.refresh();
            return;
          }
        }

        app.set_contexts(contexts::get_contexts(&config, selected_ctx));
        app.data.kubeconfig = Some(config);
      }
      Ok(None) => {
        self
          .handle_error(anyhow!(
            "Failed to load Kubernetes config. No kubeconfig was found"
          ))
          .await;
      }
      Err(e) => {
        self.handle_error(e).await;
      }
    }
  }

  /// calls the kubernetes API to list the given resource for either selected namespace or all namespaces
  pub async fn get_namespaced_resources<K, T, F>(&self, map_fn: F) -> Vec<T>
  where
    <K as ApiResource>::DynamicType: Default,
    K: kube::Resource<Scope = NamespaceResourceScope>,
    K: Clone + DeserializeOwned + fmt::Debug,
    F: Fn(K) -> T,
  {
    let api: Api<K> = self.get_namespaced_api().await;
    let lp = ListParams::default();
    match api.list(&lp).await {
      Ok(list) => list.into_iter().map(map_fn).collect::<Vec<_>>(),
      Err(e) => {
        self
          .handle_error(anyhow!(
            "Failed to get namespaced resource {}. {}",
            crate::app::utils::friendly_type_name::<T>(),
            e
          ))
          .await;
        vec![]
      }
    }
  }

  /// calls the kubernetes API to list the given resource for all namespaces
  pub async fn get_resources<K, T, F>(&self, map_fn: F) -> Vec<T>
  where
    <K as ApiResource>::DynamicType: Default,
    K: ApiResource + Clone + DeserializeOwned + fmt::Debug,
    F: Fn(K) -> T,
  {
    let api: Api<K> = Api::all(self.client.clone());
    let lp = ListParams::default();
    match api.list(&lp).await {
      Ok(list) => list.into_iter().map(map_fn).collect::<Vec<_>>(),
      Err(e) => {
        self
          .handle_error(anyhow!(
            "Failed to get resource {}. {}",
            crate::app::utils::friendly_type_name::<T>(),
            e
          ))
          .await;
        vec![]
      }
    }
  }

  pub async fn get_namespaced_api<K>(&self) -> Api<K>
  where
    <K as ApiResource>::DynamicType: Default,
    K: ApiResource + kube::Resource<Scope = NamespaceResourceScope>,
  {
    let app = self.app.lock().await;
    match &app.data.selected.ns {
      Some(ns) => Api::namespaced(self.client.clone(), ns),
      None => Api::all(self.client.clone()),
    }
  }

  /// Fetch pods matching a label selector in a specific namespace.
  /// Results are stored in `app.data.pods` for the drill-down flow.
  pub async fn get_pods_by_selector(&self, namespace: &str, selector: &str) {
    let api: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
    let lp = ListParams::default().labels(selector);
    match api.list(&lp).await {
      Ok(list) => {
        let items: Vec<KubePod> = list.into_iter().map(Pod::into).collect();
        let mut app = self.app.lock().await;
        app.data.pods.set_items(items);
      }
      Err(e) => {
        self
          .handle_error(anyhow!(
            "Failed to get pods for selector '{}'. {}",
            selector,
            e
          ))
          .await;
      }
    }
  }

  pub async fn get_pods_by_node(&self, node_name: &str) {
    let api: Api<Pod> = Api::all(self.client.clone());
    let lp = ListParams::default().fields(&format!("spec.nodeName={}", node_name));
    match api.list(&lp).await {
      Ok(list) => {
        let items: Vec<KubePod> = list.into_iter().map(Pod::into).collect();
        let mut app = self.app.lock().await;
        app.data.pods.set_items(items);
      }
      Err(e) => {
        self
          .handle_error(anyhow!(
            "Failed to get pods for node '{}'. {}",
            node_name,
            e
          ))
          .await;
      }
    }
  }

  pub async fn get_dynamic_resources(
    &self,
    drs: &KubeDynamicKind,
    namespace: Option<&str>,
  ) -> Result<Vec<crate::app::dynamic::KubeDynamicResource>> {
    let api: Api<DynamicObject> = if drs.scope == Scope::Cluster {
      Api::all_with(self.client.clone(), &drs.api_resource)
    } else {
      match namespace {
        Some(ns) => Api::namespaced_with(self.client.clone(), ns, &drs.api_resource),
        None => Api::all_with(self.client.clone(), &drs.api_resource),
      }
    };

    let list = api.list(&Default::default()).await?;
    Ok(
      list
        .items
        .into_iter()
        .map(crate::app::dynamic::KubeDynamicResource::from)
        .collect(),
    )
  }

  /// Discover and cache custom resources on the cluster
  pub async fn discover_dynamic_resources(&self) {
    let api_groups = match self.client.list_api_groups().await {
      Ok(groups) => groups.groups,
      Err(e) => {
        self
          .handle_error(anyhow!("Failed to get dynamic resources. {}", e))
          .await;
        return;
      }
    };

    let mut dynamic_resources = vec![];
    let mut dynamic_menu = vec![];

    let excluded = [
      "Namespace",
      "Pod",
      "Service",
      "Node",
      "ConfigMap",
      "StatefulSet",
      "ReplicaSet",
      "Deployment",
      "Job",
      "DaemonSet",
      "CronJob",
      "Secret",
      "ReplicationController",
      "PersistentVolumeClaim",
      "PersistentVolume",
      "StorageClass",
      "Role",
      "RoleBinding",
      "ClusterRole",
      "ClusterRoleBinding",
      "ServiceAccount",
      "Ingress",
      "NetworkPolicy",
    ];

    for api_group in api_groups {
      let group_name = api_group.name.clone();
      let Some(group_version) = preferred_group_version(&api_group) else {
        warn!(
          "Skipping dynamic API group '{}' because it has no preferred or parseable version",
          group_name
        );
        continue;
      };

      match pinned_group(&self.client, &group_version).await {
        Ok(group) => {
          for (ar, caps) in group.recommended_resources() {
            if !caps.supports_operation(verbs::LIST) || excluded.contains(&ar.kind.as_str()) {
              continue;
            }

            dynamic_menu.push((ar.kind.to_string(), ActiveBlock::DynamicResource));
            dynamic_resources.push(KubeDynamicKind::new(ar, caps.scope));
          }
        }
        Err(e) => {
          warn!(
            "Skipping dynamic API group '{}' at '{}' due to discovery error: {}",
            group_name,
            group_version.api_version(),
            e
          );
        }
      }
    }
    let mut app = self.app.lock().await;
    // sort dynamic_menu alphabetically using the first element of the tuple
    dynamic_menu.sort_by(|a, b| a.0.cmp(&b.0));
    app.dynamic_resources_menu = StatefulList::with_items(dynamic_menu);
    app.data.dynamic_kinds = dynamic_resources.clone();
  }
}

fn preferred_group_version(api_group: &DiscoveryApiGroup) -> Option<GroupVersion> {
  api_group
    .preferred_version
    .as_ref()
    .map(|version| version.group_version.as_str())
    .or_else(|| {
      api_group
        .versions
        .first()
        .map(|version| version.group_version.as_str())
    })
    .and_then(|group_version| {
      let parsed: GroupVersion = group_version.parse().ok()?;
      (parsed.group == api_group.name && !parsed.version.is_empty()).then_some(parsed)
    })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::{ActiveBlock, App, RouteId};
  use k8s_openapi::apimachinery::pkg::apis::meta::v1::GroupVersionForDiscovery;
  use kube::{client::AuthError, core::Status};
  use std::{
    ffi::OsString,
    fs,
    sync::{Mutex as StdMutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
  };

  fn temp_test_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("system time should be after epoch")
      .as_nanos();
    let path = env::temp_dir().join(format!(
      "kdash-network-tests-{}-{}-{}",
      name,
      std::process::id(),
      suffix
    ));
    fs::create_dir_all(&path).expect("temp test dir should be created");
    path
  }

  fn write_kubeconfig(path: &Path, contents: &str) {
    fs::write(path, contents).expect("kubeconfig fixture should be written");
  }

  fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<StdMutex<()>> = OnceLock::new();
    LOCK
      .get_or_init(|| StdMutex::new(()))
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  struct ProxyEnvGuard {
    https_proxy: Option<OsString>,
    https_proxy_lower: Option<OsString>,
  }

  impl ProxyEnvGuard {
    fn capture() -> Self {
      Self {
        https_proxy: env::var_os("HTTPS_PROXY"),
        https_proxy_lower: env::var_os("https_proxy"),
      }
    }
  }

  impl Drop for ProxyEnvGuard {
    fn drop(&mut self) {
      match &self.https_proxy {
        Some(value) => env::set_var("HTTPS_PROXY", value),
        None => env::remove_var("HTTPS_PROXY"),
      }

      match &self.https_proxy_lower {
        Some(value) => env::set_var("https_proxy", value),
        None => env::remove_var("https_proxy"),
      }
    }
  }

  fn kubeconfig_with_proxy(proxy_url: Option<&str>) -> String {
    let proxy_yaml = proxy_url
      .map(|proxy| format!("      proxy-url: {}\n", proxy))
      .unwrap_or_default();

    format!(
      r#"apiVersion: v1
kind: Config
clusters:
  - name: test-cluster
    cluster:
      server: https://127.0.0.1:6443
{proxy_yaml}contexts:
  - name: test-context
    context:
      cluster: test-cluster
      user: test-user
current-context: test-context
users:
  - name: test-user
    user:
      token: test-token
"#
    )
  }

  fn valid_kubeconfig() -> &'static str {
    r#"apiVersion: v1
kind: Config
clusters:
  - name: test-cluster
    cluster:
      server: https://127.0.0.1:6443
contexts:
  - name: test-context
    context:
      cluster: test-cluster
      user: test-user
current-context: test-context
users:
  - name: test-user
    user:
      token: test-token
"#
  }

  #[test]
  fn test_load_client_config_from_kubeconfig_uses_cluster_proxy_url() {
    let _env_lock = env_lock();
    let _proxy_env = ProxyEnvGuard::capture();
    env::remove_var("HTTPS_PROXY");
    env::remove_var("https_proxy");

    let kubeconfig: Kubeconfig = serde_yaml::from_str(&kubeconfig_with_proxy(Some(
      "http://cluster-proxy.internal:8443",
    )))
    .expect("proxy kubeconfig should deserialize");

    let config = tokio::runtime::Runtime::new()
      .expect("runtime should build")
      .block_on(load_client_config_from_kubeconfig(
        kubeconfig,
        KubeConfigOptions::default(),
      ))
      .expect("config should load");

    assert_eq!(
      config.proxy_url.as_ref().map(ToString::to_string),
      Some("http://cluster-proxy.internal:8443/".to_string())
    );
  }

  #[test]
  fn test_load_client_config_from_kubeconfig_uses_https_proxy_env_var() {
    let _env_lock = env_lock();
    let _proxy_env = ProxyEnvGuard::capture();
    env::set_var("HTTPS_PROXY", "http://env-proxy.internal:8080");
    env::remove_var("https_proxy");

    let kubeconfig: Kubeconfig = serde_yaml::from_str(&kubeconfig_with_proxy(None))
      .expect("base kubeconfig should deserialize");

    let config = tokio::runtime::Runtime::new()
      .expect("runtime should build")
      .block_on(load_client_config_from_kubeconfig(
        kubeconfig,
        KubeConfigOptions::default(),
      ))
      .expect("config should load");

    assert_eq!(
      config.proxy_url.as_ref().map(ToString::to_string),
      Some("http://env-proxy.internal:8080/".to_string())
    );
  }

  #[test]
  fn test_load_client_config_from_kubeconfig_uses_https_proxy_env_var_case_insensitively() {
    let _env_lock = env_lock();
    let _proxy_env = ProxyEnvGuard::capture();
    env::remove_var("HTTPS_PROXY");
    env::remove_var("https_proxy");
    env::set_var("Https_PrOxY", "http://env-proxy.internal:8080");

    let kubeconfig: Kubeconfig = serde_yaml::from_str(&kubeconfig_with_proxy(None))
      .expect("base kubeconfig should deserialize");

    let config = tokio::runtime::Runtime::new()
      .expect("runtime should build")
      .block_on(load_client_config_from_kubeconfig(
        kubeconfig,
        KubeConfigOptions::default(),
      ))
      .expect("config should load");

    env::remove_var("Https_PrOxY");

    assert_eq!(
      config.proxy_url.as_ref().map(ToString::to_string),
      Some("http://env-proxy.internal:8080/".to_string())
    );
  }

  #[test]
  fn test_load_client_config_from_kubeconfig_prefers_cluster_proxy_over_env_var() {
    let _env_lock = env_lock();
    let _proxy_env = ProxyEnvGuard::capture();
    env::set_var("HTTPS_PROXY", "http://env-proxy.internal:8080");
    env::remove_var("https_proxy");

    let kubeconfig: Kubeconfig = serde_yaml::from_str(&kubeconfig_with_proxy(Some(
      "http://cluster-proxy.internal:8443",
    )))
    .expect("proxy kubeconfig should deserialize");

    let config = tokio::runtime::Runtime::new()
      .expect("runtime should build")
      .block_on(load_client_config_from_kubeconfig(
        kubeconfig,
        KubeConfigOptions::default(),
      ))
      .expect("config should load");

    assert_eq!(
      config.proxy_url.as_ref().map(ToString::to_string),
      Some("http://cluster-proxy.internal:8443/".to_string())
    );
  }

  #[test]
  fn test_get_client_supports_http_proxy_configuration() {
    let _env_lock = env_lock();
    let _proxy_env = ProxyEnvGuard::capture();
    env::remove_var("HTTPS_PROXY");
    env::remove_var("https_proxy");

    let kubeconfig: Kubeconfig = serde_yaml::from_str(&kubeconfig_with_proxy(Some(
      "http://cluster-proxy.internal:8443",
    )))
    .expect("proxy kubeconfig should deserialize");

    let config = tokio::runtime::Runtime::new()
      .expect("runtime should build")
      .block_on(load_client_config_from_kubeconfig(
        kubeconfig,
        KubeConfigOptions::default(),
      ))
      .expect("config should load");
    let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
    let _enter = runtime.enter();

    Client::try_from(config).expect("http proxy support should be enabled");
  }

  #[test]
  fn test_load_kubeconfig_from_paths_ignores_missing_entries_when_valid_config_exists() {
    let dir = temp_test_dir("missing-valid");
    let missing = dir.join("missing-config");
    let valid = dir.join("valid-config");
    write_kubeconfig(&valid, valid_kubeconfig());

    let kubeconfig =
      load_kubeconfig_from_paths(&[missing, valid]).expect("valid kubeconfig should load");

    assert_eq!(kubeconfig.current_context.as_deref(), Some("test-context"));
    assert_eq!(kubeconfig.clusters.len(), 1);
    assert_eq!(kubeconfig.contexts.len(), 1);
    assert_eq!(kubeconfig.auth_infos.len(), 1);

    fs::remove_dir_all(dir).expect("temp test dir should be removed");
  }

  #[test]
  fn test_load_kubeconfig_from_paths_ignores_blank_entries_when_valid_config_exists() {
    let dir = temp_test_dir("blank-valid");
    let blank = dir.join("blank-config");
    let valid = dir.join("valid-config");
    write_kubeconfig(&blank, "");
    write_kubeconfig(&valid, valid_kubeconfig());

    let kubeconfig =
      load_kubeconfig_from_paths(&[blank, valid]).expect("valid kubeconfig should load");

    assert_eq!(kubeconfig.current_context.as_deref(), Some("test-context"));
    assert_eq!(kubeconfig.clusters.len(), 1);
    assert_eq!(kubeconfig.contexts.len(), 1);
    assert_eq!(kubeconfig.auth_infos.len(), 1);

    fs::remove_dir_all(dir).expect("temp test dir should be removed");
  }

  #[test]
  fn test_load_kubeconfig_from_paths_returns_clean_error_when_all_entries_are_invalid() {
    let dir = temp_test_dir("all-invalid");
    let missing = dir.join("missing-config");
    let blank = dir.join("blank-config");
    write_kubeconfig(&blank, "");

    let error = load_kubeconfig_from_paths(&[missing, blank])
      .expect_err("all invalid kubeconfigs should fail")
      .to_string();

    assert!(error.contains("Failed to load Kubernetes config from KUBECONFIG"));
    assert!(error.contains("ignored missing kubeconfig"));
    assert!(error.contains("ignored blank kubeconfig"));

    fs::remove_dir_all(dir).expect("temp test dir should be removed");
  }

  #[test]
  fn test_is_blank_kubeconfig_detects_empty_config() {
    let config = Kubeconfig::default();
    assert!(is_blank_kubeconfig(&config));
  }

  #[test]
  fn test_is_blank_kubeconfig_returns_false_for_populated_config() {
    let config = Kubeconfig {
      current_context: Some("ctx".to_string()),
      ..Default::default()
    };
    assert!(!is_blank_kubeconfig(&config));
  }

  #[test]
  fn test_load_kubeconfig_path_ok_for_valid_file() {
    let dir = temp_test_dir("path-valid");
    let file = dir.join("config");
    write_kubeconfig(&file, valid_kubeconfig());

    let config = load_kubeconfig_path(&file).expect("valid config should load");
    assert_eq!(config.current_context.as_deref(), Some("test-context"));

    fs::remove_dir_all(dir).expect("temp test dir should be removed");
  }

  #[test]
  fn test_load_kubeconfig_path_err_for_missing_file() {
    let dir = temp_test_dir("path-missing");
    let missing = dir.join("does-not-exist");

    let err = load_kubeconfig_path(&missing).expect_err("missing file should return Err");
    assert!(err.contains("ignored missing kubeconfig"));

    fs::remove_dir_all(dir).expect("temp test dir should be removed");
  }

  #[test]
  fn test_load_kubeconfig_path_err_for_blank_file() {
    let dir = temp_test_dir("path-blank");
    let blank = dir.join("blank");
    write_kubeconfig(&blank, "");

    let err = load_kubeconfig_path(&blank).expect_err("blank file should return Err");
    assert!(err.contains("ignored blank kubeconfig"));

    fs::remove_dir_all(dir).expect("temp test dir should be removed");
  }

  #[test]
  fn test_load_kubeconfig_from_paths_merges_multiple_valid_configs() {
    let dir = temp_test_dir("merge");
    let file_a = dir.join("config-a");
    let file_b = dir.join("config-b");

    write_kubeconfig(
      &file_a,
      r#"apiVersion: v1
kind: Config
clusters:
  - name: cluster-a
    cluster:
      server: https://a:6443
contexts:
  - name: ctx-a
    context:
      cluster: cluster-a
      user: user-a
current-context: ctx-a
users:
  - name: user-a
    user:
      token: token-a
"#,
    );

    write_kubeconfig(
      &file_b,
      r#"apiVersion: v1
kind: Config
clusters:
  - name: cluster-b
    cluster:
      server: https://b:6443
contexts:
  - name: ctx-b
    context:
      cluster: cluster-b
      user: user-b
users:
  - name: user-b
    user:
      token: token-b
"#,
    );

    let config =
      load_kubeconfig_from_paths(&[file_a, file_b]).expect("multiple valid configs should merge");

    // First file's current_context wins
    assert_eq!(config.current_context.as_deref(), Some("ctx-a"));
    assert_eq!(config.clusters.len(), 2);
    assert_eq!(config.contexts.len(), 2);
    assert_eq!(config.auth_infos.len(), 2);

    fs::remove_dir_all(dir).expect("temp test dir should be removed");
  }

  #[test]
  fn test_preferred_group_version_uses_preferred_version() {
    let api_group = DiscoveryApiGroup {
      name: "example.io".into(),
      preferred_version: Some(GroupVersionForDiscovery {
        group_version: "example.io/v1".into(),
        version: "v1".into(),
      }),
      server_address_by_client_cidrs: None,
      versions: vec![GroupVersionForDiscovery {
        group_version: "example.io/v1beta1".into(),
        version: "v1beta1".into(),
      }],
    };

    let group_version =
      preferred_group_version(&api_group).expect("preferred version should parse");
    assert_eq!(group_version.group, "example.io");
    assert_eq!(group_version.version, "v1");
  }

  #[test]
  fn test_preferred_group_version_falls_back_to_first_served_version() {
    let api_group = DiscoveryApiGroup {
      name: "example.io".into(),
      preferred_version: None,
      server_address_by_client_cidrs: None,
      versions: vec![GroupVersionForDiscovery {
        group_version: "example.io/v1beta1".into(),
        version: "v1beta1".into(),
      }],
    };

    let group_version = preferred_group_version(&api_group).expect("served version should parse");
    assert_eq!(group_version.group, "example.io");
    assert_eq!(group_version.version, "v1beta1");
  }

  #[test]
  fn test_preferred_group_version_returns_none_for_invalid_version_string() {
    let api_group = DiscoveryApiGroup {
      name: "example.io".into(),
      preferred_version: Some(GroupVersionForDiscovery {
        group_version: "too/many/slashes".into(),
        version: "v1".into(),
      }),
      server_address_by_client_cidrs: None,
      versions: vec![],
    };

    assert!(preferred_group_version(&api_group).is_none());
  }

  #[test]
  fn test_should_retry_kubectl_refresh_for_auth_error() {
    let error = anyhow!(kube::Error::Auth(AuthError::AuthExec(
      "refresh failed".into()
    )));

    assert!(should_retry_kubectl_refresh(&error));
  }

  #[test]
  fn test_should_retry_kubectl_refresh_for_unauthorized_api_error() {
    let error = anyhow!(kube::Error::Api(
      Status::failure("unauthorized", "Unauthorized")
        .with_code(401)
        .boxed()
    ));

    assert!(should_retry_kubectl_refresh(&error));
  }

  #[test]
  fn test_should_not_retry_kubectl_refresh_for_non_auth_error() {
    let error = anyhow!("Failed to load Kubernetes config");

    assert!(!should_retry_kubectl_refresh(&error));
  }

  #[allow(clippy::await_holding_lock)]
  #[tokio::test]
  async fn test_refresh_client_restores_home_route_and_resource_tab_state() {
    let _env_lock = env_lock();
    let previous_kubeconfig = env::var_os("KUBECONFIG");
    let dir = temp_test_dir("refresh-route-state");
    let kubeconfig_path = dir.join("config");
    write_kubeconfig(&kubeconfig_path, valid_kubeconfig());
    env::set_var("KUBECONFIG", &kubeconfig_path);

    let client = get_client(None)
      .await
      .expect("test kubeconfig should produce a client");
    let app = Arc::new(Mutex::new(App::default()));

    {
      let mut app = app.lock().await;
      app.data.selected.context = Some("test-context".into());
      app.data.selected.ns = Some("team-a".into());
      let route = app.context_tabs.set_index(1).route.clone();
      app.push_navigation_route(route);
    }

    let mut network = Network::new(client, &app);
    network.refresh_client().await;

    let app = app.lock().await;
    assert_eq!(app.main_tabs.index, 0);
    assert_eq!(app.context_tabs.index, 1);
    assert_eq!(app.get_current_route().id, RouteId::Home);
    assert_eq!(app.get_current_route().active_block, ActiveBlock::Services);
    assert_eq!(app.data.selected.context.as_deref(), Some("test-context"));
    assert_eq!(app.data.selected.ns.as_deref(), Some("team-a"));

    match previous_kubeconfig {
      Some(value) => env::set_var("KUBECONFIG", value),
      None => env::remove_var("KUBECONFIG"),
    }
    fs::remove_dir_all(dir).expect("temp test dir should be removed");
  }
}
