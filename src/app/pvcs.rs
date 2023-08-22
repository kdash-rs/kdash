use k8s_openapi::{
  api::core::v1::PersistentVolumeClaim, apimachinery::pkg::api::resource::Quantity, chrono::Utc,
};

use async_trait::async_trait;
use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  widgets::{Cell, Row},
  Frame,
};

use super::{
  models::{AppResource, KubeResource},
  utils::{self},
  ActiveBlock, App,
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

impl KubeResource<PersistentVolumeClaim> for KubePVC {
  fn get_k8s_obj(&self) -> &PersistentVolumeClaim {
    &self.k8s_obj
  }
}

static PVC_TITLE: &str = "PersistentVolumeClaims";

pub struct PvcResource {}

#[async_trait]
impl AppResource for PvcResource {
  fn render<B: Backend>(block: ActiveBlock, f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      PVC_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_pvc_block,
      app.data.pvcs
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubePVC> = nw
      .get_namespaced_resources(PersistentVolumeClaim::into)
      .await;

    let mut app = nw.app.lock().await;
    app.data.pvcs.set_items(items);
  }
}

fn draw_pvc_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, PVC_TITLE, "", app.data.pvcs.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_YAML_AND_ESC_HINT.into(),
      resource: &mut app.data.pvcs,
      table_headers: vec![
        "Namespace",
        "Name",
        "Status",
        "Volume",
        "Capacity",
        "Access Modes",
        "Storage Class",
        "Age",
      ],
      column_widths: vec![
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(20),
        Constraint::Percentage(10),
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
        Cell::from(c.status.to_owned()),
        Cell::from(c.volume.to_owned()),
        Cell::from(c.capacity.to_owned()),
        Cell::from(c.access_modes.to_owned()),
        Cell::from(c.storage_class.to_owned()),
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
