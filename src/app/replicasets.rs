use k8s_openapi::{api::apps::v1::ReplicaSet, chrono::Utc};

use super::{models::KubeResource, utils};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeReplicaSet {
  pub name: String,
  pub namespace: String,
  pub desired: i32,
  pub current: i32,
  pub ready: i32,
  pub age: String,
  k8s_obj: ReplicaSet,
}

impl From<ReplicaSet> for KubeReplicaSet {
  fn from(rps: ReplicaSet) -> Self {
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
      k8s_obj: utils::sanitize_obj(rps),
    }
  }
}

impl KubeResource<ReplicaSet> for KubeReplicaSet {
  fn get_k8s_obj(&self) -> &ReplicaSet {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_utils::*;

  #[test]
  fn test_replica_sets_from_api() {
    let (rpls, rpls_list): (Vec<KubeReplicaSet>, Vec<_>) =
      convert_resource_from_file("replicasets");

    assert_eq!(rpls.len(), 4);
    assert_eq!(
      rpls[0],
      KubeReplicaSet {
        name: "metrics-server-86cbb8457f".into(),
        namespace: "kube-system".into(),
        age: utils::to_age(Some(&get_time("2021-05-10T21:48:19Z")), Utc::now()),
        k8s_obj: rpls_list[0].clone(),
        desired: 1,
        current: 1,
        ready: 1,
      }
    );
  }
}
