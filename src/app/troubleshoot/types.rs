//! Core types for the troubleshoot subsystem.

use std::cmp::Ordering;

use strum::Display;

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

#[derive(Clone, Copy, Debug, Display, Eq, PartialEq)]
pub enum ResourceKind {
  Pod,
  #[strum(serialize = "PVC")]
  Pvc,
  ReplicaSet,
}

impl ResourceKind {
  pub fn describe_kind(&self) -> &'static str {
    match self {
      ResourceKind::Pod => "pod",
      ResourceKind::Pvc => "persistentvolumeclaim",
      ResourceKind::ReplicaSet => "replicaset",
    }
  }
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

  pub fn describe_target(&self) -> (&str, &str, Option<&str>) {
    (
      self.resource_kind.describe_kind(),
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
