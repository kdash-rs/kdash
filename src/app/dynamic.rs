use k8s_openapi::chrono::Utc;
use kube::{
  core::DynamicObject,
  discovery::{ApiResource, Scope},
  ResourceExt,
};

use crate::app::models::KubeResource;

use super::utils;

#[derive(Clone, Debug)]
pub struct KubeDynamicKind {
  pub name: String,
  pub group: String,
  pub version: String,
  pub api_version: String,
  pub kind: String,
  pub scope: Scope,
  pub api_resource: ApiResource,
}

impl KubeDynamicKind {
  pub fn new(ar: ApiResource, scope: Scope) -> Self {
    KubeDynamicKind {
      api_resource: ar.clone(),
      name: ar.plural,
      group: ar.group,
      version: ar.version,
      api_version: ar.api_version,
      kind: ar.kind,
      scope,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct KubeDynamicResource {
  pub name: String,
  pub namespace: Option<String>,
  pub age: String,
  k8s_obj: DynamicObject,
}

impl From<DynamicObject> for KubeDynamicResource {
  fn from(item: DynamicObject) -> Self {
    KubeDynamicResource {
      name: item.name_any(),
      namespace: item.clone().metadata.namespace,
      age: utils::to_age(item.metadata.creation_timestamp.as_ref(), Utc::now()),
      k8s_obj: item,
    }
  }
}

impl KubeResource<DynamicObject> for KubeDynamicResource {
  fn get_k8s_obj(&self) -> &DynamicObject {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_utils::*;

  #[test]
  fn test_dynamic_resource_from_api() {
    let (dynamic_resource, res_list): (Vec<KubeDynamicResource>, Vec<_>) =
      convert_resource_from_file("dynamic_resource");

    assert_eq!(dynamic_resource.len(), 6);
    assert_eq!(
      dynamic_resource[0],
      KubeDynamicResource {
        name: "consul-5bb65dd4c8".into(),
        namespace: Some("jhipster".into()),
        age: utils::to_age(Some(&get_time("2023-06-30T17:27:23Z")), Utc::now()),
        k8s_obj: res_list[0].clone(),
      }
    );
  }
}
