use k8s_openapi::{api::core::v1::ServiceAccount, chrono::Utc};

use super::{models::KubeResource, utils};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeSvcAcct {
  pub namespace: String,
  pub name: String,
  pub secrets: i32,
  pub age: String,
  k8s_obj: ServiceAccount,
}

// Get length of a vector
impl From<ServiceAccount> for KubeSvcAcct {
  fn from(acct: ServiceAccount) -> Self {
    KubeSvcAcct {
      namespace: acct.metadata.namespace.clone().unwrap_or_default(),
      name: acct.metadata.name.clone().unwrap_or_default(),
      secrets: acct.secrets.clone().unwrap_or_default().len() as i32,
      age: utils::to_age(acct.metadata.creation_timestamp.as_ref(), Utc::now()),
      k8s_obj: utils::sanitize_obj(acct),
    }
  }
}

impl KubeResource<ServiceAccount> for KubeSvcAcct {
  fn get_k8s_obj(&self) -> &ServiceAccount {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use k8s_openapi::chrono::Utc;

  use crate::app::{
    serviceaccounts::KubeSvcAcct,
    test_utils::{convert_resource_from_file, get_time},
    utils,
  };

  #[test]
  fn test_service_accounts_from_api() {
    let (serviceaccounts, serviceaccounts_list): (Vec<KubeSvcAcct>, Vec<_>) =
      convert_resource_from_file("serviceaccounts");

    assert_eq!(serviceaccounts.len(), 43);
    assert_eq!(
      serviceaccounts[0],
      KubeSvcAcct {
        namespace: "kube-node-lease".to_string(),
        name: "default".into(),
        secrets: 3,
        age: utils::to_age(Some(&get_time("2023-06-30T17:13:19Z")), Utc::now()),
        k8s_obj: serviceaccounts_list[0].clone(),
      }
    )
  }
}
