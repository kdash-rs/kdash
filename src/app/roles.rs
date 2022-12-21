use k8s_openapi::{
  api::rbac::v1::{ClusterRole, ClusterRoleBinding, Role, RoleBinding},
  chrono::Utc,
};

use super::{models::KubeResource, utils};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeRole {
  pub namespace: String,
  pub name: String,
  pub age: String,
  k8s_obj: Role,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KubeRoleBinding {
  pub namespace: String,
  pub name: String,
  pub role: String,
  pub age: String,
  k8s_obj: RoleBinding,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KubeClusterRole {
  pub name: String,
  pub age: String,
  k8s_obj: ClusterRole,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KubeClusterRoleBinding {
  pub name: String,
  pub role: String,
  pub age: String,
  k8s_obj: ClusterRoleBinding,
}

impl From<Role> for KubeRole {
  fn from(role: Role) -> Self {
    KubeRole {
      namespace: role.metadata.namespace.clone().unwrap_or_default(),
      name: role.metadata.name.clone().unwrap_or_default(),
      age: utils::to_age(role.metadata.creation_timestamp.as_ref(), Utc::now()),
      k8s_obj: utils::sanitize_obj(role),
    }
  }
}

impl KubeResource<Role> for KubeRole {
  fn get_k8s_obj(&self) -> &Role {
    &self.k8s_obj
  }
}

impl From<ClusterRole> for KubeClusterRole {
  fn from(cluster_role: ClusterRole) -> Self {
    KubeClusterRole {
      name: cluster_role.metadata.name.clone().unwrap_or_default(),
      age: utils::to_age(
        cluster_role.metadata.creation_timestamp.as_ref(),
        Utc::now(),
      ),
      k8s_obj: utils::sanitize_obj(cluster_role),
    }
  }
}

impl KubeResource<ClusterRole> for KubeClusterRole {
  fn get_k8s_obj(&self) -> &ClusterRole {
    &self.k8s_obj
  }
}

impl From<RoleBinding> for KubeRoleBinding {
  fn from(role_binding: RoleBinding) -> Self {
    KubeRoleBinding {
      namespace: role_binding.metadata.namespace.clone().unwrap_or_default(),
      name: role_binding.metadata.name.clone().unwrap_or_default(),
      role: role_binding.role_ref.name.clone(),
      age: utils::to_age(
        role_binding.metadata.creation_timestamp.as_ref(),
        Utc::now(),
      ),
      k8s_obj: utils::sanitize_obj(role_binding),
    }
  }
}
impl KubeResource<RoleBinding> for KubeRoleBinding {
  fn get_k8s_obj(&self) -> &RoleBinding {
    &self.k8s_obj
  }
}

impl From<ClusterRoleBinding> for KubeClusterRoleBinding {
  fn from(crb: ClusterRoleBinding) -> Self {
    KubeClusterRoleBinding {
      name: crb.metadata.name.clone().unwrap_or_default(),
      role: format!("{}/{}", crb.role_ref.kind, crb.role_ref.name),
      age: utils::to_age(crb.metadata.creation_timestamp.as_ref(), Utc::now()),
      k8s_obj: utils::sanitize_obj(crb),
    }
  }
}

impl KubeResource<ClusterRoleBinding> for KubeClusterRoleBinding {
  fn get_k8s_obj(&self) -> &ClusterRoleBinding {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use k8s_openapi::chrono::Utc;

  use crate::app::{
    roles::{KubeClusterRole, KubeClusterRoleBinding, KubeRole, KubeRoleBinding},
    test_utils::{convert_resource_from_file, get_time},
    utils,
  };

  #[test]
  fn test_roles_binding_from_rbac_api() {
    let (roles, roles_list): (Vec<KubeRole>, Vec<_>) = convert_resource_from_file("roles");

    assert_eq!(roles.len(), 1);
    assert_eq!(
      roles[0],
      KubeRole {
        namespace: "default".to_string(),
        name: "kiali-viewer".into(),
        age: utils::to_age(Some(&get_time("2022-06-27T16:33:06Z")), Utc::now()),
        k8s_obj: roles_list[0].clone(),
      }
    )
  }

  #[test]
  fn test_cluster_roles_from_rbac_api() {
    let (cluster_roles, cluster_roles_list): (Vec<KubeClusterRole>, Vec<_>) =
      convert_resource_from_file("clusterroles");

    assert_eq!(cluster_roles.len(), 1);
    assert_eq!(
      cluster_roles[0],
      KubeClusterRole {
        name: "admin".into(),
        age: utils::to_age(Some(&get_time("2021-12-14T11:04:22Z")), Utc::now()),
        k8s_obj: cluster_roles_list[0].clone(),
      }
    )
  }

  #[test]
  fn test_role_binding_from_rbac_api() {
    let (role_bindings, rolebindings_list): (Vec<KubeRoleBinding>, Vec<_>) =
      convert_resource_from_file("role_bindings");

    assert_eq!(role_bindings.len(), 1);
    assert_eq!(
      role_bindings[0],
      KubeRoleBinding {
        namespace: "default".to_string(),
        name: "kiali".into(),
        role: "kiali-viewer".into(),
        age: utils::to_age(Some(&get_time("2022-06-27T16:33:07Z")), Utc::now()),
        k8s_obj: rolebindings_list[0].clone(),
      }
    )
  }

  #[test]
  fn test_cluster_role_bindings_from_rbac_api() {
    let (cluster_role_binding, cluster_role_bindings_list): (Vec<KubeClusterRoleBinding>, Vec<_>) =
      convert_resource_from_file("clusterrole_binding");

    assert_eq!(cluster_role_binding.len(), 2);
    assert_eq!(
      cluster_role_binding[0],
      KubeClusterRoleBinding {
        name: "admin-user".into(),
        role: "ClusterRole/cluster-admin".into(),
        age: utils::to_age(Some(&get_time("2022-03-02T16:50:53Z")), Utc::now()),
        k8s_obj: cluster_role_bindings_list[0].clone(),
      }
    )
  }
}
