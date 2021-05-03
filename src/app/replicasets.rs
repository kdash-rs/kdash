use k8s_openapi::{api::apps::v1::ReplicaSet, chrono::Utc};

use super::{models::ResourceToYaml, utils};

#[derive(Clone)]
pub struct KubeReplicaSet {
  pub name: String,
  pub namespace: String,
  pub desired: i32,
  pub current: i32,
  pub ready: i32,
  pub age: String,
  k8s_obj: ReplicaSet,
}

impl KubeReplicaSet {
  pub fn from_api(rps: &ReplicaSet) -> Self {
    let (current, ready) = match rps.status.as_ref() {
      Some(s) => (s.replicas, s.ready_replicas.unwrap_or_default()),
      _ => (0, 0),
    };

    KubeReplicaSet {
      name: rps.metadata.name.clone().unwrap_or_default(),
      namespace: rps.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(rps.metadata.creation_timestamp.as_ref(), Utc::now()),
      desired: rps
        .spec
        .as_ref()
        .map_or(0, |s| s.replicas.unwrap_or_default()),
      current,
      ready,
      k8s_obj: rps.to_owned(),
    }
  }
}

impl ResourceToYaml<ReplicaSet> for KubeReplicaSet {
  fn get_k8s_obj(&self) -> &ReplicaSet {
    &self.k8s_obj
  }
}
