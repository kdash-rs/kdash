// adapted from https://github.com/Rigellute/spotify-tui
use crate::app::App;
use crate::config::ClientConfig;
use anyhow::anyhow;
use kube::{api::ListParams, Api, Client};
use serde_json::{map::Map, Value};
use std::{
  sync::Arc,
  time::{Duration, Instant, SystemTime},
};
use tokio::sync::Mutex;
use tokio::try_join;

#[derive(Debug)]
pub enum IoEvent {
  GetPods,
}

pub async fn get_client() -> Client {
  let client = Client::try_default().await.unwrap();
  client
}

#[derive(Clone)]
pub struct Network<'a> {
  pub client: Client,
  pub client_config: ClientConfig,
  pub app: &'a Arc<Mutex<App>>,
}

impl<'a> Network<'a> {
  pub fn new(client: Client, client_config: ClientConfig, app: &'a Arc<Mutex<App>>) -> Self {
    Network {
      client,
      client_config,
      app,
    }
  }

  #[allow(clippy::cognitive_complexity)]
  pub async fn handle_network_event(&mut self, io_event: IoEvent) {
    match io_event {
      _ => (),
      IoEvent::GetPods => {
        self.get_pods().await;
      }
    };

    let mut app = self.app.lock().await;
    app.is_loading = false;
  }

  async fn handle_error(&mut self, e: anyhow::Error) {
    let mut app = self.app.lock().await;
    app.handle_error(e);
  }

  async fn get_pods(&mut self) {
    // match self.client.current_user().await {
    //   Ok(user) => {
    //     let mut app = self.app.lock().await;
    //     app.user = Some(user);
    //   }
    //   Err(e) => {
    //     self.handle_error(anyhow!(e)).await;
    //   }
    // }
  }
}
