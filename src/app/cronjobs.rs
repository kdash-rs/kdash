use k8s_openapi::{api::batch::v2alpha1::CronJob, chrono::Utc};

use super::{models::KubeResource, utils};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeCronJob {
  pub name: String,
  pub namespace: String,
  pub metadata: String,
  pub completions: String,
  pub duration: String,
  pub age: String,
  k8s_obj: CronJob,
}

impl KubeResource<CronJob> for KubeCronJob {
  fn from_api(job: &CronJob) -> Self {
    let completions = match (job.spec.as_ref(), job.status.as_ref()) {
      (Some(spc), Some(stat)) => match spc.successful_jobs_history_limit {
        Some(c) => format!("{:?}/{:?}", stat.active.as_ref(), c),
        None => "".to_string(),
      },
      (None, Some(stat)) => format!("{:?}/1", stat.active.as_ref()),
      _ => "".into(),
    };

    let duration = match job.status.as_ref() {
      Some(stat) => match stat.last_schedule_time.as_ref() {
        Some(st) => match stat.last_schedule_time.as_ref() {
          Some(ct) => {
            let duration = ct.0.signed_duration_since(st.0);
            utils::duration_to_age(duration)
          }
          None => utils::to_age(stat.last_schedule_time.as_ref(), Utc::now()),
        },
        None => "<none>".to_string(),
      },
      None => "<none>".to_string(),
    };

    KubeCronJob {
      name: job.metadata.name.clone().unwrap_or_default(),
      namespace: job.metadata.namespace.clone().unwrap_or_default(),
      metadata: job.metadata.clone().generate_name.unwrap(),
      completions,
      duration,
      age: utils::to_age(job.metadata.creation_timestamp.as_ref(), Utc::now()),
      k8s_obj: job.to_owned(),
    }
  }

  fn get_k8s_obj(&self) -> &CronJob {
    &self.k8s_obj
  }
}

#[cfg(test)]
mod tests {
}
