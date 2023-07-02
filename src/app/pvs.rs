use k8s_openapi::{
  api::core::v1::PersistentVolume, apimachinery::pkg::api::resource::Quantity, chrono::Utc,
};

use super::{models::KubeResource, utils};

#[derive(Clone, Debug, PartialEq)]
pub struct KubePV {
  pub name: String,
  pub capacity: String,
  pub access_modes: String,
  pub reclaim_policy: String,
  pub status: String,
  pub claim: String,
  pub storage_class: String,
  pub reason: String,
  pub age: String,
  k8s_obj: PersistentVolume,
}

impl From<PersistentVolume> for KubePV {
  fn from(pvc: PersistentVolume) -> Self {
    let quantity = Quantity::default();
    let capacity = pvc
      .spec
      .clone()
      .unwrap_or_default()
      .capacity
      .unwrap_or_default();
    let capacity = capacity.get("storage").unwrap_or(&quantity);

    let claim = pvc.spec.clone().unwrap_or_default().claim_ref;

    let claim = format!(
      "{}/{}",
      claim
        .clone()
        .unwrap_or_default()
        .namespace
        .unwrap_or_default(),
      claim.unwrap_or_default().name.unwrap_or_default()
    );

    KubePV {
      name: pvc.metadata.name.clone().unwrap_or_default(),
      age: utils::to_age(pvc.metadata.creation_timestamp.as_ref(), Utc::now()),
      status: pvc
        .status
        .clone()
        .unwrap_or_default()
        .phase
        .unwrap_or_default(),
      capacity: capacity.0.clone(),
      access_modes: pvc
        .spec
        .clone()
        .unwrap_or_default()
        .access_modes
        .unwrap_or_default()
        .join(","),
      reclaim_policy: pvc
        .spec
        .clone()
        .unwrap_or_default()
        .persistent_volume_reclaim_policy
        .unwrap_or_default(),
      claim,
      storage_class: pvc
        .spec
        .clone()
        .unwrap_or_default()
        .storage_class_name
        .unwrap_or_default(),
      reason: pvc
        .status
        .clone()
        .unwrap_or_default()
        .reason
        .unwrap_or_default(),
      k8s_obj: utils::sanitize_obj(pvc),
    }
  }
}

impl KubeResource<PersistentVolume> for KubePV {
  fn get_k8s_obj(&self) -> &PersistentVolume {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_utils::*;

  #[test]
  fn test_persistent_volumes_from_api() {
    let (pvc, pvc_list): (Vec<KubePV>, Vec<_>) = convert_resource_from_file("pvs");

    assert_eq!(pvc.len(), 3);
    assert_eq!(
      pvc[0],
      KubePV {
        name: "pvc-149f1f3b-c0fd-471d-bc3e-d039369755ef".into(),
        age: utils::to_age(Some(&get_time("2023-06-30T17:27:26Z")), Utc::now()),
        k8s_obj: pvc_list[0].clone(),
        status: "Bound".into(),
        capacity: "8Gi".into(),
        access_modes: "ReadWriteOnce".into(),
        storage_class: "gp2".into(),
        reclaim_policy: "Delete".into(),
        claim: "jhipster/data-consul-0".into(),
        reason: "".into(),
      }
    );
  }
}
