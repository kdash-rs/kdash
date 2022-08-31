// adapted from https://github.com/Rigellute/spotify-tui
mod kube_api;
pub(crate) mod stream;

use core::convert::TryFrom;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use kube::Client;
use tokio::sync::Mutex;

use crate::app::App;

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
  GetMetrics,
  RefreshClient,
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
        self.get_nodes().await;
      }
      IoEvent::GetNamespaces => {
        self.get_namespaces().await;
      }
      IoEvent::GetPods => {
        self.get_pods().await;
      }
      IoEvent::GetServices => {
        self.get_services().await;
      }
      IoEvent::GetConfigMaps => {
        self.get_config_maps().await;
      }
      IoEvent::GetStatefulSets => {
        self.get_stateful_sets().await;
      }
      IoEvent::GetReplicaSets => {
        self.get_replica_sets().await;
      }
      IoEvent::GetJobs => {
        self.get_jobs().await;
      }
      IoEvent::GetDaemonSets => {
        self.get_daemon_sets_jobs().await;
      }
      IoEvent::GetCronJobs => {
        self.get_cron_jobs().await;
      }
      IoEvent::GetSecrets => {
        self.get_secrets().await;
      }
      IoEvent::GetDeployments => {
        self.get_deployments().await;
      }
      IoEvent::GetReplicationControllers => {
        self.get_replication_controllers().await;
      }
      IoEvent::GetMetrics => {
        self.get_utilizations().await;
      }
      IoEvent::GetStorageClasses => {
        self.get_storage_classes().await;
      }
      IoEvent::GetRoles => {
        self.get_roles().await;
      }
      IoEvent::GetRoleBindings => {
        self.get_role_bindings().await;
      }
      IoEvent::GetClusterRoles => {
        self.get_cluster_roles().await;
      }
      IoEvent::GetClusterRoleBinding => {
        self.get_cluster_role_binding().await;
      }
    };

    let mut app = self.app.lock().await;
    app.is_loading = false;
  }

  async fn handle_error(&self, e: anyhow::Error) {
    let mut app = self.app.lock().await;
    app.handle_error(e);
  }
}
