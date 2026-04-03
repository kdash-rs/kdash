use std::{collections::HashSet, sync::Arc, time::Duration};

use anyhow::anyhow;
use futures::AsyncBufReadExt;
use k8s_openapi::api::core::v1::Pod;
use kube::{api::LogParams, Api, Client};
use log::{debug, error, warn};
use tokio::{sync::Mutex, time::Instant};
use tokio_stream::StreamExt;

use super::refresh_kube_config;
use crate::app::App;

const INITIAL_TAIL_LINES: i64 = 100;
const BATCH_SIZE: usize = 50;
const BATCH_FLUSH_MS: u64 = 100;
const RECONNECT_OVERLAP_SECS: i64 = 5;
const MAX_RECONNECT_ATTEMPTS: u32 = 10;
const DEDUP_WINDOW: usize = 50;

#[derive(Debug, Eq, PartialEq)]
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
    let context = {
      let app = self.app.lock().await;
      app.data.selected.context.clone()
    };
    match refresh_kube_config(&context).await {
      Ok(client) => {
        self.client = client;
      }
      Err(e) => {
        self
          .handle_error(anyhow!("Failed to refresh client. {:}", e))
          .await
      }
    }
  }

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
    app.loading_complete();
  }

  async fn handle_error(&self, e: anyhow::Error) {
    error!("{:?}", e);
    let mut app = self.app.lock().await;
    app.handle_error(e);
  }

  pub async fn stream_container_logs(&self, tail: bool) {
    let (namespace, pod_name, cont_name, cancel_rx) = {
      let app = self.app.lock().await;
      let ns = app
        .data
        .pods
        .get_selected_item_copy()
        .map(|p| p.namespace)
        .unwrap_or_else(|| std::env::var("NAMESPACE").unwrap_or_else(|_| "default".into()));
      let pod = app
        .data
        .pods
        .get_selected_item_copy()
        .map(|p| p.name)
        .unwrap_or_default();
      let cont = app.data.selected.container.clone().unwrap_or_default();
      let rx = app.new_log_cancel_rx();
      (ns, pod, cont, rx)
    };

    if pod_name.is_empty() || cont_name.is_empty() {
      return;
    }

    {
      let mut app = self.app.lock().await;
      app.is_streaming = true;
    }

    let api: Api<Pod> = Api::namespaced(self.client.clone(), &namespace);
    let mut since_seconds: Option<i64> = None;
    let mut reconnect_count: u32 = 0;
    let stream_start = Instant::now();

    loop {
      let lp = LogParams {
        container: Some(cont_name.clone()),
        follow: true,
        previous: false,
        tail_lines: if since_seconds.is_none() && tail {
          Some(INITIAL_TAIL_LINES)
        } else {
          None
        },
        since_seconds,
        ..Default::default()
      };

      match api.log_stream(&pod_name, &lp).await {
        Ok(logs) => {
          reconnect_count = 0;
          let mut lines_stream = logs.lines();
          let mut batch: Vec<String> = Vec::with_capacity(BATCH_SIZE);
          let mut last_flush = Instant::now();
          let mut cancel_rx = cancel_rx.clone();

          // Build dedup set from existing records
          let dedup_set: HashSet<String> = if since_seconds.is_some() {
            let app = self.app.lock().await;
            app
              .data
              .logs
              .last_n_records(DEDUP_WINDOW)
              .into_iter()
              .map(|s| s.to_string())
              .collect()
          } else {
            HashSet::new()
          };

          loop {
            let flush_deadline =
              tokio::time::sleep_until(last_flush + Duration::from_millis(BATCH_FLUSH_MS));

            tokio::select! {
              // Cancellation signal
              _ = cancel_rx.changed() => {
                if *cancel_rx.borrow() {
                  // Flush remaining batch before exiting
                  if !batch.is_empty() {
                    let mut app = self.app.lock().await;
                    app.data.logs.add_records(batch);
                  }
                  debug!("Log stream cancelled for {}/{}", pod_name, cont_name);
                  let mut app = self.app.lock().await;
                  app.is_streaming = false;
                  return;
                }
              }
              // Next log line
              line = lines_stream.next() => {
                match line {
                  Some(Ok(line)) => {
                    let line = line.trim().to_string();
                    if !line.is_empty() {
                      // Skip duplicates on reconnect
                      if since_seconds.is_some() && dedup_set.contains(&line) {
                        continue;
                      }
                      batch.push(line);

                      if batch.len() >= BATCH_SIZE {
                        let mut app = self.app.lock().await;
                        app.data.logs.add_records(std::mem::replace(
                          &mut batch,
                          Vec::with_capacity(BATCH_SIZE),
                        ));
                        last_flush = Instant::now();
                      }
                    }
                  }
                  Some(Err(e)) => {
                    warn!("Log stream read error for {}/{}: {}", pod_name, cont_name, e);
                    break; // Break inner loop to reconnect
                  }
                  None => {
                    debug!("Log stream ended for {}/{}", pod_name, cont_name);
                    break; // Break inner loop to reconnect
                  }
                }
              }
              // Flush timer
              _ = flush_deadline => {
                if !batch.is_empty() {
                  let mut app = self.app.lock().await;
                  app.data.logs.add_records(std::mem::replace(
                    &mut batch,
                    Vec::with_capacity(BATCH_SIZE),
                  ));
                  last_flush = Instant::now();
                }
              }
            }
          }

          // Flush any remaining lines after inner loop break
          if !batch.is_empty() {
            let mut app = self.app.lock().await;
            app.data.logs.add_records(batch);
          }
        }
        Err(e) => {
          warn!(
            "Failed to open log stream for {}/{}: {}",
            pod_name, cont_name, e
          );
          reconnect_count += 1;
          if reconnect_count >= MAX_RECONNECT_ATTEMPTS {
            self
              .handle_error(anyhow!(
                "Failed to stream logs after {} attempts: {}",
                MAX_RECONNECT_ATTEMPTS,
                e
              ))
              .await;
            break;
          }
        }
      }

      // Check cancellation before reconnecting
      if *cancel_rx.borrow() {
        break;
      }

      // Calculate since_seconds for reconnection with overlap
      let elapsed = stream_start.elapsed().as_secs() as i64;
      since_seconds = Some((elapsed + RECONNECT_OVERLAP_SECS).max(RECONNECT_OVERLAP_SECS));

      // Backoff before reconnecting
      let backoff = Duration::from_millis(500 * (reconnect_count as u64).min(4));
      debug!(
        "Reconnecting log stream for {}/{} (attempt {}, backoff {:?})",
        pod_name, cont_name, reconnect_count, backoff
      );
      tokio::time::sleep(backoff).await;
    }

    let mut app = self.app.lock().await;
    app.is_streaming = false;
  }
}
