use anyhow::anyhow;
use async_trait::async_trait;
use k8s_openapi::api::core::v1::Namespace;
use kube::{api::ListParams, Api};
use ratatui::{
  layout::{Constraint, Rect},
  widgets::{Cell, Row, Table},
  Frame,
};

use super::{
  key_binding::DEFAULT_KEYBINDING,
  models::{AppResource, KubeResource},
  utils::{self, UNKNOWN},
  ActiveBlock, App,
};
use crate::{
  network::Network,
  ui::{
    utils::{
      filter_by_resource_name, layout_block_default, loading, style_highlight, style_primary,
      style_secondary, table_header_style,
    },
    HIGHLIGHT,
  },
};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct KubeNs {
  pub name: String,
  pub status: String,
  k8s_obj: Namespace,
}

impl From<Namespace> for KubeNs {
  fn from(ns: Namespace) -> Self {
    let status = match &ns.status {
      Some(stat) => match &stat.phase {
        Some(phase) => phase.clone(),
        _ => UNKNOWN.into(),
      },
      _ => UNKNOWN.into(),
    };

    KubeNs {
      name: ns.metadata.name.clone().unwrap_or_default(),
      status,
      k8s_obj: utils::sanitize_obj(ns),
    }
  }
}

impl KubeResource<Namespace> for KubeNs {
  fn get_name(&self) -> &String {
    &self.name
  }
  fn get_k8s_obj(&self) -> &Namespace {
    &self.k8s_obj
  }
}

pub struct NamespaceResource {}

#[async_trait]
impl AppResource for NamespaceResource {
  fn render(_block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let title = format!(
      " Namespaces {} (all: {}) ",
      DEFAULT_KEYBINDING.jump_to_namespace.key, DEFAULT_KEYBINDING.select_all_namespace.key
    );
    let mut block = layout_block_default(title.as_str());

    if app.get_current_route().active_block == ActiveBlock::Namespaces {
      block = block.style(style_secondary(app.light_theme))
    }

    if !app.data.namespaces.items.is_empty() {
      let rows = app.data.namespaces.items.iter().filter_map(|s| {
        let style = if Some(s.name.clone()) == app.data.selected.ns {
          style_secondary(app.light_theme)
        } else {
          style_primary(app.light_theme)
        };

        let mapper = row_cell_mapper(s).style(style);
        // return only rows that match filter if filter is set
        filter_by_resource_name(app.data.selected.filter.clone(), s, mapper)
      });

      let table = Table::new(rows, [Constraint::Length(22), Constraint::Length(6)])
        .header(table_header_style(vec!["Name", "Status"], app.light_theme))
        .block(block)
        .row_highlight_style(style_highlight())
        .highlight_symbol(HIGHLIGHT);

      f.render_stateful_widget(table, area, &mut app.data.namespaces.state);
    } else {
      loading(f, block, area, app.is_loading, app.light_theme);
    }
  }

  async fn get_resource(nw: &Network<'_>) {
    let api: Api<Namespace> = Api::all(nw.client.clone());

    let lp = ListParams::default();
    match api.list(&lp).await {
      Ok(ns_list) => {
        let items = ns_list.into_iter().map(KubeNs::from).collect::<Vec<_>>();
        let mut app = nw.app.lock().await;
        app.data.namespaces.set_items(items);
      }
      Err(e) => {
        nw.handle_error(anyhow!("Failed to get namespaces. {:?}", e))
          .await;
      }
    }
  }
}

fn row_cell_mapper(s: &KubeNs) -> Row<'static> {
  Row::new(vec![
    Cell::from(s.name.to_owned()),
    Cell::from(s.status.to_owned()),
  ])
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_utils::convert_resource_from_file;

  #[test]
  fn test_namespace_from_api() {
    let (nss, ns_list): (Vec<KubeNs>, Vec<_>) = convert_resource_from_file("ns");

    assert_eq!(nss.len(), 4);
    assert_eq!(
      nss[0],
      KubeNs {
        name: "default".into(),
        status: "Active".into(),
        k8s_obj: ns_list[0].clone()
      }
    );
  }
}
