use std::collections::BTreeMap;

use async_trait::async_trait;
use k8s_openapi::{api::core::v1::Secret, chrono::Utc, ByteString};
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
    draw_describe_block, draw_resource_block, get_describe_active, get_resource_title,
    style_primary, title_with_dual_style, ResourceTableProps, COPY_HINT,
    DESCRIBE_YAML_DECODE_AND_ESC_HINT,
  },
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct KubeSecret {
  pub name: String,
  pub namespace: String,
  pub type_: String,
  pub data: BTreeMap<String, ByteString>,
  pub age: String,
  k8s_obj: Secret,
}

impl From<Secret> for KubeSecret {
  fn from(secret: Secret) -> Self {
    KubeSecret {
      name: secret.metadata.name.clone().unwrap_or_default(),
      namespace: secret.metadata.namespace.clone().unwrap_or_default(),
      type_: secret.type_.clone().unwrap_or_default(),
      age: utils::to_age(secret.metadata.creation_timestamp.as_ref(), Utc::now()),
      data: secret.data.clone().unwrap_or_default(),
      k8s_obj: utils::sanitize_obj(secret),
    }
  }
}

impl KubeResource<Secret> for KubeSecret {
  fn get_name(&self) -> &String {
    &self.name
  }
  fn get_k8s_obj(&self) -> &Secret {
    &self.k8s_obj
  }
}

static SECRETS_TITLE: &str = "Secrets";

pub struct SecretResource {}

#[async_trait]
impl AppResource for SecretResource {
  fn render<B: Backend>(block: ActiveBlock, f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      SECRETS_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_block,
      app.data.secrets
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeSecret> = nw.get_namespaced_resources(Secret::into).await;

    let mut app = nw.app.lock().await;
    app.data.secrets.set_items(items);
  }
}

fn draw_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, SECRETS_TITLE, "", app.data.secrets.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_YAML_DECODE_AND_ESC_HINT.into(),
      resource: &mut app.data.secrets,
      table_headers: vec!["Namespace", "Name", "Type", "Data", "Age"],
      column_widths: vec![
        Constraint::Percentage(25),
        Constraint::Percentage(30),
        Constraint::Percentage(25),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.type_.to_owned()),
        Cell::from(c.data.len().to_string()),
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

  use super::*;
  use crate::{app::test_utils::*, map_string_object};

  #[test]
  fn test_config_map_from_api() {
    let (secrets, secret_list): (Vec<KubeSecret>, Vec<_>) = convert_resource_from_file("secrets");

    assert_eq!(secrets.len(), 2);
    assert_eq!(
      secrets[0],
      KubeSecret {
        name: "default-token-rxd8v".into(),
        namespace: "kube-public".into(),
        type_: "kubernetes.io/service-account-token".into(),
        data: map_string_object! {
            "ca.crt" => ByteString("-----BEGIN CERTIFICATE-----\nMIIBeDCCAR2gAwIBAgIBADAKBggqhkjOPQQDAjAjMSEwHwYDVQQDDBhrM3Mtc2Vy\ndmVyLWNhQDE2MjU0Nzc3NTkwHhcNMjEwNzA1MDkzNTU5WhcNMzEwNzAzMDkzNTU5\nWjAjMSEwHwYDVQQDDBhrM3Mtc2VydmVyLWNhQDE2MjU0Nzc3NTkwWTATBgcqhkjO\nPQIBBggqhkjOPQMBBwNCAARI13csf0c5dEbU/0cnZipIrttsmn5UJFUwdLy8ONw0\nFUoK57PeVI6gmqNtnoycpja9n/SuJA+lWqqPNogbiQO7o0IwQDAOBgNVHQ8BAf8E\nBAMCAqQwDwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4EFgQUQJAgrpGYs7lMKt9PWrjh\nyRuhaKwwCgYIKoZIzj0EAwIDSQAwRgIhAIS7h2bW4seeELupl6JhXWgicJK15Jbl\nAAdjs5mfHccqAiEAmaVLQt2V50C8ZLOsR5Lf3FlFH7qpFt3RMto0peGFqB4=\n-----END CERTIFICATE-----\n".as_bytes().into()),
            "namespace" => ByteString("kube-public".as_bytes().into()),
            "token" => ByteString("eyJhbGciOiJSUzI1NiIsImtpZCI6Imp0U29OeTE4V0FrdC1FUDU5N05RaUdBQVZkdHdZT1k3dW5rbGVLWDhjME0ifQ.eyJpc3MiOiJrdWJlcm5ldGVzL3NlcnZpY2VhY2NvdW50Iiwia3ViZXJuZXRlcy5pby9zZXJ2aWNlYWNjb3VudC9uYW1lc3BhY2UiOiJrdWJlLXB1YmxpYyIsImt1YmVybmV0ZXMuaW8vc2VydmljZWFjY291bnQvc2VjcmV0Lm5hbWUiOiJkZWZhdWx0LXRva2VuLXJ4ZDh2Iiwia3ViZXJuZXRlcy5pby9zZXJ2aWNlYWNjb3VudC9zZXJ2aWNlLWFjY291bnQubmFtZSI6ImRlZmF1bHQiLCJrdWJlcm5ldGVzLmlvL3NlcnZpY2VhY2NvdW50L3NlcnZpY2UtYWNjb3VudC51aWQiOiJkNGJjYjVkOC1jYjU1LTRiYzEtYTdmZC0xNzNlOTIwOTc2MDIiLCJzdWIiOiJzeXN0ZW06c2VydmljZWFjY291bnQ6a3ViZS1wdWJsaWM6ZGVmYXVsdCJ9.Hmq3BhdapUIds2MKpTNtnl_yq5jTYpodeJ-MJD6IpdsWLUnhvktLuDy-yfDgMo_55XOOSp4MaTqbt07unh_yGrGacmd8o7PzMTiPDkONnGjN3XJUB2jg5ww9XQ0C5C_wcOzgOO4nrPMisYDsDGc_DNzZf5FwBM6z-x93OLq2URVfv-vv4ceC05d-1TSDLEyT51LqvJ9u0M7qinYbzJsizdW8UM6mc56Ma52gSELC5DljZVugXL9Hoj7nD6ZAUHdjrxdrqk0mVKNeZQKEmbLJXsGGg3c-fv6EO462AvlQvE0gXa-TrwIUvesAxG4fT6D1c0O17n0RNp76meAfCGOu0w".as_bytes().into()),
        },
        age: utils::to_age(Some(&get_time("2021-07-05T09:36:17Z")), Utc::now()),
        k8s_obj: secret_list[0].clone()
      }
    );
    assert_eq!(
      secrets[1],
      KubeSecret {
        name: "default-token-rrxdm".into(),
        namespace: "default".into(),
        type_: "kubernetes.io/service-account-token".into(),
        data: map_string_object! {
            "ca.crt" => ByteString("-----BEGIN CERTIFICATE-----\nMIIBeDCCAR2gAwIBAgIBADAKBggqhkjOPQQDAjAjMSEwHwYDVQQDDBhrM3Mtc2Vy\ndmVyLWNhQDE2MjU0Nzc3NTkwHhcNMjEwNzA1MDkzNTU5WhcNMzEwNzAzMDkzNTU5\nWjAjMSEwHwYDVQQDDBhrM3Mtc2VydmVyLWNhQDE2MjU0Nzc3NTkwWTATBgcqhkjO\nPQIBBggqhkjOPQMBBwNCAARI13csf0c5dEbU/0cnZipIrttsmn5UJFUwdLy8ONw0\nFUoK57PeVI6gmqNtnoycpja9n/SuJA+lWqqPNogbiQO7o0IwQDAOBgNVHQ8BAf8E\nBAMCAqQwDwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4EFgQUQJAgrpGYs7lMKt9PWrjh\nyRuhaKwwCgYIKoZIzj0EAwIDSQAwRgIhAIS7h2bW4seeELupl6JhXWgicJK15Jbl\nAAdjs5mfHccqAiEAmaVLQt2V50C8ZLOsR5Lf3FlFH7qpFt3RMto0peGFqB4=\n-----END CERTIFICATE-----\n".as_bytes().into()),
            "namespace" => ByteString("default".as_bytes().into()),
            "token" => ByteString("eyJhbGciOiJSUzI1NiIsImtpZCI6Imp0U29OeTE4V0FrdC1FUDU5N05RaUdBQVZkdHdZT1k3dW5rbGVLWDhjME0ifQ.eyJpc3MiOiJrdWJlcm5ldGVzL3NlcnZpY2VhY2NvdW50Iiwia3ViZXJuZXRlcy5pby9zZXJ2aWNlYWNjb3VudC9uYW1lc3BhY2UiOiJkZWZhdWx0Iiwia3ViZXJuZXRlcy5pby9zZXJ2aWNlYWNjb3VudC9zZWNyZXQubmFtZSI6ImRlZmF1bHQtdG9rZW4tcnJ4ZG0iLCJrdWJlcm5ldGVzLmlvL3NlcnZpY2VhY2NvdW50L3NlcnZpY2UtYWNjb3VudC5uYW1lIjoiZGVmYXVsdCIsImt1YmVybmV0ZXMuaW8vc2VydmljZWFjY291bnQvc2VydmljZS1hY2NvdW50LnVpZCI6IjFmZDFiOTc1LTlmZTEtNDdiOC1iNzE3LTIwZmY5ODI5OTBlMyIsInN1YiI6InN5c3RlbTpzZXJ2aWNlYWNjb3VudDpkZWZhdWx0OmRlZmF1bHQifQ.6Tccp-EaoyVcP6y3VaJLGSDpYtYdXBdwhO26G7FxdIksRrRPAi6CgQw52FO8-mvJP3L3GRpbs34yzMBYmoYeVwjZ3UFL51I8exL332g9PbEs85Fafq8WkhNylnsYnZk0nJ81Wj-53_AkRl0Bt0f4Q4tU9EJUOl2uRjZWYyQmB91M_8vzCNSKNjUMwjRabPVXJzg8sY8JR0xuY7dZlc5h7gNP7HJFX0AyqKuFTqsG8Crb3tixC0bXhyXa_dM04SjXz_OCfLC-vZBOzQ5E1lPBzm3nhuZIQrr_eZaJJYgw7CieYe2qq2QmwXTve-0_n3LgUNDUcKMp-BUQbm6zxXJqBA".as_bytes().into()),
        },
        age: utils::to_age(Some(&get_time("2021-07-05T09:36:17Z")), Utc::now()),
        k8s_obj: secret_list[1].clone()
      }
    );
  }
}
