use kube::config::{Context, Kubeconfig, NamedContext};

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
