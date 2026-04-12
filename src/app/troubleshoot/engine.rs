//! Generic evaluation engine for troubleshoot checks.

use super::types::{Diagnostic, DisplayFinding, HealthCheck};

/// Runs every check against every resource, collecting findings.
pub fn evaluate_resource<T: Diagnostic>(
  resources: &[T],
  checks: &[HealthCheck<T>],
) -> Vec<DisplayFinding> {
  resources
    .iter()
    .flat_map(|res| {
      checks.iter().filter_map(|check| {
        check(res).map(|(severity, raw)| DisplayFinding {
          severity,
          reason: raw.reason,
          resource_kind: res.resource_kind(),
          namespace: res.namespace().map(str::to_string),
          resource_name: res.name().to_string(),
          message: raw.message,
          age: res.age().to_string(),
        })
      })
    })
    .collect()
}
