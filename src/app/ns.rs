use anyhow::anyhow;
use async_trait::async_trait;
use k8s_openapi::api::core::v1::Namespace;
use kube::{api::ListParams, config::Kubeconfig, Api, Error};
use log::warn;
use ratatui::{
  layout::{Constraint, Rect},
  widgets::{Cell, Row, Table},
  Frame,
};

use super::{
  key_binding::DEFAULT_KEYBINDING,
  models::{AppResource, KubeResource, Named},
  utils::{self, UNKNOWN},
  ActiveBlock, App,
};
use crate::{
  network::Network,
  ui::{
    utils::{
      default_part, filter_by_resource_name, filter_cursor_position, filter_status_parts,
      help_part, layout_block_default_line, loading, mixed_bold_line, style_highlight,
      style_primary, style_secondary, table_header_style,
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

impl Named for KubeNs {
  fn get_name(&self) -> &String {
    &self.name
  }
}

impl KubeResource<Namespace> for KubeNs {
  fn get_k8s_obj(&self) -> &Namespace {
    &self.k8s_obj
  }
}

pub struct NamespaceResource {}

fn is_forbidden_namespace_list_error(error: &Error) -> bool {
  matches!(error, Error::Api(status) if status.is_forbidden())
}

fn kubeconfig_namespace(
  kubeconfig: Option<&Kubeconfig>,
  context_name: Option<&str>,
) -> Option<String> {
  let kubeconfig = kubeconfig?;
  let context_name = context_name.or(kubeconfig.current_context.as_deref())?;

  kubeconfig
    .contexts
    .iter()
    .find(|context| context.name == context_name)
    .and_then(|context| context.context.as_ref())
    .and_then(|context| context.namespace.clone())
    .filter(|namespace| !namespace.is_empty())
}

fn fallback_namespace_name(app: &App) -> String {
  app
    .data
    .active_context
    .as_ref()
    .and_then(|context| context.namespace.clone())
    .filter(|namespace| !namespace.is_empty())
    .or_else(|| {
      kubeconfig_namespace(
        app.data.kubeconfig.as_ref(),
        app
          .data
          .active_context
          .as_ref()
          .map(|context| context.name.as_str()),
      )
    })
    .unwrap_or_else(|| "default".into())
}

fn apply_namespace_list_fallback(app: &mut App, error: &Error) -> bool {
  if !is_forbidden_namespace_list_error(error) {
    return false;
  }

  let namespace = fallback_namespace_name(app);
  warn!(
    "Failed to list namespaces ({:?}), falling back to configured namespace: {}",
    error, namespace
  );

  app.api_error.clear();
  app.data.namespaces.set_items(vec![KubeNs {
    name: namespace.clone(),
    status: UNKNOWN.into(),
    ..Default::default()
  }]);
  app.data.selected.ns = Some(namespace);

  true
}

#[async_trait]
impl AppResource for NamespaceResource {
  fn render(_block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let title = if app.ns_filter_active {
      let mut parts = vec![default_part(" Namespaces ".to_string())];
      parts.extend(filter_status_parts(&app.ns_filter, true));
      mixed_bold_line(parts, app.light_theme)
    } else {
      mixed_bold_line(
        [
          default_part(" Namespaces ".to_string()),
          help_part(format!(
            "{} | all: {} | filter {} ",
            DEFAULT_KEYBINDING.jump_to_namespace.key,
            DEFAULT_KEYBINDING.select_all_namespace.key,
            DEFAULT_KEYBINDING.filter.key
          )),
        ],
        app.light_theme,
      )
    };
    let mut block = layout_block_default_line(title);

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
        filter_by_resource_name(&app.ns_filter, s, mapper)
      });

      let table = Table::new(rows, [Constraint::Length(22), Constraint::Length(6)])
        .header(table_header_style(vec!["Name", "Status"], app.light_theme))
        .block(block)
        .row_highlight_style(style_highlight())
        .highlight_symbol(HIGHLIGHT);

      f.render_stateful_widget(table, area, &mut app.data.namespaces.state);
    } else {
      loading(f, block, area, app.is_loading(), app.light_theme);
    }

    if app.ns_filter_active {
      f.set_cursor_position(filter_cursor_position(
        area,
        " Namespaces [".chars().count(),
        &app.ns_filter,
      ));
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
        let mut app = nw.app.lock().await;
        if !apply_namespace_list_fallback(&mut app, &e) {
          drop(app);
          nw.handle_error(anyhow!("Failed to get namespaces. {}", e))
            .await;
        }
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
  use ratatui::{backend::TestBackend, Terminal};

  use super::*;
  use crate::app::{contexts::KubeContext, test_utils::convert_resource_from_file, App};
  use kube::{
    config::{Context, NamedContext},
    core::Status,
  };

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

  #[test]
  fn test_apply_namespace_list_fallback_uses_active_context_namespace() {
    let mut app = App::default();
    app.data.active_context = Some(KubeContext {
      name: "ctx-a".into(),
      namespace: Some("team-a".into()),
      ..Default::default()
    });
    let error = Error::Api(
      Status::failure("forbidden", "Forbidden")
        .with_code(403)
        .boxed(),
    );

    assert!(apply_namespace_list_fallback(&mut app, &error));
    assert_eq!(app.data.namespaces.items.len(), 1);
    assert_eq!(app.data.namespaces.items[0].name, "team-a");
    assert_eq!(app.data.selected.ns.as_deref(), Some("team-a"));
  }

  #[test]
  fn test_apply_namespace_list_fallback_uses_kubeconfig_namespace() {
    let mut app = App::default();
    app.data.kubeconfig = Some(Kubeconfig {
      current_context: Some("ctx-a".into()),
      contexts: vec![NamedContext {
        name: "ctx-a".into(),
        context: Some(Context {
          namespace: Some("from-kubeconfig".into()),
          ..Default::default()
        }),
      }],
      ..Default::default()
    });
    let error = Error::Api(
      Status::failure("forbidden", "Forbidden")
        .with_code(403)
        .boxed(),
    );

    assert!(apply_namespace_list_fallback(&mut app, &error));
    assert_eq!(app.data.namespaces.items.len(), 1);
    assert_eq!(app.data.namespaces.items[0].name, "from-kubeconfig");
    assert_eq!(app.data.selected.ns.as_deref(), Some("from-kubeconfig"));
  }

  #[test]
  fn test_apply_namespace_list_fallback_defaults_to_default_namespace() {
    let mut app = App::default();
    let error = Error::Api(
      Status::failure("forbidden", "Forbidden")
        .with_code(403)
        .boxed(),
    );

    assert!(apply_namespace_list_fallback(&mut app, &error));
    assert_eq!(app.data.namespaces.items.len(), 1);
    assert_eq!(app.data.namespaces.items[0].name, "default");
    assert_eq!(app.data.selected.ns.as_deref(), Some("default"));
  }

  #[test]
  fn test_apply_namespace_list_fallback_ignores_non_forbidden_errors() {
    let mut app = App::default();
    let error = Error::Api(
      Status::failure("boom", "InternalError")
        .with_code(500)
        .boxed(),
    );

    assert!(!apply_namespace_list_fallback(&mut app, &error));
    assert!(app.data.namespaces.items.is_empty());
    assert!(app.data.selected.ns.is_none());
  }

  #[test]
  fn test_render_shows_clear_hint_when_namespace_filter_is_active() {
    let backend = TestBackend::new(60, 6);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
      .draw(|f| {
        let size = f.area();
        let mut app = App::default();
        app.push_navigation_stack(crate::app::RouteId::Home, ActiveBlock::Namespaces);
        app.ns_filter_active = true;
        app.ns_filter = "prod".into();
        app.data.namespaces.set_items(vec![KubeNs {
          name: "prod".into(),
          status: "Active".into(),
          ..Default::default()
        }]);
        NamespaceResource::render(ActiveBlock::Namespaces, f, &mut app, size);
      })
      .unwrap();

    let first_line = (0..terminal.backend().buffer().area.width)
      .map(|col| terminal.backend().buffer()[(col, 0)].symbol())
      .collect::<String>();

    assert!(first_line.contains("[prod] | clear <Esc>"));
  }
}
