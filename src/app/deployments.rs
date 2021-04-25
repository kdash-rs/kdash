use k8s_openapi::{api::apps::v1::Deployment, chrono::Utc};

use super::utils;

#[derive(Clone)]
pub struct KubeDeployments {
  pub name: String,
  pub namespace: String,
  pub ready: String,
  pub updated: i32,
  pub available: i32,
  pub age: String,
}

impl KubeDeployments {
  pub fn from_api(dp: &Deployment) -> Self {
    let (ready, available, updated) = match &dp.status {
      Some(s) => (
        format!(
          "{}/{}",
          s.available_replicas.unwrap_or_default(),
          s.replicas.unwrap_or_default()
        ),
        s.available_replicas.unwrap_or_default(),
        s.updated_replicas.unwrap_or_default(),
      ),
      _ => ("".into(), 0, 0),
    };

    KubeDeployments {
      name: dp.metadata.name.clone().unwrap_or_default(),
      namespace: dp.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(dp.metadata.creation_timestamp.as_ref(), Utc::now()),
      available,
      updated,
      ready,
    }
  }
}
