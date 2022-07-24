use k8s_openapi::{api::rbac::v1::Role, chrono::Utc};

use super::{models::KubeResource, utils};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeRoles {
  pub name: String,
  pub age: String,
  k8s_obj: Role,
}

impl KubeResource<Role> for KubeRoles {
  fn from_api(role: &Role) -> Self {
    KubeRoles {
      name: role.metadata.name.clone().unwrap_or_default(),
      age: utils::to_age(role.metadata.creation_timestamp.as_ref(), Utc::now()),
      k8s_obj: role.to_owned(),
    }
  }

  fn get_k8s_obj(&self) -> &Role {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use crate::app::roles::KubeRoles;
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
        name: "kiali-viewer".into(),
        age: utils::to_age(Some(&get_time("2022-06-27T16:33:06Z")), Utc::now()),
        k8s_obj: roles_list[0].clone(),
      }
    )
  }
}
