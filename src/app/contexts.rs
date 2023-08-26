use async_trait::async_trait;
use kube::config::{Context, Kubeconfig, NamedContext};
use tui::{
  backend::Backend,
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
  pub user: String,
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
  fn render<B: Backend>(_block: ActiveBlock, f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
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
          Cell::from(c.name.as_ref()),
          Cell::from(c.cluster.as_ref()),
          Cell::from(c.user.as_ref()),
        ])
        .style(style)
      });

      let table = Table::new(rows)
        .header(table_header_style(
          vec!["Context", "Cluster", "User"],
          app.light_theme,
        ))
        .block(block)
        .widths(&[
          Constraint::Percentage(34),
          Constraint::Percentage(33),
          Constraint::Percentage(33),
        ])
        .highlight_style(style_highlight())
        .highlight_symbol(HIGHLIGHT);

      f.render_stateful_widget(table, area, &mut app.data.contexts.state);
    } else {
      loading(f, block, area, app.is_loading, app.light_theme);
    }
  }

  async fn get_resource(_nw: &Network<'_>) {
    // not required
    unimplemented!()
  }
}
