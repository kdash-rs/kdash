// adapted from https://github.com/Rigellute/spotify-tui
mod kube_api;
pub(crate) mod stream;

use super::app::{self, App};

use anyhow::{anyhow, Result};
use kube::Client;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
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
  GetMetrics,
  RefreshClient,
}

async fn _refresh_kube_config(context: &Option<String>) -> Result<()> {
  //HACK force refresh token by calling "kubectl cluster-info before loading configuration"
  use std::process::Command;
  let mut cmd = Command::new("kubectl");
  cmd.arg("cluster-info");
  if let Some(ref context) = context {
    cmd.arg("--context").arg(context);
  }
  let output = cmd.output()?;
  if !output.status.success() {
    return Err(anyhow!("`kubectl cluster-info` failed with: {:?}", &output));
  }
  Ok(())
}

pub async fn get_client(context: Option<String>) -> kube::Result<Client> {
  use core::convert::TryFrom;

  //   refresh_kube_config(&context)
  //     .await
  //     .expect("kubectl cluster-info` failed");

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
  kube::Client::try_from(client_config)
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
    // TODO find a better way to do this
    match get_client(None).await {
      Ok(client) => {
        self.client = client;
        let mut app = self.app.lock().await;
        app.reset();
      }
      Err(e) => self.handle_error(anyhow!(e)).await,
    };
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
      IoEvent::GetDeployments => {
        self.get_deployments().await;
      }
      IoEvent::GetMetrics => {
        self.get_utilizations().await;
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
