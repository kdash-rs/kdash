use async_trait::async_trait;
use chrono::Utc;
use k8s_openapi::{
  api::core::v1::PersistentVolumeClaim, apimachinery::pkg::api::resource::Quantity,
};
use ratatui::{
  layout::Rect,
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
    get_describe_active, get_resource_title, help_bold_line, responsive_columns, style_caution,
    style_primary, title_with_dual_style, ColumnDef, ResourceTableProps, ViewTier,
  },
};

#[derive(Clone, Debug, PartialEq)]
pub struct KubePVC {
  pub name: String,
  pub namespace: String,
  pub status: String,
  pub volume: String,
  pub capacity: String,
  pub access_modes: String,
  pub storage_class: String,
  pub age: String,
  k8s_obj: PersistentVolumeClaim,
}

impl From<PersistentVolumeClaim> for KubePVC {
  fn from(pvc: PersistentVolumeClaim) -> Self {
    let quantity = Quantity::default();
    let capacity = pvc
      .status
      .clone()
      .unwrap_or_default()
      .capacity
      .unwrap_or_default();
    let capacity = capacity.get("storage").unwrap_or(&quantity);

    KubePVC {
      name: pvc.metadata.name.clone().unwrap_or_default(),
      namespace: pvc.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(pvc.metadata.creation_timestamp.as_ref(), Utc::now()),
      status: pvc
        .status
        .clone()
        .unwrap_or_default()
        .phase
        .unwrap_or_default(),
      volume: pvc
        .spec
        .clone()
        .unwrap_or_default()
        .volume_name
        .unwrap_or_default(),
      capacity: capacity.0.clone(),
      access_modes: pvc
        .spec
        .clone()
        .unwrap_or_default()
        .access_modes
        .unwrap_or_default()
        .join(","),
      storage_class: pvc
        .spec
        .clone()
        .unwrap_or_default()
        .storage_class_name
        .unwrap_or_default(),
      k8s_obj: utils::sanitize_obj(pvc),
    }
  }
}

impl Named for KubePVC {
  fn get_name(&self) -> &String {
    &self.name
  }
}

impl KubeResource<PersistentVolumeClaim> for KubePVC {
  fn get_k8s_obj(&self) -> &PersistentVolumeClaim {
    &self.k8s_obj
  }
}

static PVC_TITLE: &str = "PersistentVolumeClaims";

pub struct PvcResource {}

#[async_trait]
impl AppResource for PvcResource {
  fn render(block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      PVC_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_block,
      app.data.persistent_volume_claims
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubePVC> = nw
      .get_namespaced_resources(PersistentVolumeClaim::into)
      .await;

    let mut app = nw.app.lock().await;
    app.data.persistent_volume_claims.set_items(items);
  }
}

const PVC_COLUMNS: [ColumnDef; 8] = [
  ColumnDef::all("Namespace", 10, 10, 10),
  ColumnDef::all("Name", 10, 10, 10),
  ColumnDef::all("Status", 10, 10, 10),
  ColumnDef::all("Volume", 20, 20, 20),
  ColumnDef::all("Capacity", 10, 10, 10),
  ColumnDef::all("Access Modes", 10, 10, 10),
  ColumnDef::all("Storage Class", 10, 10, 10),
  ColumnDef::all("Age", 10, 10, 10),
];

fn draw_block(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let is_loading = app.is_loading();
  let title = get_resource_title(
    app,
    PVC_TITLE,
    "",
    app.data.persistent_volume_claims.items.len(),
  );

  let (headers, widths) = responsive_columns(&PVC_COLUMNS, ViewTier::Compact);

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: help_bold_line(describe_yaml_and_esc_hint(), app.light_theme),
      resource: &mut app.data.persistent_volume_claims,
      table_headers: headers,
      column_widths: widths,
    },
    |c| {
      let style = if c.status == "Pending" {
        style_caution(app.light_theme)
      } else {
        style_primary(app.light_theme)
      };
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.status.to_owned()),
        Cell::from(c.volume.to_owned()),
        Cell::from(c.capacity.to_owned()),
        Cell::from(c.access_modes.to_owned()),
        Cell::from(c.storage_class.to_owned()),
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
  fn test_persistent_volume_claims_from_api() {
    let (pvc, pvc_list): (Vec<KubePVC>, Vec<_>) = convert_resource_from_file("pvcs");

    assert_eq!(pvc.len(), 3);
    assert_eq!(
      pvc[0],
      KubePVC {
        name: "data-consul-0".into(),
        namespace: "jhipster".into(),
        age: utils::to_age(Some(&get_time("2023-06-30T17:27:23Z")), Utc::now()),
        k8s_obj: pvc_list[0].clone(),
        status: "Bound".into(),
        volume: "pvc-149f1f3b-c0fd-471d-bc3e-d039369755ef".into(),
        capacity: "8Gi".into(),
        access_modes: "ReadWriteOnce".into(),
        storage_class: "gp2".into(),
      }
    );
  }
}
