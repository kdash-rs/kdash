//! Active `kubectl port-forward` children tracked by the app.
//!
//! KDash shells out to `kubectl port-forward` (already a hard dependency) and
//! holds each background [`Child`] so it can be listed and stopped. Readiness is
//! detected by the network-stream task reading kubectl's stdout; see
//! `network::stream`.

use tokio::process::Child;

/// Lifecycle of a single forward.
#[derive(Debug)]
pub enum PortForwardStatus {
  /// Spawned; waiting for kubectl's "Forwarding from …" line.
  Starting,
  /// kubectl reported the local listener is up.
  Active,
  /// kubectl exited before/while forwarding; carries a short reason.
  Failed(String),
}

impl PortForwardStatus {
  pub fn label(&self) -> String {
    match self {
      PortForwardStatus::Starting => "starting".to_owned(),
      PortForwardStatus::Active => "active".to_owned(),
      PortForwardStatus::Failed(reason) => format!("failed: {reason}"),
    }
  }
}

/// A tracked `kubectl port-forward` child plus the metadata needed to render and
/// stop it. Not `Clone`/`Eq` because it owns the [`Child`] handle.
#[derive(Debug)]
pub struct PortForward {
  /// Stable id used to target stop/status updates from the network thread.
  pub id: u64,
  /// kubectl resource type (`pods` / `services`).
  pub kind: String,
  pub namespace: String,
  pub name: String,
  pub local_port: u16,
  pub remote_port: u16,
  pub status: PortForwardStatus,
  /// The running child. `take`n when the forward is stopped so the owning
  /// runtime can kill+reap it.
  pub child: Option<Child>,
}
