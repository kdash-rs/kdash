use k8s_openapi::{api::batch::v1::Job, chrono::Utc};

use super::{models::KubeResource, utils};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeJob {
  pub name: String,
  pub namespace: String,
  pub desired: i32,
  pub current: i32,
  pub ready: i32,
  pub age: String,
  k8s_obj: Job,
}

impl KubeResource<Job> for KubeJob {
  fn from_api(rps: &Job) -> Self {
    let (_current, ready) = match rps.status.as_ref() {
      Some(s) => (s.active, s.active.unwrap_or_default()),
      _ => (std::option::Option::None, 0),
    };

    KubeJob {
      name: rps.metadata.name.clone().unwrap_or_default(),
      namespace: rps.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(rps.metadata.creation_timestamp.as_ref(), Utc::now()),
      desired: rps
        .spec
        .as_ref()
        .map_or(0, |s| s.completions.unwrap_or_default()),
      ready,
      k8s_obj: rps.to_owned(),
      current: 0,
    }
  }

  fn get_k8s_obj(&self) -> &Job {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use super::super::test_utils::{convert_resource_from_file, get_time};
  use super::*;

  #[test]
  fn test_jobs_from_api() {
    let (jobs, jobs_list): (Vec<KubeJob>, Vec<_>) = convert_resource_from_file("jobs");

    assert_eq!(jobs.len(), 4);
    assert_eq!(
      jobs[0],
      KubeJob {
        name: "metrics-server-86cbb8457f".into(),
        namespace: "kube-system".into(),
        age: utils::to_age(Some(&get_time("2021-05-10T21:48:19Z")), Utc::now()),
        k8s_obj: jobs_list[0].clone(),
        desired: 0,
        current: 0,
        ready: 0,
      }
    );
  }
}
