pub(crate) mod stream;

use core::convert::TryFrom;
use std::{fmt, sync::Arc};

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
  pods::PodResource,
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
  ActiveBlock, App,
};
use anyhow::{anyhow, Result};
use k8s_openapi::NamespaceResourceScope;
use kube::{
  api::ListParams, config::Kubeconfig, discovery::verbs, Api, Client, Discovery,
  Resource as ApiResource,
};
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;

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
  GetClusterRoleBindings,
  GetIngress,
  GetPvcs,
  GetPvs,
  GetServiceAccounts,
  GetMetrics,
  RefreshClient,
  DiscoverDynamicRes,
  GetDynamicRes,
  GetNetworkPolicies,
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
    return Err(anyhow!("Running `kubectl cluster-info` failed",));
  }
  get_client(context.to_owned()).await
}

pub async fn get_client(context: Option<String>) -> Result<kube::Client> {
  let client_config = match context.as_ref() {
    Some(context) => {
      kube::Config::from_kubeconfig(&kube::config::KubeConfigOptions {
        context: Some(context.to_owned()),
        ..Default::default()
      })
      .await?
    }
    None => kube::Config::infer().await?,
  };
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
    let context = {
      let mut app = self.app.lock().await;
      let context = app.data.selected.context.clone();
      // so that if refresh fails we dont see mixed results
      app.data.selected.context = None;
      context
    };

    match refresh_kube_config(&context).await {
      Ok(client) => {
        self.client = client;
        let mut app = self.app.lock().await;
        app.reset();
        app.data.selected.context = context;
      }
      Err(e) => {
        self
          .handle_error(anyhow!(
            "Failed to refresh client. {:?}. Loading default context. ",
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
      IoEvent::GetClusterRoleBindings => {
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
    };

    let mut app = self.app.lock().await;
    app.is_loading = false;
  }

  pub async fn handle_error(&self, e: anyhow::Error) {
    let mut app = self.app.lock().await;
    app.handle_error(e);
  }

  pub async fn get_kube_config(&self) {
    match Kubeconfig::read() {
      Ok(config) => {
        let mut app = self.app.lock().await;
        let selected_ctx = app.data.selected.context.to_owned();
        app.set_contexts(contexts::get_contexts(&config, selected_ctx));
        app.data.kubeconfig = Some(config);
      }
      Err(e) => {
        self
          .handle_error(anyhow!("Failed to load Kubernetes config. {:?}", e))
          .await;
      }
    }
  }

  /// calls the kubernetes API to list the given resource for either selected namespace or all namespaces
  pub async fn get_namespaced_resources<K: ApiResource, T, F>(&self, map_fn: F) -> Vec<T>
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
            "Failed to get namespaced resource {}. {:?}",
            std::any::type_name::<T>(),
            e
          ))
          .await;
        vec![]
      }
    }
  }

  /// calls the kubernetes API to list the given resource for all namespaces
  pub async fn get_resources<K: ApiResource, T, F>(&self, map_fn: F) -> Vec<T>
  where
    <K as ApiResource>::DynamicType: Default,
    K: Clone + DeserializeOwned + fmt::Debug,
    F: Fn(K) -> T,
  {
    let api: Api<K> = Api::all(self.client.clone());
    let lp = ListParams::default();
    match api.list(&lp).await {
      Ok(list) => list.into_iter().map(map_fn).collect::<Vec<_>>(),
      Err(e) => {
        self
          .handle_error(anyhow!(
            "Failed to get resource {}. {:?}",
            std::any::type_name::<T>(),
            e
          ))
          .await;
        vec![]
      }
    }
  }

  pub async fn get_namespaced_api<K: ApiResource>(&self) -> Api<K>
  where
    <K as ApiResource>::DynamicType: Default,
    K: kube::Resource<Scope = NamespaceResourceScope>,
  {
    let app = self.app.lock().await;
    match &app.data.selected.ns {
      Some(ns) => Api::namespaced(self.client.clone(), ns),
      None => Api::all(self.client.clone()),
    }
  }

  /// Discover and cache custom resources on the cluster
  pub async fn discover_dynamic_resources(&self) {
    let discovery = match Discovery::new(self.client.clone()).run().await {
      Ok(d) => d,
      Err(e) => {
        self
          .handle_error(anyhow!("Failed to get dynamic resources. {:?}", e))
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
