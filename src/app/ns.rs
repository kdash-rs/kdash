use k8s_openapi::api::core::v1::Namespace;

use super::utils::UNKNOWN;

#[derive(Clone)]
pub struct KubeNs {
  pub name: String,
  pub status: String,
}

impl KubeNs {
  pub fn from_api(ns: &Namespace) -> Self {
    let status = match &ns.status {
      Some(stat) => match &stat.phase {
        Some(phase) => phase.clone(),
        _ => UNKNOWN.into(),
      },
      _ => UNKNOWN.into(),
    };

    KubeNs {
      name: ns.metadata.name.clone().unwrap_or_default(),
      status,
    }
  }
}
