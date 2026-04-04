pub(crate) mod stream;

use core::convert::TryFrom;
use std::{
  env, fmt,
  io::ErrorKind,
  path::{Path, PathBuf},
  sync::Arc,
};

use anyhow::{anyhow, Result};
use k8s_openapi::{api::core::v1::Pod, NamespaceResourceScope};
use kube::{
  api::ListParams,
  config::{KubeConfigOptions, Kubeconfig},
  discovery::verbs,
  Api, Client, Discovery, Resource as ApiResource,
};
use log::{debug, error, info, warn};
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;

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

#[derive(Debug, Eq, PartialEq)]
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
}

async fn refresh_kube_config(context: &Option<String>) -> Result<kube::Client> {
  // HACK force refresh token by calling "kubectl cluster-info before loading configuration"
  let mut args = vec!["cluster-info"];

  if let Some(context) = context {
    args.push("--context");
    args.push(context.as_str());
  }
  let out = duct::cmd("kubectl", &args)
    .stderr_null()
    // we don't care about the output
    .stdout_null()
    .read();

  if out.is_err() {
    error!("Running `kubectl cluster-info` failed");
    return Err(anyhow!("Running `kubectl cluster-info` failed",));
  }
  get_client(context.to_owned()).await
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
    return kube::Config::from_custom_kubeconfig(kubeconfig, &options)
      .await
      .map_err(|e| anyhow!("Failed to load Kubernetes config. {}", e));
  }

  if let Some(context) = context {
    Err(anyhow!(
      "Failed to load Kubernetes config: no valid kubeconfig was found for context {}",
      context
    ))
  } else {
    kube::Config::incluster().map_err(|e| anyhow!("Failed to load Kubernetes config. {}", e))
  }
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
  Ok(kube::Client::try_from(client_config)?)
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
    let (context, ns) = {
      let mut app = self.app.lock().await;
      let context = app.data.selected.context.clone();
      let ns = app.data.selected.ns.clone();
      // so that if refresh fails we dont see mixed results
      app.data.selected.context = None;
      (context, ns)
    };

    match refresh_kube_config(&context).await {
      Ok(client) => {
        self.client = client;
        let mut app = self.app.lock().await;
        app.reset();
        app.data.selected.context = context;
        app.data.selected.ns = ns;
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

  /// Discover and cache custom resources on the cluster
  pub async fn discover_dynamic_resources(&self) {
    let discovery = match Discovery::new(self.client.clone()).run().await {
      Ok(d) => d,
      Err(e) => {
        self
          .handle_error(anyhow!("Failed to get dynamic resources. {}", e))
          .await;
        return;
      }
    };

    let mut dynamic_resources = vec![];
    let mut dynamic_menu = vec![];

    let excluded = vec![
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

    for group in discovery.groups() {
      for (ar, caps) in group.recommended_resources() {
        if !caps.supports_operation(verbs::LIST) || excluded.contains(&ar.kind.as_str()) {
          continue;
        }

        dynamic_menu.push((ar.kind.to_string(), ActiveBlock::DynamicResource));
        dynamic_resources.push(KubeDynamicKind::new(ar, caps.scope));
      }
    }
    let mut app = self.app.lock().await;
    // sort dynamic_menu alphabetically using the first element of the tuple
    dynamic_menu.sort_by(|a, b| a.0.cmp(&b.0));
    app.dynamic_resources_menu = StatefulList::with_items(dynamic_menu);
    app.data.dynamic_kinds = dynamic_resources.clone();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::{
    fs,
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
}
