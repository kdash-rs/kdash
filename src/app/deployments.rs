use k8s_openapi::{api::apps::v1::Deployment, chrono::Utc};

use super::{models::KubeResource, utils};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeDeployment {
  pub name: String,
  pub namespace: String,
  pub ready: String,
  pub updated: i32,
  pub available: i32,
  pub age: String,
  k8s_obj: Deployment,
}

impl KubeResource<Deployment> for KubeDeployment {
  fn from_api(deployment: &Deployment) -> Self {
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

  fn get_k8s_obj(&self) -> &Deployment {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use super::{
    super::test_utils::{convert_resource_from_file, get_time},
    *,
  };

  #[test]
  fn test_deployments_from_api() {
    let (deployments, deployment_list): (Vec<KubeDeployment>, Vec<_>) =
      convert_resource_from_file("deployments");

    assert_eq!(deployments.len(), 4);
    assert_eq!(
      deployments[0],
      KubeDeployment {
        name: "metrics-server".into(),
        namespace: "kube-system".into(),
        age: utils::to_age(Some(&get_time("2021-05-10T21:48:06Z")), Utc::now()),
        k8s_obj: deployment_list[0].clone(),
        available: 1,
        updated: 1,
        ready: "1/1".into(),
      }
    );
  }
}
