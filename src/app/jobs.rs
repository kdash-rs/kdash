use k8s_openapi::{api::batch::v1::Job, chrono::Utc};

use super::{models::KubeResource, utils};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeJob {
  pub name: String,
  pub namespace: String,
  pub completions: String,
  pub duration: String,
  pub age: String,
  k8s_obj: Job,
}
 
impl From<Job> for KubeJob {
  fn from(job: Job) -> Self {
    let completions = match (job.spec.as_ref(), job.status.as_ref()) {
      (Some(spc), Some(stat)) => match spc.completions {
        Some(c) => format!("{:?}/{:?}", stat.succeeded.unwrap_or_default(), c),
        None => match spc.parallelism {
          Some(p) => format!("{:?}/1 of {}", stat.succeeded.unwrap_or_default(), p),
          None => format!("{:?}/1", stat.succeeded),
        },
      },
      (None, Some(stat)) => format!("{:?}/1", stat.succeeded.unwrap_or_default()),
      _ => "".into(),
    };

    let duration = match job.status.as_ref() {
      Some(stat) => match stat.start_time.as_ref() {
        Some(st) => match stat.completion_time.as_ref() {
          Some(ct) => {
            let duration = ct.0.signed_duration_since(st.0);
            utils::duration_to_age(duration, true)
          }
          None => utils::to_age(stat.start_time.as_ref(), Utc::now()),
        },
        None => "<none>".to_string(),
      },
      None => "<none>".to_string(),
    };

    Self {
      name: job.metadata.name.clone().unwrap_or_default(),
      namespace: job.metadata.namespace.clone().unwrap_or_default(),
      completions,
      duration,
      age: utils::to_age(job.metadata.creation_timestamp.as_ref(), Utc::now()),
      k8s_obj: utils::sanitize_obj(job),
    }
  }
}

impl KubeResource<Job> for KubeJob {
  fn get_k8s_obj(&self) -> &Job {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_utils::{convert_resource_from_file, get_time};

  #[test]
  fn test_jobs_from_api() {
    let (jobs, jobs_list): (Vec<KubeJob>, Vec<_>) = convert_resource_from_file("jobs");

    assert_eq!(jobs.len(), 3);
    assert_eq!(
      jobs[0],
      KubeJob {
        name: "helm-install-traefik".into(),
        namespace: "kube-system".into(),
        age: utils::to_age(Some(&get_time("2021-06-11T13:49:45Z")), Utc::now()),
        k8s_obj: jobs_list[0].clone(),
        completions: "1/1".into(),
        duration: "39m44s".into()
      }
    );
    assert_eq!(
      jobs[1],
      KubeJob {
        name: "helm-install-traefik-2".into(),
        namespace: "kube-system".into(),
        age: utils::to_age(Some(&get_time("2021-06-11T13:49:45Z")), Utc::now()),
        k8s_obj: jobs_list[1].clone(),
        completions: "1/1 of 1".into(),
        duration: "39m44s".into()
      }
    );
    assert_eq!(
      jobs[2],
      KubeJob {
        name: "helm-install-traefik-3".into(),
        namespace: "kube-system".into(),
        age: utils::to_age(Some(&get_time("2021-06-11T13:49:45Z")), Utc::now()),
        k8s_obj: jobs_list[2].clone(),
        completions: "1/1".into(),
        duration: "39m44s".into()
      }
    );
  }
}
