use std::{collections::HashSet, sync::Arc, time::Duration};

use anyhow::anyhow;
use futures::AsyncBufReadExt;
use k8s_openapi::api::core::v1::Pod;
use kube::{
  api::{ListParams, LogParams},
  Api, Client,
};
use log::{debug, error, info, warn};
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
const MAX_AGGREGATE_PODS: usize = 20;

#[derive(Debug, Eq, PartialEq)]
pub enum IoStreamEvent {
  RefreshClient,
  GetPodLogs(bool),
  GetAggregateLogs { namespace: String, selector: String },
  GetPodAllContainerLogs,
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
      IoStreamEvent::GetAggregateLogs {
        namespace,
        selector,
      } => {
        self.stream_aggregate_logs(&namespace, &selector).await;
      }
      IoStreamEvent::GetPodAllContainerLogs => {
        self.stream_pod_all_container_logs().await;
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

      // Ask Kubernetes for a small overlap window so reconnects only refetch recent logs.
      since_seconds = Some(RECONNECT_OVERLAP_SECS);

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

  /// Stream logs from all containers of the selected pod concurrently.
  pub async fn stream_pod_all_container_logs(&self) {
    let (namespace, pod_name, container_names, cancel_rx) = {
      let app = self.app.lock().await;
      let pod = app.data.pods.get_selected_item_copy();
      let ns = pod
        .as_ref()
        .map(|p| p.namespace.clone())
        .unwrap_or_else(|| std::env::var("NAMESPACE").unwrap_or_else(|_| "default".into()));
      let name = pod.as_ref().map(|p| p.name.clone()).unwrap_or_default();
      let containers: Vec<String> = pod
        .as_ref()
        .map(|p| p.containers.iter().map(|c| c.name.clone()).collect())
        .unwrap_or_default();
      let rx = app.new_log_cancel_rx();
      (ns, name, containers, rx)
    };

    if pod_name.is_empty() || container_names.is_empty() {
      return;
    }

    // Single container — delegate to the standard single-container stream
    if container_names.len() == 1 {
      {
        let mut app = self.app.lock().await;
        app.data.selected.container = Some(container_names[0].clone());
      }
      self.stream_container_logs(true).await;
      return;
    }

    {
      let mut app = self.app.lock().await;
      app.is_streaming = true;
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(256);

    let mut join_set = tokio::task::JoinSet::new();
    for cont_name in container_names {
      let client = self.client.clone();
      let ns = namespace.clone();
      let pod = pod_name.clone();
      let tx = tx.clone();
      let cancel_rx = cancel_rx.clone();

      join_set.spawn(async move {
        stream_single_pod_for_aggregate(
          client,
          ns,
          pod,
          cont_name.clone(),
          cont_name,
          tx,
          cancel_rx,
        )
        .await;
      });
    }

    drop(tx);

    let mut batch: Vec<String> = Vec::with_capacity(BATCH_SIZE);
    let mut last_flush = Instant::now();
    let mut cancel_rx_collector = cancel_rx.clone();

    loop {
      let flush_deadline =
        tokio::time::sleep_until(last_flush + Duration::from_millis(BATCH_FLUSH_MS));

      tokio::select! {
        _ = cancel_rx_collector.changed() => {
          if *cancel_rx_collector.borrow() {
            if !batch.is_empty() {
              let mut app = self.app.lock().await;
              app.data.logs.add_records(batch);
            }
            break;
          }
        }
        line = rx.recv() => {
          match line {
            Some(line) => {
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
            None => {
              if !batch.is_empty() {
                let mut app = self.app.lock().await;
                app.data.logs.add_records(batch);
              }
              break;
            }
          }
        }
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

    join_set.shutdown().await;
    let mut app = self.app.lock().await;
    app.is_streaming = false;
  }

  /// Stream logs from all pods matching a label selector concurrently.
  /// Lines are prefixed with the pod name for disambiguation.
  pub async fn stream_aggregate_logs(&self, namespace: &str, selector: &str) {
    let cancel_rx = {
      let app = self.app.lock().await;
      app.new_log_cancel_rx()
    };

    // Fetch pods matching the selector
    let api: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
    let lp = ListParams::default().labels(selector);
    let pods = match api.list(&lp).await {
      Ok(list) => list.items,
      Err(e) => {
        self
          .handle_error(anyhow!(
            "Failed to list pods for selector '{}': {}",
            selector,
            e
          ))
          .await;
        return;
      }
    };

    if pods.is_empty() {
      let mut app = self.app.lock().await;
      app
        .data
        .logs
        .add_records(vec!["[kdash] No pods found for this resource".to_string()]);
      return;
    }

    let total_pods = pods.len();
    let pods: Vec<Pod> = pods.into_iter().take(MAX_AGGREGATE_PODS).collect();
    let streaming_count = pods.len();

    if total_pods > MAX_AGGREGATE_PODS {
      let mut app = self.app.lock().await;
      app.data.logs.add_records(vec![format!(
        "[kdash] Showing logs from {} of {} pods",
        MAX_AGGREGATE_PODS, total_pods
      )]);
    }

    {
      let mut app = self.app.lock().await;
      app.is_streaming = true;
    }

    let pod_info = collect_pod_container_info(&pods);

    info!(
      "Starting aggregate log stream for {} container streams (selector: {})",
      pod_info.len(),
      selector
    );

    // Use a channel to collect lines from all pod streams
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(256);

    // Spawn a task per container
    let mut join_set = tokio::task::JoinSet::new();
    for (pod_name, cont_name, prefix) in pod_info {
      let client = self.client.clone();
      let ns = namespace.to_string();
      let tx = tx.clone();
      let cancel_rx = cancel_rx.clone();

      join_set.spawn(async move {
        stream_single_pod_for_aggregate(client, ns, pod_name, cont_name, prefix, tx, cancel_rx)
          .await;
      });
    }

    // Drop our copy of tx so rx will close when all tasks finish
    drop(tx);

    // Collector loop: read from channel, batch-flush to app.data.logs
    let mut batch: Vec<String> = Vec::with_capacity(BATCH_SIZE);
    let mut last_flush = Instant::now();
    let mut cancel_rx_collector = cancel_rx.clone();

    loop {
      let flush_deadline =
        tokio::time::sleep_until(last_flush + Duration::from_millis(BATCH_FLUSH_MS));

      tokio::select! {
        _ = cancel_rx_collector.changed() => {
          if *cancel_rx_collector.borrow() {
            if !batch.is_empty() {
              let mut app = self.app.lock().await;
              app.data.logs.add_records(batch);
            }
            debug!("Aggregate log stream cancelled (selector: {})", selector);
            break;
          }
        }
        line = rx.recv() => {
          match line {
            Some(line) => {
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
            None => {
              // All senders dropped — all pod streams finished
              if !batch.is_empty() {
                let mut app = self.app.lock().await;
                app.data.logs.add_records(batch);
              }
              break;
            }
          }
        }
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

    // Wait for all spawned tasks to finish
    join_set.abort_all();

    let mut app = self.app.lock().await;
    app.is_streaming = false;
    info!(
      "Aggregate log stream ended for {} pods (selector: {})",
      streaming_count, selector
    );
  }
}

/// Extract a short pod name suffix for log prefixes.
/// e.g., "myapp-deploy-abc123" → "abc123"
fn short_pod_name(name: &str) -> String {
  name.rsplit('-').next().unwrap_or(name).to_string()
}

/// Collect (pod_name, container_name, log_line_prefix) for every container in every pod.
/// When a pod has multiple containers, the prefix includes both pod suffix and container name.
fn collect_pod_container_info(pods: &[Pod]) -> Vec<(String, String, String)> {
  pods
    .iter()
    .filter_map(|pod| {
      let name = pod.metadata.name.clone().unwrap_or_default();
      let containers: Vec<String> = pod
        .spec
        .as_ref()
        .map(|s| s.containers.iter().map(|c| c.name.clone()).collect())
        .unwrap_or_default();
      if name.is_empty() || containers.is_empty() {
        None
      } else {
        let short = short_pod_name(&name);
        let multi = containers.len() > 1;
        Some(
          containers
            .into_iter()
            .map(move |c| {
              let prefix = if multi {
                format!("{}:{}", short, c)
              } else {
                short.clone()
              };
              (name.clone(), c, prefix)
            })
            .collect::<Vec<_>>(),
        )
      }
    })
    .flatten()
    .collect()
}

/// Stream logs from a single pod, prefixing each line and sending to the channel.
async fn stream_single_pod_for_aggregate(
  client: Client,
  namespace: String,
  pod_name: String,
  cont_name: String,
  short_name: String,
  tx: tokio::sync::mpsc::Sender<String>,
  cancel_rx: tokio::sync::watch::Receiver<bool>,
) {
  let api: Api<Pod> = Api::namespaced(client, &namespace);
  let lp = LogParams {
    container: Some(cont_name.clone()),
    follow: true,
    previous: false,
    tail_lines: Some(INITIAL_TAIL_LINES),
    ..Default::default()
  };

  match api.log_stream(&pod_name, &lp).await {
    Ok(logs) => {
      let mut lines_stream = logs.lines();
      let mut cancel_rx = cancel_rx;

      loop {
        tokio::select! {
          _ = cancel_rx.changed() => {
            if *cancel_rx.borrow() {
              return;
            }
          }
          line = lines_stream.next() => {
            match line {
              Some(Ok(line)) => {
                let line = line.trim().to_string();
                if !line.is_empty() {
                  let prefixed = format!("[{}] {}", short_name, line);
                  if tx.send(prefixed).await.is_err() {
                    return; // Receiver dropped
                  }
                }
              }
              Some(Err(e)) => {
                warn!("Aggregate stream error for {}/{}: {}", pod_name, cont_name, e);
                return;
              }
              None => {
                debug!("Aggregate stream ended for {}/{}", pod_name, cont_name);
                return;
              }
            }
          }
        }
      }
    }
    Err(e) => {
      warn!(
        "Failed to open aggregate log stream for {}/{}: {}",
        pod_name, cont_name, e
      );
      let _ = tx
        .send(format!(
          "[{}] Error: failed to stream logs: {}",
          short_name, e
        ))
        .await;
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_short_pod_name_extracts_suffix() {
    assert_eq!(short_pod_name("myapp-deploy-abc123"), "abc123");
  }

  #[test]
  fn test_short_pod_name_single_segment() {
    assert_eq!(short_pod_name("mypod"), "mypod");
  }

  #[test]
  fn test_short_pod_name_empty() {
    assert_eq!(short_pod_name(""), "");
  }

  #[test]
  fn test_short_pod_name_trailing_dash() {
    assert_eq!(short_pod_name("nginx-"), "");
  }

  #[test]
  fn test_io_stream_event_variants() {
    let event = IoStreamEvent::GetAggregateLogs {
      namespace: "default".into(),
      selector: "app=nginx".into(),
    };
    assert_eq!(
      event,
      IoStreamEvent::GetAggregateLogs {
        namespace: "default".into(),
        selector: "app=nginx".into(),
      }
    );
  }

  fn make_pod(name: &str, containers: &[&str]) -> Pod {
    use k8s_openapi::api::core::v1::{Container, PodSpec};
    Pod {
      metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
        name: Some(name.into()),
        ..Default::default()
      },
      spec: Some(PodSpec {
        containers: containers
          .iter()
          .map(|c| Container {
            name: c.to_string(),
            ..Default::default()
          })
          .collect(),
        ..Default::default()
      }),
      ..Default::default()
    }
  }

  #[test]
  fn test_collect_pod_container_info_single_container() {
    let pods = vec![make_pod("myapp-deploy-abc123", &["web"])];
    let info = collect_pod_container_info(&pods);

    assert_eq!(info.len(), 1);
    assert_eq!(info[0].0, "myapp-deploy-abc123");
    assert_eq!(info[0].1, "web");
    // single container: prefix is just the short pod name
    assert_eq!(info[0].2, "abc123");
  }

  #[test]
  fn test_collect_pod_container_info_multi_container() {
    let pods = vec![make_pod("myapp-deploy-abc123", &["web", "sidecar"])];
    let info = collect_pod_container_info(&pods);

    assert_eq!(info.len(), 2);
    assert_eq!(
      info[0],
      (
        "myapp-deploy-abc123".into(),
        "web".into(),
        "abc123:web".into()
      )
    );
    assert_eq!(
      info[1],
      (
        "myapp-deploy-abc123".into(),
        "sidecar".into(),
        "abc123:sidecar".into()
      )
    );
  }

  #[test]
  fn test_collect_pod_container_info_multiple_pods() {
    let pods = vec![
      make_pod("app-abc12", &["main"]),
      make_pod("app-def34", &["main", "logging"]),
    ];
    let info = collect_pod_container_info(&pods);

    assert_eq!(info.len(), 3);
    // first pod: single container
    assert_eq!(info[0], ("app-abc12".into(), "main".into(), "abc12".into()));
    // second pod: multi container
    assert_eq!(
      info[1],
      ("app-def34".into(), "main".into(), "def34:main".into())
    );
    assert_eq!(
      info[2],
      ("app-def34".into(), "logging".into(), "def34:logging".into())
    );
  }

  #[test]
  fn test_collect_pod_container_info_skips_empty_name() {
    let pods = vec![make_pod("", &["web"])];
    let info = collect_pod_container_info(&pods);
    assert!(info.is_empty());
  }

  #[test]
  fn test_collect_pod_container_info_skips_no_containers() {
    let pods = vec![make_pod("myapp-abc123", &[])];
    let info = collect_pod_container_info(&pods);
    assert!(info.is_empty());
  }

  #[test]
  fn test_collect_pod_container_info_skips_no_spec() {
    let pod = Pod {
      metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
        name: Some("myapp".into()),
        ..Default::default()
      },
      spec: None,
      ..Default::default()
    };
    let info = collect_pod_container_info(&[pod]);
    assert!(info.is_empty());
  }
}
