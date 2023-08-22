use k8s_openapi::{api::batch::v1::CronJob, chrono::Utc};

use async_trait::async_trait;
use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  widgets::{Cell, Row},
  Frame,
};

use super::{
  models::{AppResource, KubeResource},
  utils, ActiveBlock, App,
};
use crate::{
  draw_resource_tab,
  network::Network,
  ui::utils::{
    draw_describe_block, draw_resource_block, get_describe_active, get_resource_title,
    style_primary, title_with_dual_style, ResourceTableProps, COPY_HINT,
    DESCRIBE_YAML_AND_ESC_HINT,
  },
};

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

impl From<CronJob> for KubeCronJob {
  fn from(cronjob: CronJob) -> Self {
    let (last_schedule, active) = match &cronjob.status {
      Some(cjs) => (
        utils::to_age_secs(cjs.last_schedule_time.as_ref(), Utc::now()),
        cjs.active.clone().unwrap_or_default().len(),
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
      k8s_obj: utils::sanitize_obj(cronjob),
    }
  }
}

impl KubeResource<CronJob> for KubeCronJob {
  fn get_k8s_obj(&self) -> &CronJob {
    &self.k8s_obj
  }
}

static CRON_JOBS_TITLE: &str = "CronJobs";

pub struct CronJobResource {}

#[async_trait]
impl AppResource for CronJobResource {
  fn render<B: Backend>(block: ActiveBlock, f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      CRON_JOBS_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_cronjobs_block,
      app.data.cronjobs
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeCronJob> = nw.get_namespaced_resources(CronJob::into).await;

    let mut app = nw.app.lock().await;
    app.data.cronjobs.set_items(items);
  }
}

fn draw_cronjobs_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, CRON_JOBS_TITLE, "", app.data.cronjobs.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_YAML_AND_ESC_HINT.into(),
      resource: &mut app.data.cronjobs,
      table_headers: vec![
        "Namespace",
        "Name",
        "Schedule",
        "Last Scheduled",
        "Suspend",
        "Active",
        "Age",
      ],
      column_widths: vec![
        Constraint::Percentage(20),
        Constraint::Percentage(25),
        Constraint::Percentage(15),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.schedule.to_owned()),
        Cell::from(c.last_schedule.to_string()),
        Cell::from(c.suspend.to_string()),
        Cell::from(c.active.to_string()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary(app.light_theme))
    },
    app.light_theme,
    app.is_loading,
  );
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
        last_schedule: utils::to_age_secs(Some(&get_time("2021-07-05T09:39:00Z")), Utc::now()),
        age: utils::to_age(Some(&get_time("2021-07-05T09:37:21Z")), Utc::now()),
        k8s_obj: jobs_list[0].clone(),
      }
    );
  }
}
