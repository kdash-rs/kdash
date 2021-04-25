use k8s_openapi::{api::apps::v1::ReplicaSet, chrono::Utc};

use super::utils;

#[derive(Clone)]
pub struct KubeReplicaSet {
  pub name: String,
  pub namespace: String,
  pub desired: i32,
  pub current: i32,
  pub ready: i32,
  pub age: String,
}

impl KubeReplicaSet {
  pub fn from_api(rp: &ReplicaSet) -> Self {
    KubeReplicaSet {
      name: rp.metadata.name.clone().unwrap_or_default(),
      namespace: rp.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(rp.metadata.creation_timestamp.as_ref(), Utc::now()),
      desired: rp
        .spec
        .as_ref()
        .map_or(0, |s| s.replicas.unwrap_or_default()),
      current: rp.status.as_ref().map_or(0, |s| s.replicas),
      ready: rp
        .status
        .as_ref()
        .map_or(0, |s| s.ready_replicas.unwrap_or_default()),
    }
  }
}
