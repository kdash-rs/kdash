use std::vec;

use k8s_openapi::{api::networking::v1::NetworkPolicy, chrono::Utc};

use super::{models::KubeResource, utils};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeNetworkPolicy {
  pub name: String,
  pub namespace: String,
  pub pod_selector: String,
  pub policy_types: String,
  pub age: String,
  k8s_obj: NetworkPolicy,
}

impl From<NetworkPolicy> for KubeNetworkPolicy {
  fn from(nw_policy: NetworkPolicy) -> Self {
    let pod_selector = match &nw_policy.spec {
      Some(s) => {
        let mut pod_selector = vec![];
        if let Some(match_labels) = &s.pod_selector.match_labels {
          for (k, v) in match_labels {
            pod_selector.push(format!("{}={}", k, v));
          }
        }
        pod_selector
      }
      _ => vec![],
    };

    Self {
      name: nw_policy.metadata.name.clone().unwrap_or_default(),
      namespace: nw_policy.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(nw_policy.metadata.creation_timestamp.as_ref(), Utc::now()),
      pod_selector: pod_selector.join(","),
      policy_types: nw_policy.spec.as_ref().map_or_else(
        || "".into(),
        |s| s.policy_types.clone().unwrap_or_default().join(","),
      ),
      k8s_obj: utils::sanitize_obj(nw_policy),
    }
  }
}

impl KubeResource<NetworkPolicy> for KubeNetworkPolicy {
  fn get_k8s_obj(&self) -> &NetworkPolicy {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_utils::*;

  #[test]
  fn test_nw_policys_from_api() {
    let (nw_policys, nw_policy_list): (Vec<KubeNetworkPolicy>, Vec<_>) =
      convert_resource_from_file("network_policy");

    assert_eq!(nw_policys.len(), 4);
    assert_eq!(
      nw_policys[3],
      KubeNetworkPolicy {
        name: "sample-network-policy-4".into(),
        namespace: "default".into(),
        age: utils::to_age(Some(&get_time("2023-07-04T17:04:33Z")), Utc::now()),
        k8s_obj: nw_policy_list[3].clone(),
        pod_selector: "app=webapp,app3=webapp3".into(),
        policy_types: "Egress,Ingress".into(),
      }
    );
  }
}
