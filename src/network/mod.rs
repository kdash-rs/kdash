// adapted from https://github.com/Rigellute/spotify-tui
mod info;
mod kube_resources;

use super::app::{self, App};
use anyhow::anyhow;
use kube::Client;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
pub enum IoEvent {
  GetCLIInfo,
  GetKubeConfig,
  GetNodes,
  GetTopNodes,
  GetNamespaces,
  GetPods,
  GetServices,
  RefreshClient,
}

pub async fn get_client() -> kube::Result<Client> {
  Client::try_default().await
}

#[derive(Clone)]
pub struct Network<'a> {
  pub client: Client,
  pub app: &'a Arc<Mutex<App>>,
}

static UNKNOWN: &'static str = "Unknown";
static NOT_FOUND: &'static str = "Not found";

impl<'a> Network<'a> {
  pub fn new(client: Client, app: &'a Arc<Mutex<App>>) -> Self {
    Network { client, app }
  }

  pub async fn refresh_client(&mut self) {
    // TODO find a better way to do this
    match get_client().await {
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
      IoEvent::GetCLIInfo => {
        self.get_cli_info().await;
      }
      IoEvent::GetKubeConfig => {
        self.get_kube_config().await;
      }
      IoEvent::GetNodes => {
        self.get_nodes().await;
      }
      IoEvent::GetTopNodes => {
        self.get_top_node().await;
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
    };

    let mut app = self.app.lock().await;
    app.is_loading = false;
  }

  async fn handle_error(&mut self, e: anyhow::Error) {
    let mut app = self.app.lock().await;
    app.handle_error(e);
  }
}
