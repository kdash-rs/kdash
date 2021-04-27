use std::collections::BTreeMap;

use k8s_openapi::{api::core::v1::ConfigMap, chrono::Utc};

use super::utils;

#[derive(Clone)]
pub struct KubeConfigMap {
  pub name: String,
  pub namespace: String,
  pub data: BTreeMap<String, String>,
  pub age: String,
}

impl KubeConfigMap {
  pub fn from_api(cm: &ConfigMap) -> Self {
    let data = match cm.data.as_ref() {
      Some(data) => data.to_owned(),
      _ => BTreeMap::new(),
    };

    KubeConfigMap {
      name: cm.metadata.name.clone().unwrap_or_default(),
      namespace: cm.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(cm.metadata.creation_timestamp.as_ref(), Utc::now()),
      data,
    }
  }
}
