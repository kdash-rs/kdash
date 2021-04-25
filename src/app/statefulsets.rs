use k8s_openapi::{api::apps::v1::StatefulSet, chrono::Utc};

use super::utils;

#[derive(Clone)]
pub struct KubeStatefulSet {
  pub name: String,
  pub namespace: String,
  pub ready: String,
  pub service: String,
  pub age: String,
}

impl KubeStatefulSet {
  pub fn from_api(sts: &StatefulSet) -> Self {
    let ready = match &sts.status {
      Some(s) => format!("{}/{}", s.ready_replicas.unwrap_or_default(), s.replicas),
      _ => "".into(),
    };

    KubeStatefulSet {
      name: sts.metadata.name.clone().unwrap_or_default(),
      namespace: sts.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(sts.metadata.creation_timestamp.as_ref(), Utc::now()),
      service: sts
        .spec
        .as_ref()
        .map_or("n/a".into(), |spec| spec.service_name.to_owned()),
      ready,
    }
  }
}
