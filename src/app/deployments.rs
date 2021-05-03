use k8s_openapi::{api::apps::v1::Deployment, chrono::Utc};

use super::{models::ResourceToYaml, utils};

#[derive(Clone)]
pub struct KubeDeployment {
  pub name: String,
  pub namespace: String,
  pub ready: String,
  pub updated: i32,
  pub available: i32,
  pub age: String,
  k8s_obj: Deployment,
}

impl KubeDeployment {
  pub fn from_api(deployment: &Deployment) -> Self {
    let (ready, available, updated) = match &deployment.status {
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

    KubeDeployment {
      name: deployment.metadata.name.clone().unwrap_or_default(),
      namespace: deployment.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(deployment.metadata.creation_timestamp.as_ref(), Utc::now()),
      available,
      updated,
      ready,
      k8s_obj: deployment.to_owned(),
    }
  }
}

impl ResourceToYaml<Deployment> for KubeDeployment {
  fn get_k8s_obj(&self) -> &Deployment {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  // TODO
}
