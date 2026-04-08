//! Dynamic resources are discovered and accessed at runtime via the Kubernetes
//! discovery API and handled as untyped `DynamicObject`s.
//!
//! This is a client-side concept (not a Kubernetes kind). It lets kdash render
//! arbitrary resources (including CRDs) without hardcoded Rust types.
//! Typed resources like pods/services are modeled explicitly elsewhere for
//! richer, schema-aware views.
use anyhow::anyhow;
use async_trait::async_trait;
use chrono::Utc;
use kube::{
  core::DynamicObject,
  discovery::{ApiResource, Scope},
  ResourceExt,
};
use ratatui::{
  layout::{Constraint, Rect},
  widgets::{Cell, Row},
  Frame,
};
use std::collections::{BTreeMap, VecDeque};

use super::{
  models::{AppResource, KubeResource},
  utils, ActiveBlock, App,
};
use crate::{
  draw_resource_tab,
  network::Network,
  ui::utils::{
    describe_yaml_and_esc_hint, draw_describe_block, draw_resource_block, draw_yaml_block,
    get_describe_active, get_resource_title, help_bold_line, style_primary, title_with_dual_style,
    ResourceTableProps,
  },
};

#[derive(Clone, Debug)]
pub struct KubeDynamicKind {
  //   pub name: String,
  //   pub group: String,
  //   pub version: String,
  //   pub api_version: String,
  pub kind: String,
  pub scope: Scope,
  pub api_resource: ApiResource,
}

impl KubeDynamicKind {
  pub fn new(ar: ApiResource, scope: Scope) -> Self {
    KubeDynamicKind {
      api_resource: ar.clone(),
      //   name: ar.plural,
      //   group: ar.group,
      //   version: ar.version,
      //   api_version: ar.api_version,
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
  fn get_name(&self) -> &String {
    &self.name
  }
  fn get_k8s_obj(&self) -> &DynamicObject {
    &self.k8s_obj
  }
}

const DYNAMIC_CACHE_LIMIT: usize = 20;

#[derive(Debug, Clone, Default)]
pub struct DynamicResourceCache {
  entries: BTreeMap<String, Vec<KubeDynamicResource>>,
  order: VecDeque<String>,
}

impl DynamicResourceCache {
  fn touch(&mut self, key: &str) {
    self.order.retain(|entry| entry != key);
    self.order.push_back(key.to_owned());
  }

  pub fn get_cloned(&mut self, key: &str) -> Option<Vec<KubeDynamicResource>> {
    let items = self.entries.get(key).cloned()?;
    self.touch(key);
    Some(items)
  }

  pub fn insert(&mut self, key: String, items: Vec<KubeDynamicResource>) {
    self.entries.insert(key.clone(), items);
    self.touch(&key);

    while self.entries.len() > DYNAMIC_CACHE_LIMIT {
      if let Some(oldest) = self.order.pop_front() {
        if self.entries.remove(&oldest).is_some() {
          continue;
        }
      } else {
        break;
      }
    }
  }

  #[cfg(test)]
  fn contains_key(&self, key: &str) -> bool {
    self.entries.contains_key(key)
  }

  #[cfg(test)]
  fn len(&self) -> usize {
    self.entries.len()
  }

  #[cfg(test)]
  fn order(&self) -> Vec<String> {
    self.order.iter().cloned().collect()
  }
}

pub struct DynamicResource {}

pub fn dynamic_cache_key(kind: &KubeDynamicKind, namespace: Option<&str>) -> String {
  match kind.scope {
    Scope::Cluster => format!(
      "cluster:{}:{}",
      kind.api_resource.api_version, kind.api_resource.plural
    ),
    Scope::Namespaced => format!(
      "ns:{}:{}:{}",
      namespace.unwrap_or("*"),
      kind.api_resource.api_version,
      kind.api_resource.plural
    ),
  }
}

#[async_trait]
impl AppResource for DynamicResource {
  fn render(block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let title = app
      .data
      .selected
      .dynamic_kind
      .as_ref()
      .map(|res| res.kind.clone())
      .unwrap_or_default();
    draw_resource_tab!(
      title.as_str(),
      block,
      f,
      app,
      area,
      Self::render,
      draw_block,
      app.data.dynamic_resources
    );
  }

  /// fetch entries for a custom resource from the cluster
  async fn get_resource(nw: &Network<'_>) {
    let (selected_kind, selected_ns) = {
      let app = nw.app.lock().await;
      (
        app.data.selected.dynamic_kind.clone(),
        app.data.selected.ns.clone(),
      )
    };

    let Some(drs) = selected_kind else {
      return;
    };

    let cache_key = dynamic_cache_key(&drs, selected_ns.as_deref());
    let items = match nw.get_dynamic_resources(&drs, selected_ns.as_deref()).await {
      Ok(items) => items,
      Err(e) => {
        nw.handle_error(anyhow!("Failed to get dynamic resources. {}", e))
          .await;
        return;
      }
    };

    let mut app = nw.app.lock().await;
    app
      .data
      .dynamic_resource_cache
      .insert(cache_key.clone(), items.clone());
    if app.selected_dynamic_cache_key().as_deref() == Some(cache_key.as_str()) {
      app.data.dynamic_resources.set_items(items);
    }
  }
}

fn draw_block(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let is_loading = app.is_loading();
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
      inline_help: help_bold_line(describe_yaml_and_esc_hint(), app.light_theme),
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
    is_loading,
  );
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_utils::*;
  use kube::{core::ApiResource, discovery::Scope};

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

  #[test]
  fn test_dynamic_cache_key_uses_namespace_for_namespaced_resources() {
    let kind = KubeDynamicKind::new(
      ApiResource {
        group: "example.com".into(),
        version: "v1".into(),
        api_version: "example.com/v1".into(),
        kind: "Widget".into(),
        plural: "widgets".into(),
      },
      Scope::Namespaced,
    );

    assert_eq!(
      dynamic_cache_key(&kind, Some("team-a")),
      "ns:team-a:example.com/v1:widgets"
    );
    assert_eq!(
      dynamic_cache_key(&kind, Some("team-b")),
      "ns:team-b:example.com/v1:widgets"
    );
  }

  #[test]
  fn test_dynamic_cache_key_ignores_namespace_for_cluster_resources() {
    let kind = KubeDynamicKind::new(
      ApiResource {
        group: "example.com".into(),
        version: "v1".into(),
        api_version: "example.com/v1".into(),
        kind: "ClusterWidget".into(),
        plural: "clusterwidgets".into(),
      },
      Scope::Cluster,
    );

    assert_eq!(
      dynamic_cache_key(&kind, Some("team-a")),
      "cluster:example.com/v1:clusterwidgets"
    );
    assert_eq!(
      dynamic_cache_key(&kind, Some("team-b")),
      "cluster:example.com/v1:clusterwidgets"
    );
  }

  #[test]
  fn test_dynamic_resource_cache_evicts_oldest_entry_after_limit() {
    let mut cache = DynamicResourceCache::default();

    for idx in 0..=DYNAMIC_CACHE_LIMIT {
      cache.insert(format!("key-{idx}"), vec![]);
    }

    assert_eq!(cache.len(), DYNAMIC_CACHE_LIMIT);
    assert!(!cache.contains_key("key-0"));
    assert!(cache.contains_key(&format!("key-{}", DYNAMIC_CACHE_LIMIT)));
  }

  #[test]
  fn test_dynamic_resource_cache_get_refreshes_lru_order() {
    let mut cache = DynamicResourceCache::default();
    cache.insert("a".into(), vec![]);
    cache.insert("b".into(), vec![]);
    cache.insert("c".into(), vec![]);

    let _ = cache.get_cloned("a");

    assert_eq!(cache.order(), vec!["b", "c", "a"]);
  }
}
