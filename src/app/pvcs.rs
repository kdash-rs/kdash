use k8s_openapi::{
  api::core::v1::PersistentVolumeClaim, apimachinery::pkg::api::resource::Quantity, chrono::Utc,
};

use super::{models::KubeResource, utils};

#[derive(Clone, Debug, PartialEq)]
pub struct KubePVC {
  pub name: String,
  pub namespace: String,
  pub status: String,
  pub volume: String,
  pub capacity: String,
  pub access_modes: String,
  pub storage_class: String,
  pub age: String,
  k8s_obj: PersistentVolumeClaim,
}

impl From<PersistentVolumeClaim> for KubePVC {
  fn from(pvc: PersistentVolumeClaim) -> Self {
    let quantity = Quantity::default();
    let capacity = pvc
      .status
      .clone()
      .unwrap_or_default()
      .capacity
      .unwrap_or_default();
    let capacity = capacity.get("storage").unwrap_or(&quantity);

    KubePVC {
      name: pvc.metadata.name.clone().unwrap_or_default(),
      namespace: pvc.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(pvc.metadata.creation_timestamp.as_ref(), Utc::now()),
      status: pvc
        .status
        .clone()
        .unwrap_or_default()
        .phase
        .unwrap_or_default(),
      volume: pvc
        .spec
        .clone()
        .unwrap_or_default()
        .volume_name
        .unwrap_or_default(),
      capacity: capacity.0.clone(),
      access_modes: pvc
        .spec
        .clone()
        .unwrap_or_default()
        .access_modes
        .unwrap_or_default()
        .join(","),
      storage_class: pvc
        .spec
        .clone()
        .unwrap_or_default()
        .storage_class_name
        .unwrap_or_default(),
      k8s_obj: utils::sanitize_obj(pvc),
    }
  }
}

impl KubeResource<PersistentVolumeClaim> for KubePVC {
  fn get_k8s_obj(&self) -> &PersistentVolumeClaim {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_utils::*;

  #[test]
  fn test_replica_sets_from_api() {
    let (pvc, pvc_list): (Vec<KubePVC>, Vec<_>) = convert_resource_from_file("pvcs");

    assert_eq!(pvc.len(), 3);
    assert_eq!(
      pvc[0],
      KubePVC {
        name: "data-consul-0".into(),
        namespace: "jhipster".into(),
        age: utils::to_age(Some(&get_time("2023-06-30T17:27:23Z")), Utc::now()),
        k8s_obj: pvc_list[0].clone(),
        status: "Bound".into(),
        volume: "pvc-149f1f3b-c0fd-471d-bc3e-d039369755ef".into(),
        capacity: "8Gi".into(),
        access_modes: "ReadWriteOnce".into(),
        storage_class: "gp2".into(),
      }
    );
  }
}
