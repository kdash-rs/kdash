use std::collections::BTreeMap;

use k8s_openapi::{api::core::v1::ReplicationController, chrono::Utc};

use super::{models::KubeResource, utils};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeReplicationController {
  pub name: String,
  pub namespace: String,
  pub desired: i32,
  pub current: i32,
  pub ready: i32,
  pub containers: String,
  pub images: String,
  pub selector: String,
  pub age: String,
  k8s_obj: ReplicationController,
}

impl KubeResource<ReplicationController> for KubeReplicationController {
  fn from_api(rplc: &ReplicationController) -> Self {
    let (current, ready) = match rplc.status.as_ref() {
      Some(s) => (s.replicas, s.ready_replicas.unwrap_or_default()),
      _ => (0, 0),
    };

    let (desired, selector, (containers, images)) = match rplc.spec.as_ref() {
      Some(spec) => (
        spec.replicas.unwrap_or_default(),
        spec
          .selector
          .as_ref()
          .unwrap_or(&BTreeMap::new())
          .iter()
          .map(|(key, val)| format!("{}={}", key, val))
          .collect::<Vec<String>>()
          .join(","),
        match spec.template.as_ref() {
          Some(tmpl) => match tmpl.spec.as_ref() {
            Some(pspec) => (
              pspec
                .containers
                .iter()
                .map(|c| c.name.to_owned())
                .collect::<Vec<String>>()
                .join(","),
              pspec
                .containers
                .iter()
                .filter_map(|c| c.image.to_owned())
                .collect::<Vec<String>>()
                .join(","),
            ),
            None => ("".into(), "".into()),
          },
          None => ("".into(), "".into()),
        },
      ),
      None => (0, "".into(), ("".into(), "".into())),
    };

    KubeReplicationController {
      name: rplc.metadata.name.clone().unwrap_or_default(),
      namespace: rplc.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(rplc.metadata.creation_timestamp.as_ref(), Utc::now()),
      desired,
      current,
      ready,
      containers,
      images,
      selector,
      k8s_obj: rplc.to_owned(),
    }
  }

  fn get_k8s_obj(&self) -> &ReplicationController {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_utils::*;

  #[test]
  fn test_replica_sets_from_api() {
    let (rplc, rplc_list): (Vec<KubeReplicationController>, Vec<_>) =
      convert_resource_from_file("replication_controllers");

    assert_eq!(rplc.len(), 2);
    assert_eq!(
      rplc[0],
      KubeReplicationController {
        name: "nginx".into(),
        namespace: "default".into(),
        age: utils::to_age(Some(&get_time("2021-07-27T14:37:49Z")), Utc::now()),
        k8s_obj: rplc_list[0].clone(),
        desired: 3,
        current: 3,
        ready: 3,
        containers: "nginx".into(),
        images: "nginx".into(),
        selector: "app=nginx".into(),
      }
    );
    assert_eq!(
      rplc[1],
      KubeReplicationController {
        name: "nginx-new".into(),
        namespace: "default".into(),
        age: utils::to_age(Some(&get_time("2021-07-27T14:45:24Z")), Utc::now()),
        k8s_obj: rplc_list[1].clone(),
        desired: 3,
        current: 3,
        ready: 0,
        containers: "nginx,nginx2".into(),
        images: "nginx,nginx".into(),
        selector: "app=nginx".into(),
      }
    );
  }
}
