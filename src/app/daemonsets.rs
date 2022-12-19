use k8s_openapi::{api::apps::v1::DaemonSet, chrono::Utc};

use super::{models::KubeResource, utils};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeDaemonSet {
  pub name: String,
  pub namespace: String,
  pub desired: i32,
  pub current: i32,
  pub ready: i32,
  pub up_to_date: i32,
  pub available: i32,
  pub age: String,
  k8s_obj: DaemonSet,
}
impl From<DaemonSet> for KubeDaemonSet {
  fn from(ds: DaemonSet) -> Self {
    let (desired, current, ready, up_to_date, available) = match ds.status.as_ref() {
      Some(s) => (
        s.desired_number_scheduled,
        s.current_number_scheduled,
        s.number_ready,
        s.updated_number_scheduled.unwrap_or_default(),
        s.number_available.unwrap_or_default(),
      ),
      _ => (0, 0, 0, 0, 0),
    };

    KubeDaemonSet {
      name: ds.metadata.name.clone().unwrap_or_default(),
      namespace: ds.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(ds.metadata.creation_timestamp.as_ref(), Utc::now()),
      desired,
      current,
      ready,
      up_to_date,
      available,
      k8s_obj: utils::sanitize_obj(ds),
    }
  }
}
impl KubeResource<DaemonSet> for KubeDaemonSet {
  fn get_k8s_obj(&self) -> &DaemonSet {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_utils::*;

  #[test]
  fn test_daemon_sets_from_api() {
    let (dss, dss_list): (Vec<KubeDaemonSet>, Vec<_>) = convert_resource_from_file("daemonsets");

    assert_eq!(dss.len(), 1);
    assert_eq!(
      dss[0],
      KubeDaemonSet {
        name: "svclb-traefik".into(),
        namespace: "kube-system".into(),
        age: utils::to_age(Some(&get_time("2021-07-05T09:36:45Z")), Utc::now()),
        k8s_obj: dss_list[0].clone(),
        desired: 1,
        current: 1,
        ready: 1,
        up_to_date: 1,
        available: 1,
      }
    );
  }
}
