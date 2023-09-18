use async_trait::async_trait;
use k8s_openapi::{api::core::v1::ServiceAccount, chrono::Utc};
use ratatui::{
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
pub struct KubeSvcAcct {
  pub namespace: String,
  pub name: String,
  pub secrets: i32,
  pub age: String,
  k8s_obj: ServiceAccount,
}

// Get length of a vector
impl From<ServiceAccount> for KubeSvcAcct {
  fn from(acct: ServiceAccount) -> Self {
    KubeSvcAcct {
      namespace: acct.metadata.namespace.clone().unwrap_or_default(),
      name: acct.metadata.name.clone().unwrap_or_default(),
      secrets: acct.secrets.clone().unwrap_or_default().len() as i32,
      age: utils::to_age(acct.metadata.creation_timestamp.as_ref(), Utc::now()),
      k8s_obj: utils::sanitize_obj(acct),
    }
  }
}

impl KubeResource<ServiceAccount> for KubeSvcAcct {
  fn get_name(&self) -> &String {
    &self.name
  }
  fn get_k8s_obj(&self) -> &ServiceAccount {
    &self.k8s_obj
  }
}

static SVC_ACCT_TITLE: &str = "ServiceAccounts";

pub struct SvcAcctResource {}

#[async_trait]
impl AppResource for SvcAcctResource {
  fn render<B: Backend>(block: ActiveBlock, f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      SVC_ACCT_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_block,
      app.data.service_accounts
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeSvcAcct> = nw.get_namespaced_resources(ServiceAccount::into).await;

    let mut app = nw.app.lock().await;
    app.data.service_accounts.set_items(items);
  }
}

fn draw_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let title = get_resource_title(
    app,
    SVC_ACCT_TITLE,
    "",
    app.data.service_accounts.items.len(),
  );

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_YAML_AND_ESC_HINT.into(),
      resource: &mut app.data.service_accounts,
      table_headers: vec!["Namespace", "Name", "Secrets", "Age"],
      column_widths: vec![
        Constraint::Percentage(30),
        Constraint::Percentage(30),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.secrets.to_string()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary(app.light_theme))
    },
    app.light_theme,
    app.is_loading,
    app.data.selected.filter.to_owned(),
  );
}

#[cfg(test)]
mod tests {
  use k8s_openapi::chrono::Utc;

  use crate::app::{
    serviceaccounts::KubeSvcAcct,
    test_utils::{convert_resource_from_file, get_time},
    utils,
  };

  #[test]
  fn test_service_accounts_from_api() {
    let (serviceaccounts, serviceaccounts_list): (Vec<KubeSvcAcct>, Vec<_>) =
      convert_resource_from_file("serviceaccounts");

    assert_eq!(serviceaccounts.len(), 43);
    assert_eq!(
      serviceaccounts[0],
      KubeSvcAcct {
        namespace: "kube-node-lease".to_string(),
        name: "default".into(),
        secrets: 3,
        age: utils::to_age(Some(&get_time("2023-06-30T17:13:19Z")), Utc::now()),
        k8s_obj: serviceaccounts_list[0].clone(),
      }
    )
  }
}
