use async_trait::async_trait;
use chrono::Utc;
use k8s_openapi::api::batch::v1::CronJob;
use ratatui::{
  layout::Rect,
  widgets::{Cell, Row},
  Frame,
};

use super::{
  models::{self, AppResource, KubeResource, Named},
  utils, ActiveBlock, App,
};
use crate::{
  app::key_binding::DEFAULT_KEYBINDING,
  draw_resource_tab,
  network::Network,
  ui::utils::{
    action_hint, describe_yaml_logs_and_esc_hint, draw_describe_block, draw_resource_block,
    draw_yaml_block, get_describe_active, get_resource_title, help_bold_line, responsive_columns,
    style_primary, title_with_dual_style, wide_hint, ColumnDef, ResourceTableProps, ViewTier,
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
  pub concurrency_policy: String,
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

    let (schedule, suspend, concurrency_policy) = match &cronjob.spec {
      Some(cjs) => (
        cjs.schedule.clone(),
        cjs.suspend.unwrap_or_default(),
        cjs
          .concurrency_policy
          .clone()
          .unwrap_or_else(|| "Allow".into()),
      ),
      None => ("".to_string(), false, "Allow".into()),
    };

    KubeCronJob {
      name: cronjob.metadata.name.clone().unwrap_or_default(),
      namespace: cronjob.metadata.namespace.clone().unwrap_or_default(),
      schedule,
      suspend,
      last_schedule,
      active,
      concurrency_policy,
      age: utils::to_age(cronjob.metadata.creation_timestamp.as_ref(), Utc::now()),
      k8s_obj: utils::sanitize_obj(cronjob),
    }
  }
}

impl Named for KubeCronJob {
  fn get_name(&self) -> &String {
    &self.name
  }
}

impl KubeResource<CronJob> for KubeCronJob {
  fn get_k8s_obj(&self) -> &CronJob {
    &self.k8s_obj
  }
}

impl models::HasPodSelector for KubeCronJob {
  fn pod_label_selector(&self) -> Option<String> {
    // CronJobs don't directly own pods — they create Jobs which create Pods.
    // Pod lookup for CronJobs requires resolving the CronJob→Job→Pod chain,
    // which is handled at the network layer rather than via a simple label selector.
    None
  }
}

static CRON_JOBS_TITLE: &str = "CronJobs";

pub struct CronJobResource {}

#[async_trait]
impl AppResource for CronJobResource {
  fn render(block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      CRON_JOBS_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_block,
      app.data.cronjobs
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeCronJob> = nw.get_namespaced_resources(CronJob::into).await;

    let mut app = nw.app.lock().await;
    app.data.cronjobs.set_items(items);
  }
}

const CRON_COLUMNS: [ColumnDef; 8] = [
  ColumnDef::all("Namespace", 20, 15, 15),
  ColumnDef::all("Name", 25, 20, 20),
  ColumnDef::all("Schedule", 15, 12, 12),
  ColumnDef::all("Last Scheduled", 10, 12, 12),
  ColumnDef::all("Suspend", 10, 8, 8),
  ColumnDef::all("Active", 10, 8, 8),
  ColumnDef::standard("Concurrency", 15, 15),
  ColumnDef::all("Age", 10, 10, 10),
];

fn draw_block(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let is_loading = app.is_loading();
  let title = get_resource_title(app, CRON_JOBS_TITLE, "", app.data.cronjobs.items.len());

  let tier = ViewTier::from_width(area.width, app.wide_columns);
  let (headers, widths) = responsive_columns(&CRON_COLUMNS, tier);

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: help_bold_line(
        format!(
          "{} | {} | {}",
          action_hint("pods", DEFAULT_KEYBINDING.submit.key),
          describe_yaml_logs_and_esc_hint(),
          wide_hint()
        ),
        app.light_theme,
      ),
      resource: &mut app.data.cronjobs,
      table_headers: headers,
      column_widths: widths,
    },
    |c| {
      let mut cells = vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.schedule.to_owned()),
        Cell::from(c.last_schedule.to_string()),
        Cell::from(c.suspend.to_string()),
        Cell::from(c.active.to_string()),
      ];
      if tier >= ViewTier::Standard {
        cells.push(Cell::from(c.concurrency_policy.to_owned()));
      }
      cells.push(Cell::from(c.age.to_owned()));
      Row::new(cells).style(style_primary(app.light_theme))
    },
    app.light_theme,
    is_loading,
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
        concurrency_policy: "Allow".into(),
        k8s_obj: jobs_list[0].clone(),
      }
    );
  }
}
