use k8s_openapi::{api::batch::v2alpha1::CronJob, chrono::Utc};

use super::{models::KubeResource, utils};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeCronJob {
  pub name: String,
  pub namespace: String,
  pub schedule: String,
  pub last_schedule: String,
  pub suspend: bool,
  pub active: usize,
  pub age: String,
  k8s_obj: CronJob,
}

impl KubeResource<CronJob> for KubeCronJob {
  fn from_api(cronjob: &CronJob) -> Self {
    let (last_schedule, active) = match &cronjob.status {
      Some(cjs) => (
        utils::to_age(cjs.last_schedule_time.as_ref(), Utc::now()),
        cjs.active.len(),
      ),
      None => ("<none>".to_string(), 0),
    };

    let (schedule, suspend) = match &cronjob.spec {
      Some(cjs) => (cjs.schedule.clone(), cjs.suspend.unwrap_or_default()),
      None => ("".to_string(), false),
    };

    KubeCronJob {
      name: cronjob.metadata.name.clone().unwrap_or_default(),
      namespace: cronjob.metadata.namespace.clone().unwrap_or_default(),
      schedule,
      suspend,
      last_schedule,
      active,
      age: utils::to_age(cronjob.metadata.creation_timestamp.as_ref(), Utc::now()),
      k8s_obj: cronjob.to_owned(),
    }
  }

  fn get_k8s_obj(&self) -> &CronJob {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_utils::{convert_resource_from_file, get_time};

  #[test]
  fn test_cronjobs_from_api() {
    let (jobs, jobs_list): (Vec<KubeCronJob>, Vec<_>) = convert_resource_from_file("cronjobs");

    assert_eq!(jobs.len(), 1);
    assert_eq!(
      jobs[0],
      KubeCronJob {
        name: "hello".into(),
        namespace: "default".into(),
        schedule: "*/1 * * * *".into(),
        suspend: false,
        active: 0,
        last_schedule: utils::to_age(Some(&get_time("2021-07-05T09:39:00Z")), Utc::now()),
        age: utils::to_age(Some(&get_time("2021-07-05T09:37:21Z")), Utc::now()),
        k8s_obj: jobs_list[0].clone(),
      }
    );
  }
}
