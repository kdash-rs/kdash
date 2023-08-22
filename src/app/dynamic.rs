use k8s_openapi::chrono::Utc;
use kube::{
  core::DynamicObject,
  discovery::{ApiResource, Scope},
  Api, ResourceExt,
};

use anyhow::anyhow;
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

#[derive(Clone, Debug)]
pub struct KubeDynamicKind {
  pub name: String,
  pub group: String,
  pub version: String,
  pub api_version: String,
  pub kind: String,
  pub scope: Scope,
  pub api_resource: ApiResource,
}

impl KubeDynamicKind {
  pub fn new(ar: ApiResource, scope: Scope) -> Self {
    KubeDynamicKind {
      api_resource: ar.clone(),
      name: ar.plural,
      group: ar.group,
      version: ar.version,
      api_version: ar.api_version,
      kind: ar.kind,
      scope,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct KubeDynamicResource {
  pub name: String,
  pub namespace: Option<String>,
  pub age: String,
  k8s_obj: DynamicObject,
}

impl From<DynamicObject> for KubeDynamicResource {
  fn from(item: DynamicObject) -> Self {
    KubeDynamicResource {
      name: item.name_any(),
      namespace: item.clone().metadata.namespace,
      age: utils::to_age(item.metadata.creation_timestamp.as_ref(), Utc::now()),
      k8s_obj: item,
    }
  }
}

impl KubeResource<DynamicObject> for KubeDynamicResource {
  fn get_k8s_obj(&self) -> &DynamicObject {
    &self.k8s_obj
  }
}

pub struct DynamicResource {}

#[async_trait]
impl AppResource for DynamicResource {
  fn render<B: Backend>(block: ActiveBlock, f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
    let title = if let Some(res) = &app.data.selected.dynamic_kind {
      res.kind.as_str()
    } else {
      ""
    };
    draw_resource_tab!(
      title,
      block,
      f,
      app,
      area,
      Self::render,
      draw_dynamic_res_block,
      app.data.dynamic_resources
    );
  }

  /// fetch entries for a custom resource from the cluster
  async fn get_resource(nw: &Network<'_>) {
    let mut app = nw.app.lock().await;

    if let Some(drs) = &app.data.selected.dynamic_kind {
      let api: Api<DynamicObject> = if drs.scope == Scope::Cluster {
        Api::all_with(nw.client.clone(), &drs.api_resource)
      } else {
        match &app.data.selected.ns {
          Some(ns) => Api::namespaced_with(nw.client.clone(), ns, &drs.api_resource),
          None => Api::all_with(nw.client.clone(), &drs.api_resource),
        }
      };

      let items = match api.list(&Default::default()).await {
        Ok(list) => list
          .items
          .iter()
          .map(|item| KubeDynamicResource::from(item.clone()))
          .collect::<Vec<KubeDynamicResource>>(),
        Err(e) => {
          nw.handle_error(anyhow!("Failed to get dynamic resources. {:?}", e))
            .await;
          return;
        }
      };
      app.data.dynamic_resources.set_items(items);
    }
  }
}

fn draw_dynamic_res_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let (title, scope) = if let Some(res) = &app.data.selected.dynamic_kind {
    (res.kind.as_str(), res.scope.clone())
  } else {
    ("", Scope::Cluster)
  };
  let title = get_resource_title(app, title, "", app.data.dynamic_resources.items.len());

  let (table_headers, column_widths) = if scope == Scope::Cluster {
    (
      vec!["Name", "Age"],
      vec![Constraint::Percentage(70), Constraint::Percentage(30)],
    )
  } else {
    (
      vec!["Namespace", "Name", "Age"],
      vec![
        Constraint::Percentage(30),
        Constraint::Percentage(50),
        Constraint::Percentage(20),
      ],
    )
  };

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_YAML_AND_ESC_HINT.into(),
      resource: &mut app.data.dynamic_resources,
      table_headers,
      column_widths,
    },
    |c| {
      let rows = if scope == Scope::Cluster {
        Row::new(vec![
          Cell::from(c.name.to_owned()),
          Cell::from(c.age.to_owned()),
        ])
      } else {
        Row::new(vec![
          Cell::from(c.namespace.clone().unwrap_or_default()),
          Cell::from(c.name.to_owned()),
          Cell::from(c.age.to_owned()),
        ])
      };
      rows.style(style_primary(app.light_theme))
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
  fn test_dynamic_resource_from_api() {
    let (dynamic_resource, res_list): (Vec<KubeDynamicResource>, Vec<_>) =
      convert_resource_from_file("dynamic_resource");

    assert_eq!(dynamic_resource.len(), 6);
    assert_eq!(
      dynamic_resource[0],
      KubeDynamicResource {
        name: "consul-5bb65dd4c8".into(),
        namespace: Some("jhipster".into()),
        age: utils::to_age(Some(&get_time("2023-06-30T17:27:23Z")), Utc::now()),
        k8s_obj: res_list[0].clone(),
      }
    );
  }
}
