use k8s_openapi::api::core::v1::Namespace;

use super::{models::KubeResource, utils::UNKNOWN};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct KubeNs {
  pub name: String,
  pub status: String,
  k8s_obj: Namespace,
}

impl KubeResource<Namespace> for KubeNs {
  fn from_api(ns: &Namespace) -> Self {
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
      k8s_obj: ns.to_owned(),
    }
  }

  fn get_k8s_obj(&self) -> &Namespace {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use crate::app::test_utils::convert_resource_from_file;

  use super::*;

  #[test]
  fn test_namespace_from_api() {
    let (nss, ns_list): (Vec<KubeNs>, Vec<_>) = convert_resource_from_file("ns");

    assert_eq!(nss.len(), 4);
    assert_eq!(
      nss[0],
      KubeNs {
        name: "default".into(),
        status: "Active".into(),
        k8s_obj: ns_list[0].clone()
      }
    );
  }
}
