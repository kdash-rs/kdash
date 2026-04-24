//! Core types for the troubleshoot subsystem.

use std::cmp::Ordering;

use strum::{Display, EnumIter};

use crate::app::models::Named;

/// Severity-tagged finding.
///
/// `Ord` is implemented explicitly so that `Error` sorts first, then `Warn`,
/// then `Info`. This ordering is **independent** of the declaration order of
/// variants — reordering them will not silently change sort behaviour.
#[derive(Clone, Copy, Debug, Display, Eq, PartialEq)]
pub enum Severity {
  Error,
  Warn,
  Info,
}

impl Severity {
  const fn rank(self) -> u8 {
    match self {
      Severity::Error => 0,
      Severity::Warn => 1,
      Severity::Info => 2,
    }
  }
}

impl Ord for Severity {
  fn cmp(&self, other: &Self) -> Ordering {
    self.rank().cmp(&other.rank())
  }
}

impl PartialOrd for Severity {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

// ---------------------------------------------------------------------------
// Display enums shared across resource-specific findings
// ---------------------------------------------------------------------------

/// Kubernetes resource kind for troubleshoot findings.
///
/// The `strum` serialization for each variant **must** be recognizable by `kubectl`.
/// This string is used both as the UI table label and as the `kind` argument in
/// `kubectl describe` commands.
#[derive(Clone, Copy, Debug, Display, EnumIter, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceKind {
  #[strum(serialize = "pod")]
  Pod,
  #[strum(serialize = "pvc")]
  Pvc,
  #[strum(serialize = "rs")]
  ReplicaSet,
}

// ---------------------------------------------------------------------------
// DisplayFinding — the concrete, type-erased row
// ---------------------------------------------------------------------------

/// Flattened UI row for a finding.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayFinding {
  pub severity: Severity,
  pub reason: String,
  pub resource_kind: ResourceKind,
  pub namespace: Option<String>,
  pub resource_name: String,
  pub message: String,
  pub age: String,
}

impl DisplayFinding {
  pub fn resource_ref(&self) -> String {
    match &self.namespace {
      Some(ns) if !ns.is_empty() => format!("{}/{}", ns, self.resource_name),
      _ => self.resource_name.clone(),
    }
  }

  pub fn describe_target(&self) -> (String, &str, Option<&str>) {
    (
      self.resource_kind.to_string(),
      self.resource_name.as_str(),
      self.namespace.as_deref(),
    )
  }
}

impl Named for DisplayFinding {
  fn get_name(&self) -> &String {
    &self.resource_name
  }
}
