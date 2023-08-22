use k8s_openapi::{api::storage::v1::StorageClass, chrono::Utc};

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
    draw_describe_block, draw_resource_block, get_cluster_wide_resource_title, get_describe_active,
    get_resource_title, style_primary, title_with_dual_style, ResourceTableProps, COPY_HINT,
    DESCRIBE_YAML_AND_ESC_HINT,
  },
};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeStorageClass {
  pub name: String,
  pub provisioner: String,
  pub reclaim_policy: String,
  pub volume_binding_mode: String,
  pub allow_volume_expansion: bool,
  pub age: String,
  k8s_obj: StorageClass,
}

impl From<StorageClass> for KubeStorageClass {
  fn from(storage_class: StorageClass) -> Self {
    KubeStorageClass {
      name: storage_class.metadata.name.clone().unwrap_or_default(),
      provisioner: storage_class.provisioner.clone(),
      reclaim_policy: storage_class.reclaim_policy.clone().unwrap_or_default(),
      volume_binding_mode: storage_class
        .volume_binding_mode
        .clone()
        .unwrap_or_default(),
      allow_volume_expansion: storage_class.allow_volume_expansion.unwrap_or_default(),
      age: utils::to_age(
        storage_class.metadata.creation_timestamp.as_ref(),
        Utc::now(),
      ),
      k8s_obj: utils::sanitize_obj(storage_class),
    }
  }
}

impl KubeResource<StorageClass> for KubeStorageClass {
  fn get_k8s_obj(&self) -> &StorageClass {
    &self.k8s_obj
  }
}

static STORAGE_CLASSES_LABEL: &str = "StorageClasses";

pub struct StorageClassResource {}

#[async_trait]
impl AppResource for StorageClassResource {
  fn render<B: Backend>(block: ActiveBlock, f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      STORAGE_CLASSES_LABEL,
      block,
      f,
      app,
      area,
      Self::render,
      draw_storage_classes_block,
      app.data.storage_classes
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeStorageClass> = nw.get_resources(StorageClass::into).await;

    let mut app = nw.app.lock().await;
    app.data.storage_classes.set_items(items);
  }
}

fn draw_storage_classes_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let title = get_cluster_wide_resource_title(
    STORAGE_CLASSES_LABEL,
    app.data.storage_classes.items.len(),
    "",
  );

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_YAML_AND_ESC_HINT.into(),
      resource: &mut app.data.storage_classes,
      table_headers: vec![
        "Name",
        "Provisioner",
        "Reclaim Policy",
        "Volume Binding Mode",
        "Allow Volume Expansion",
        "Age",
      ],
      column_widths: vec![
        Constraint::Percentage(10),
        Constraint::Percentage(20),
        Constraint::Percentage(10),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.name.to_owned()),
        Cell::from(c.provisioner.to_owned()),
        Cell::from(c.reclaim_policy.to_owned()),
        Cell::from(c.volume_binding_mode.to_owned()),
        Cell::from(c.allow_volume_expansion.to_string()),
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
  use k8s_openapi::chrono::Utc;

  use crate::app::{
    storageclass::KubeStorageClass,
    test_utils::{convert_resource_from_file, get_time},
    utils,
  };

  #[tokio::test]
  async fn test_storageclass_from_api() {
    let (storage_classes, storage_classes_list): (Vec<KubeStorageClass>, Vec<_>) =
      convert_resource_from_file("storageclass");
    assert_eq!(storage_classes_list.len(), 4);
    assert_eq!(
      storage_classes[0],
      KubeStorageClass {
        name: "ebs-performance".into(),
        provisioner: "kubernetes.io/aws-ebs".into(),
        reclaim_policy: "Delete".into(),
        volume_binding_mode: "Immediate".into(),
        allow_volume_expansion: false,
        age: utils::to_age(Some(&get_time("2021-12-14T11:08:59Z")), Utc::now()),
        k8s_obj: storage_classes_list[0].clone(),
      }
    );
  }
}
