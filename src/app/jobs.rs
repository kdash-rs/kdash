use async_trait::async_trait;
use k8s_openapi::{api::batch::v1::Job, chrono::Utc};
use ratatui::{
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
    draw_describe_block, draw_yaml_block, draw_resource_block, get_describe_active, get_resource_title,
    style_primary, title_with_dual_style, ResourceTableProps, COPY_HINT, DESCRIBE_AND_YAML_HINT,
  },
};

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
  fn get_name(&self) -> &String {
    &self.name
  }
  fn get_k8s_obj(&self) -> &Job {
    &self.k8s_obj
  }
}

static JOBS_TITLE: &str = "Jobs";

pub struct JobResource {}

#[async_trait]
impl AppResource for JobResource {
  fn render<B: Backend>(block: ActiveBlock, f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      JOBS_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_block,
      app.data.jobs
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeJob> = nw.get_namespaced_resources(Job::into).await;

    let mut app = nw.app.lock().await;
    app.data.jobs.set_items(items);
  }
}

fn draw_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, JOBS_TITLE, "", app.data.jobs.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_AND_YAML_HINT.into(),
      resource: &mut app.data.jobs,
      table_headers: vec!["Namespace", "Name", "Completions", "Duration", "Age"],
      column_widths: vec![
        Constraint::Percentage(25),
        Constraint::Percentage(40),
        Constraint::Percentage(15),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.completions.to_owned()),
        Cell::from(c.duration.to_string()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary(app.light_theme))
    },
    app.light_theme,
    app.is_loading,
    app.data.selected.filter.to_owned(),
  );
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
