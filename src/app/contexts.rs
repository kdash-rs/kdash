use async_trait::async_trait;
use kube::config::{Context, Kubeconfig, NamedContext};
use ratatui::{
  layout::{Constraint, Rect},
  widgets::{Cell, Row, Table},
  Frame,
};

use super::{models::AppResource, ActiveBlock, App};
use crate::{
  network::Network,
  ui::{
    utils::{
      layout_block_active, loading, style_highlight, style_primary, style_secondary,
      table_header_style,
    },
    HIGHLIGHT,
  },
};

#[derive(Clone, Default)]
pub struct KubeContext {
  pub name: String,
  pub cluster: String,
  pub user: Option<String>,
  pub namespace: Option<String>,
  pub is_active: bool,
}

impl KubeContext {
  pub fn from_api(ctx: &NamedContext, is_active: bool) -> Self {
    let def_context = Context::default();
    let context = ctx.context.as_ref().unwrap_or(&def_context);
    KubeContext {
      name: ctx.name.clone(),
      cluster: context.cluster.clone(),
      user: context.user.clone(),
      namespace: context.namespace.clone(),
      is_active,
    }
  }
}

pub fn get_contexts(config: &Kubeconfig, selected_ctx: Option<String>) -> Vec<KubeContext> {
  config
    .contexts
    .iter()
    .map(|ctx| {
      KubeContext::from_api(
        ctx,
        is_active_context(&ctx.name, &config.current_context, selected_ctx.to_owned()),
      )
    })
    .collect::<Vec<KubeContext>>()
}

fn is_active_context(
  name: &str,
  current_ctx: &Option<String>,
  selected_ctx: Option<String>,
) -> bool {
  match selected_ctx {
    Some(ctx) => name == ctx,
    None => match current_ctx {
      Some(ctx) => name == ctx,
      None => false,
    },
  }
}

pub struct ContextResource {}

#[async_trait]
impl AppResource for ContextResource {
  fn render(_block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let title = format!(" Contexts [{}] ", app.data.contexts.items.len());
    let block = layout_block_active(title.as_str(), app.light_theme);

    if !app.data.contexts.items.is_empty() {
      let rows = app.data.contexts.items.iter().map(|c| {
        let style = if c.is_active {
          style_secondary(app.light_theme)
        } else {
          style_primary(app.light_theme)
        };
        Row::new(vec![
          Cell::from(c.name.to_owned()),
          Cell::from(c.cluster.to_owned()),
          Cell::from(c.user.clone().unwrap_or("<none>".to_string())),
        ])
        .style(style)
      });

      let table = Table::new(
        rows,
        [
          Constraint::Percentage(34),
          Constraint::Percentage(33),
          Constraint::Percentage(33),
        ],
      )
      .header(table_header_style(
        vec!["Context", "Cluster", "User"],
        app.light_theme,
      ))
      .block(block)
      .row_highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT);

      f.render_stateful_widget(table, area, &mut app.data.contexts.state);
    } else {
      loading(f, block, area, app.is_loading(), app.light_theme);
    }
  }

  async fn get_resource(_nw: &Network<'_>) {
    // not required
    unimplemented!()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use kube::config::{Context, NamedContext};

  fn make_named_context(name: &str, cluster: &str, namespace: Option<&str>) -> NamedContext {
    NamedContext {
      name: name.to_string(),
      context: Some(Context {
        cluster: cluster.to_string(),
        user: Some("user".to_string()),
        namespace: namespace.map(String::from),
        ..Default::default()
      }),
    }
  }

  #[test]
  fn test_from_api_extracts_namespace() {
    let ctx = make_named_context("prod", "prod-cluster", Some("kube-system"));
    let kube_ctx = KubeContext::from_api(&ctx, true);

    assert_eq!(kube_ctx.name, "prod");
    assert_eq!(kube_ctx.cluster, "prod-cluster");
    assert_eq!(kube_ctx.namespace, Some("kube-system".to_string()));
    assert!(kube_ctx.is_active);
  }

  #[test]
  fn test_from_api_namespace_none_when_absent() {
    let ctx = make_named_context("dev", "dev-cluster", None);
    let kube_ctx = KubeContext::from_api(&ctx, false);

    assert_eq!(kube_ctx.namespace, None);
    assert!(!kube_ctx.is_active);
  }

  #[test]
  fn test_get_contexts_marks_active_from_kubeconfig() {
    let config = Kubeconfig {
      current_context: Some("ctx-b".to_string()),
      contexts: vec![
        make_named_context("ctx-a", "c1", Some("ns-a")),
        make_named_context("ctx-b", "c2", Some("ns-b")),
      ],
      ..Default::default()
    };

    let contexts = get_contexts(&config, None);
    assert_eq!(contexts.len(), 2);
    assert!(!contexts[0].is_active);
    assert!(contexts[1].is_active);
    assert_eq!(contexts[1].namespace, Some("ns-b".to_string()));
  }

  #[test]
  fn test_get_contexts_selected_overrides_current_context() {
    let config = Kubeconfig {
      current_context: Some("ctx-b".to_string()),
      contexts: vec![
        make_named_context("ctx-a", "c1", None),
        make_named_context("ctx-b", "c2", None),
      ],
      ..Default::default()
    };

    let contexts = get_contexts(&config, Some("ctx-a".to_string()));
    assert!(contexts[0].is_active);
    assert!(!contexts[1].is_active);
  }
}
