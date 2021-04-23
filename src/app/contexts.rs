use kube::config::{Kubeconfig, NamedContext};

#[derive(Clone)]
pub struct KubeContext {
  pub name: String,
  pub cluster: String,
  pub user: String,
  pub namespace: Option<String>,
  pub is_active: bool,
}

impl KubeContext {
  pub fn from_api(ctx: &NamedContext, is_active: bool) -> Self {
    KubeContext {
      name: ctx.name.clone(),
      cluster: ctx.context.cluster.clone(),
      user: ctx.context.user.clone(),
      namespace: ctx.context.namespace.clone(),
      is_active,
    }
  }
}

pub fn get_contexts(config: &Kubeconfig) -> Vec<KubeContext> {
  config
    .contexts
    .iter()
    .map(|it| KubeContext::from_api(it, is_active_context(&it.name, &config.current_context)))
    .collect::<Vec<KubeContext>>()
}

fn is_active_context(name: &str, current_ctx: &Option<String>) -> bool {
  match current_ctx {
    Some(ctx) => name == ctx,
    None => false,
  }
}
