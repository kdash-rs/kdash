use k8s_openapi::{api::apps::v1::StatefulSet, chrono::Utc};

use super::{models::KubeResource, utils};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeStatefulSet {
  pub name: String,
  pub namespace: String,
  pub ready: String,
  pub service: String,
  pub age: String,
  k8s_obj: StatefulSet,
}

impl KubeResource<StatefulSet> for KubeStatefulSet {
  fn from_api(stfs: &StatefulSet) -> Self {
    let ready = match &stfs.status {
      Some(s) => format!("{}/{}", s.ready_replicas.unwrap_or_default(), s.replicas),
      _ => "".into(),
    };

    KubeStatefulSet {
      name: stfs.metadata.name.clone().unwrap_or_default(),
      namespace: stfs.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(stfs.metadata.creation_timestamp.as_ref(), Utc::now()),
      service: stfs
        .spec
        .as_ref()
        .map_or("n/a".into(), |spec| spec.service_name.to_owned()),
      ready,
      k8s_obj: stfs.to_owned(),
    }
  }

  fn get_k8s_obj(&self) -> &StatefulSet {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use super::super::test_utils::{convert_resource_from_file, get_time};
  use super::*;

  #[test]
  fn test_stateful_sets_from_api() {
    let (stfs, stfs_list): (Vec<KubeStatefulSet>, Vec<_>) = convert_resource_from_file("stfs");

    assert_eq!(stfs.len(), 1);
    assert_eq!(
      stfs[0],
      KubeStatefulSet {
        name: "web".into(),
        namespace: "default".into(),
        age: utils::to_age(Some(&get_time("2021-04-25T14:23:47Z")), Utc::now()),
        k8s_obj: stfs_list[0].clone(),
        service: "nginx".into(),
        ready: "2/2".into(),
      }
    );
  }
}
