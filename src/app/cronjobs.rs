use k8s_openapi::api::batch::v2alpha1::CronJob;

use super::models::KubeResource;

#[derive(Clone, Debug, PartialEq)]
pub struct KubeCronJob {
  pub name: String,
  pub namespace: String,
  pub schedule: String,
  pub suspend: Option<bool>,
  pub active: Option<bool>,
  k8s_obj: CronJob,
}

impl KubeResource<CronJob> for KubeCronJob {
  fn from_api(job: &CronJob) -> Self {
    KubeCronJob {
      name: job.metadata.name.clone().unwrap_or_default(),
      namespace: job.metadata.namespace.clone().unwrap_or_default(),
      schedule: "".to_string(),     //job.spec.schedule.to_string(),
      suspend: Option::from(false), //job.spec.suspend,
      active: Option::from(false),  //job.status.active.clone().unwrap_or_default(),
      k8s_obj: job.to_owned(),
    }
  }

  fn get_k8s_obj(&self) -> &CronJob {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use super::super::test_utils::convert_resource_from_file;
  use super::*;

  #[test]
  fn test_jobs_from_api() {
    let (jobs, jobs_list): (Vec<KubeCronJob>, Vec<_>) = convert_resource_from_file("cronjobs");

    assert_eq!(jobs.len(), 3);
    assert_eq!(
      jobs[0],
      KubeCronJob {
        name: "cronjob-1".into(),
        namespace: "default".into(),
        schedule: "*/1 * * * *".into(),
        suspend: None,
        active: Option::None,
        k8s_obj: jobs_list[0].clone(),
      }
    );
  }
}
