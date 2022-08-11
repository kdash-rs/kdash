use k8s_openapi::{api::rbac::v1::ClusterRole, api::rbac::v1::Role, chrono::Utc};

use super::{models::KubeResource, utils};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeRoles {
  pub namespace: String,
  pub name: String,
  pub age: String,
  k8s_obj: Role,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KubeClusterRoles {
  pub name: String,
  pub age: String,
  k8s_obj: ClusterRole,
}

impl KubeResource<Role> for KubeRoles {
  fn from_api(role: &Role) -> Self {
    KubeRoles {
      namespace: role.metadata.namespace.clone().unwrap_or_default(),
      name: role.metadata.name.clone().unwrap_or_default(),
      age: utils::to_age(role.metadata.creation_timestamp.as_ref(), Utc::now()),
      k8s_obj: role.to_owned(),
    }
  }

  fn get_k8s_obj(&self) -> &Role {
    &self.k8s_obj
  }
}

impl KubeResource<ClusterRole> for KubeClusterRoles {
  fn from_api(clusterrole: &ClusterRole) -> Self {
    KubeClusterRoles {
      name: clusterrole.metadata.name.clone().unwrap_or_default(),
      age: utils::to_age(clusterrole.metadata.creation_timestamp.as_ref(), Utc::now()),
      k8s_obj: clusterrole.to_owned(),
    }
  }

  fn get_k8s_obj(&self) -> &ClusterRole {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use crate::app::roles::{KubeClusterRoles, KubeRoles};
  use crate::app::test_utils::{convert_resource_from_file, get_time};
  use crate::app::utils;
  use k8s_openapi::chrono::Utc;

  #[test]
  fn test_roles_binding_from_rbac_api() {
    let (roles, roles_list): (Vec<KubeRoles>, Vec<_>) = convert_resource_from_file("roles");

    assert_eq!(roles.len(), 1);
    assert_eq!(
      roles[0],
      KubeRoles {
        namespace: "default".to_string(),
        name: "kiali-viewer".into(),
        age: utils::to_age(Some(&get_time("2022-06-27T16:33:06Z")), Utc::now()),
        k8s_obj: roles_list[0].clone(),
      }
    )
  }

  #[test]
  fn test_cluster_roles_binding_from_rbac_api() {
    let (clusterroles, cluster_roles_list): (Vec<KubeClusterRoles>, Vec<_>) =
      convert_resource_from_file("clusterroles");

    assert_eq!(clusterroles.len(), 1);
    assert_eq!(
      clusterroles[0],
      KubeClusterRoles {
        name: "admin".into(),
        age: utils::to_age(Some(&get_time("2021-12-14T11:04:22Z")), Utc::now()),
        k8s_obj: cluster_roles_list[0].clone(),
      }
    )
  }
}
