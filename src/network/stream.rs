use super::app::{ActiveBlock, App};
use super::get_client;

use anyhow::anyhow;
use k8s_openapi::api::core::v1::Pod;
use kube::Client;
use kube::{api::LogParams, Api};
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tokio_stream::StreamExt;

#[derive(Debug)]
pub enum IoStreamEvent {
  RefreshClient,
  GetPodLogs(bool),
}

#[derive(Clone)]
pub struct NetworkStream<'a> {
  pub client: Client,
  pub app: &'a Arc<Mutex<App>>,
}

impl<'a> NetworkStream<'a> {
  pub fn new(client: Client, app: &'a Arc<Mutex<App>>) -> Self {
    NetworkStream { client, app }
  }

  pub async fn refresh_client(&mut self) {
    match get_client().await {
      Ok(client) => {
        self.client = client;
      }
      Err(e) => self.handle_error(anyhow!(e)).await,
    };
  }

  #[allow(clippy::cognitive_complexity)]
  pub async fn handle_network_stream_event(&mut self, io_event: IoStreamEvent) {
    match io_event {
      IoStreamEvent::RefreshClient => {
        self.refresh_client().await;
      }
      IoStreamEvent::GetPodLogs(tail) => {
        self.stream_container_logs(tail).await;
      }
    };

    let mut app = self.app.lock().await;
    app.is_loading = false;
  }

  async fn handle_error(&self, e: anyhow::Error) {
    let mut app = self.app.lock().await;
    app.handle_error(e);
  }

  pub async fn stream_container_logs(&self, tail: bool) {
    let (namespace, pod_name, cont_name) = {
      let app = self.app.lock().await;
      if let Some(p) = app.data.pods.get_selected_item() {
        (
          p.namespace,
          p.name,
          p.containers
            .get_selected_item()
            .map_or("".to_string(), |c| c.name),
        )
      } else {
        (
          std::env::var("NAMESPACE").unwrap_or_else(|_| "default".into()),
          "".to_string(),
          "".to_string(),
        )
      }
    };

    if pod_name.is_empty() {
      return;
    }
    let pods: Api<Pod> = Api::namespaced(self.client.clone(), &namespace);
    let lp = LogParams {
      container: Some(cont_name.clone()),
      follow: true,
      previous: false,
      //   timestamps: true,
      tail_lines: if tail { Some(10) } else { Some(0) },
      ..Default::default()
    };

    {
      let mut app = self.app.lock().await;
      app.is_streaming = true;
    }

    // TODO investigate why this stops working at times
    match pods.log_stream(&pod_name, &lp).await {
      Ok(logs) => {
        // set a timeout so we dont wait for next item and block the thread
        let logs = logs.timeout(Duration::from_secs(5));
        tokio::pin!(logs);

        #[allow(clippy::eval_order_dependence)]
        while let (true, Ok(Some(Ok(line)))) = (
          {
            let app = self.app.lock().await;
            app.get_current_route().active_block == ActiveBlock::Logs
              || app.data.logs.id == cont_name
          },
          logs.try_next().await,
        ) {
          let line = String::from_utf8_lossy(&line).trim().to_string();
          if !line.is_empty() {
            let mut app = self.app.lock().await;
            app.data.logs.add_record(line);
          }
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    };

    let mut app = self.app.lock().await;
    app.is_streaming = false;
  }
}
