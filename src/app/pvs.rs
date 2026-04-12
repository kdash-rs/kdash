use async_trait::async_trait;
use chrono::Utc;
use k8s_openapi::{api::core::v1::PersistentVolume, apimachinery::pkg::api::resource::Quantity};
use ratatui::{
  layout::{Constraint, Rect},
  widgets::{Cell, Row},
  Frame,
};

use super::{
  models::{AppResource, KubeResource, Named},
  utils::{self},
  ActiveBlock, App,
};
use crate::{
  draw_resource_tab,
  network::Network,
  ui::utils::{
    describe_yaml_and_esc_hint, draw_describe_block, draw_resource_block, draw_yaml_block,
    get_describe_active, get_resource_title, help_bold_line, style_caution, style_primary,
    title_with_dual_style, ResourceTableProps,
  },
};

#[derive(Clone, Debug, PartialEq)]
pub struct KubePV {
  pub name: String,
  pub capacity: String,
  pub access_modes: String,
  pub reclaim_policy: String,
  pub status: String,
  pub claim: String,
  pub storage_class: String,
  pub reason: String,
  pub age: String,
  k8s_obj: PersistentVolume,
}

impl From<PersistentVolume> for KubePV {
  fn from(pvc: PersistentVolume) -> Self {
    let quantity = Quantity::default();
    let capacity = pvc
      .spec
      .clone()
      .unwrap_or_default()
      .capacity
      .unwrap_or_default();
    let capacity = capacity.get("storage").unwrap_or(&quantity);

    let claim = pvc.spec.clone().unwrap_or_default().claim_ref;

    let claim = format!(
      "{}/{}",
      claim
        .clone()
        .unwrap_or_default()
        .namespace
        .unwrap_or_default(),
      claim.unwrap_or_default().name.unwrap_or_default()
    );

    KubePV {
      name: pvc.metadata.name.clone().unwrap_or_default(),
      age: utils::to_age(pvc.metadata.creation_timestamp.as_ref(), Utc::now()),
      status: pvc
        .status
        .clone()
        .unwrap_or_default()
        .phase
        .unwrap_or_default(),
      capacity: capacity.0.clone(),
      access_modes: pvc
        .spec
        .clone()
        .unwrap_or_default()
        .access_modes
        .unwrap_or_default()
        .join(","),
      reclaim_policy: pvc
        .spec
        .clone()
        .unwrap_or_default()
        .persistent_volume_reclaim_policy
        .unwrap_or_default(),
      claim,
      storage_class: pvc
        .spec
        .clone()
        .unwrap_or_default()
        .storage_class_name
        .unwrap_or_default(),
      reason: pvc
        .status
        .clone()
        .unwrap_or_default()
        .reason
        .unwrap_or_default(),
      k8s_obj: utils::sanitize_obj(pvc),
    }
  }
}

impl Named for KubePV {
  fn get_name(&self) -> &String {
    &self.name
  }
}

impl KubeResource<PersistentVolume> for KubePV {
  fn get_k8s_obj(&self) -> &PersistentVolume {
    &self.k8s_obj
  }
}

static PV_TITLE: &str = "PersistentVolumes";

pub struct PvResource {}

#[async_trait]
impl AppResource for PvResource {
  fn render(block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      PV_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_block,
      app.data.persistent_volumes
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubePV> = nw.get_resources(PersistentVolume::into).await;

    let mut app = nw.app.lock().await;
    app.data.persistent_volumes.set_items(items);
  }
}

fn draw_block(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let is_loading = app.is_loading();
  let title = get_resource_title(app, PV_TITLE, "", app.data.persistent_volumes.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: help_bold_line(describe_yaml_and_esc_hint(), app.light_theme),
      resource: &mut app.data.persistent_volumes,
      table_headers: vec![
        "Name",
        "Capacity",
        "Access Modes",
        "Reclaim Policy",
        "Status",
        "Claim",
        "Storage Class",
        "Reason",
        "Age",
      ],
      column_widths: vec![
        Constraint::Percentage(20),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      let style = if c.status == "Pending" {
        style_caution(app.light_theme)
      } else {
        style_primary(app.light_theme)
      };
      Row::new(vec![
        Cell::from(c.name.to_owned()),
        Cell::from(c.capacity.to_owned()),
        Cell::from(c.access_modes.to_owned()),
        Cell::from(c.reclaim_policy.to_owned()),
        Cell::from(c.status.to_owned()),
        Cell::from(c.claim.to_owned()),
        Cell::from(c.storage_class.to_owned()),
        Cell::from(c.reason.to_owned()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style)
    },
    app.light_theme,
    is_loading,
  );
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_utils::*;

  #[test]
  fn test_persistent_volumes_from_api() {
    let (pvc, pvc_list): (Vec<KubePV>, Vec<_>) = convert_resource_from_file("pvs");

    assert_eq!(pvc.len(), 3);
    assert_eq!(
      pvc[0],
      KubePV {
        name: "pvc-149f1f3b-c0fd-471d-bc3e-d039369755ef".into(),
        age: utils::to_age(Some(&get_time("2023-06-30T17:27:26Z")), Utc::now()),
        k8s_obj: pvc_list[0].clone(),
        status: "Bound".into(),
        capacity: "8Gi".into(),
        access_modes: "ReadWriteOnce".into(),
        storage_class: "gp2".into(),
        reclaim_policy: "Delete".into(),
        claim: "jhipster/data-consul-0".into(),
        reason: "".into(),
      }
    );
  }
}
