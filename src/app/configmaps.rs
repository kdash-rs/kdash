use std::collections::BTreeMap;

use async_trait::async_trait;
use k8s_openapi::{api::core::v1::ConfigMap, chrono::Utc};
use ratatui::{
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
    draw_describe_block, draw_resource_block, draw_yaml_block, get_describe_active,
    get_resource_title, style_primary, title_with_dual_style, ResourceTableProps, COPY_HINT,
    DESCRIBE_AND_YAML_HINT,
  },
};

#[derive(Clone, PartialEq, Debug)]
pub struct KubeConfigMap {
  pub name: String,
  pub namespace: String,
  pub data: BTreeMap<String, String>,
  pub age: String,
  k8s_obj: ConfigMap,
}

impl From<ConfigMap> for KubeConfigMap {
  fn from(cm: ConfigMap) -> Self {
    Self {
      name: cm.metadata.name.clone().unwrap_or_default(),
      namespace: cm.metadata.namespace.clone().unwrap_or_default(),
      age: utils::to_age(cm.metadata.creation_timestamp.as_ref(), Utc::now()),
      data: cm.data.clone().unwrap_or_default(),
      k8s_obj: utils::sanitize_obj(cm),
    }
  }
}

impl KubeResource<ConfigMap> for KubeConfigMap {
  fn get_name(&self) -> &String {
    &self.name
  }
  fn get_k8s_obj(&self) -> &ConfigMap {
    &self.k8s_obj
  }
}

static CONFIG_MAPS_TITLE: &str = "ConfigMaps";

pub struct ConfigMapResource {}

#[async_trait]
impl AppResource for ConfigMapResource {
  fn render(block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      CONFIG_MAPS_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_block,
      app.data.config_maps
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeConfigMap> = nw.get_namespaced_resources(ConfigMap::into).await;

    let mut app = nw.app.lock().await;
    app.data.config_maps.set_items(items);
  }
}

fn draw_block(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, CONFIG_MAPS_TITLE, "", app.data.config_maps.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_AND_YAML_HINT.into(),
      resource: &mut app.data.config_maps,
      table_headers: vec!["Namespace", "Name", "Data", "Age"],
      column_widths: vec![
        Constraint::Percentage(30),
        Constraint::Percentage(40),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
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
    let (cms, cm_list): (Vec<KubeConfigMap>, Vec<_>) = convert_resource_from_file("cm");

    assert_eq!(cms.len(), 6);
    assert_eq!(
      cms[0],
      KubeConfigMap {
        name: "extension-apiserver-authentication".into(),
        namespace: "kube-system".into(),
        data: map_string_object! {
            "client-ca-file" => "-----BEGIN CERTIFICATE-----\nMIIBdjCCAR2gAwIBAgIBADAKBggqhkjOPQQDAjAjMSEwHwYDVQQDDBhrM3MtY2xp\nZW50LWNhQDE2MjA2ODMyNzkwHhcNMjEwNTEwMjE0NzU5WhcNMzEwNTA4MjE0NzU5\nWjAjMSEwHwYDVQQDDBhrM3MtY2xpZW50LWNhQDE2MjA2ODMyNzkwWTATBgcqhkjO\nPQIBBggqhkjOPQMBBwNCAATQnQ4/3PQe/VdAfIjWoaDxN2vX7hMpHr5uOTW8V+GR\nzRxwLHNB2h4b3bbfDwkCjXg0HJWv4KQB3KyQ1GBND6ZAo0IwQDAOBgNVHQ8BAf8E\nBAMCAqQwDwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4EFgQUy/tsudG9dX/Y1pa+jfnM\nT3yXZfkwCgYIKoZIzj0EAwIDRwAwRAIgdTf7esWYeszzj3riNNqYytXpTsZK3prb\ngGu/jkVqvaICIDQCAM/1NyHHgpdtntOgeDVEeWNomHHb8TZmXbDcx+XX\n-----END CERTIFICATE-----\n".to_string(),
            "requestheader-allowed-names" => "[\"system:auth-proxy\"]".to_string(),
            "requestheader-client-ca-file"=> "-----BEGIN CERTIFICATE-----\nMIIBhzCCAS2gAwIBAgIBADAKBggqhkjOPQQDAjArMSkwJwYDVQQDDCBrM3MtcmVx\ndWVzdC1oZWFkZXItY2FAMTYyMDY4MzI3OTAeFw0yMTA1MTAyMTQ3NTlaFw0zMTA1\nMDgyMTQ3NTlaMCsxKTAnBgNVBAMMIGszcy1yZXF1ZXN0LWhlYWRlci1jYUAxNjIw\nNjgzMjc5MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEJTPbt57KGef9LeZlR2DA\njYUaMdhXM8xnwQW9cIiH5dlJIQaEgOVEEiHYx0EmhSj9nhPxTTBRwiBrTVMembXR\nK6NCMEAwDgYDVR0PAQH/BAQDAgKkMA8GA1UdEwEB/wQFMAMBAf8wHQYDVR0OBBYE\nFPmlBLfAmlxF5H4snC7wB8Zd1iDzMAoGCCqGSM49BAMCA0gAMEUCIQCKwv/4dJH9\nJqtnv35balKAtG+bJyIVop73W18t9ZaatAIgU3h5i3IQp+cgVjQhi4ZvRXUA6c3l\nDSZ1C2gMf1ONFJQ=\n-----END CERTIFICATE-----\n".to_string(),
            "requestheader-extra-headers-prefix"=> "[\"X-Remote-Extra-\"]".to_string(),
            "requestheader-group-headers"=> "[\"X-Remote-Group\"]".to_string(),
            "requestheader-username-headers" => "[\"X-Remote-User\"]".to_string()
        },
        age: utils::to_age(Some(&get_time("2021-05-10T21:48:02Z")), Utc::now()),
        k8s_obj: cm_list[0].clone()
      }
    );
    assert_eq!(
      cms[1],
      KubeConfigMap {
        name: "cluster-dns".into(),
        namespace: "kube-system".into(),
        data: map_string_object! {"clusterDNS" => "10.43.0.10".to_string(), "clusterDomain" => "cluster.local".to_string()},
        age: utils::to_age(Some(&get_time("2021-05-10T21:48:06Z")), Utc::now()),
        k8s_obj: cm_list[1].clone()
      }
    );
    assert_eq!(
      cms[2],
      KubeConfigMap {
        name: "local-path-config".into(),
        namespace: "kube-system".into(),
        data: map_string_object! {
        "config.json"=> "{\n  \"nodePathMap\":[\n  {\n    \"node\":\"DEFAULT_PATH_FOR_NON_LISTED_NODES\",\n    \"paths\":[\"/var/lib/rancher/k3s/storage\"]\n  }\n  ]\n}".to_string(),
        "helperPod.yaml"=> "apiVersion: v1\nkind: Pod\nmetadata:\n  name: helper-pod\nspec:\n  containers:\n  - name: helper-pod\n    image: rancher/library-busybox:1.32.1".to_string(),
        "setup"=> "#!/bin/sh\nwhile getopts \"m:s:p:\" opt\ndo\n    case $opt in\n        p)\n        absolutePath=$OPTARG\n        ;;\n        s)\n        sizeInBytes=$OPTARG\n        ;;\n        m)\n        volMode=$OPTARG\n        ;;\n    esac\ndone\nmkdir -m 0777 -p ${absolutePath}".to_string(),
        "teardown"=> "#!/bin/sh\nwhile getopts \"m:s:p:\" opt\ndo\n    case $opt in\n        p)\n        absolutePath=$OPTARG\n        ;;\n        s)\n        sizeInBytes=$OPTARG\n        ;;\n        m)\n        volMode=$OPTARG\n        ;;\n    esac\ndone\nrm -rf ${absolutePath}".to_string()
        },
        age: utils::to_age(Some(&get_time("2021-05-10T21:48:06Z")), Utc::now()),
        k8s_obj: cm_list[2].clone()
      }
    );
    assert_eq!(
      cms[3],
      KubeConfigMap {
        name: "chart-content-traefik".into(),
        namespace: "kube-system".into(),
        data: map_string_object! {},
        age: utils::to_age(Some(&get_time("2021-05-10T21:48:06Z")), Utc::now()),
        k8s_obj: cm_list[3].clone()
      }
    );
    assert_eq!(
      cms[4],
      KubeConfigMap {
        name: "kube-root-ca.crt".into(),
        namespace: "default".into(),
        data: map_string_object! {
            "ca.crt"=> "-----BEGIN CERTIFICATE-----\nMIIBeDCCAR2gAwIBAgIBADAKBggqhkjOPQQDAjAjMSEwHwYDVQQDDBhrM3Mtc2Vy\ndmVyLWNhQDE2MjA2ODMyNzkwHhcNMjEwNTEwMjE0NzU5WhcNMzEwNTA4MjE0NzU5\nWjAjMSEwHwYDVQQDDBhrM3Mtc2VydmVyLWNhQDE2MjA2ODMyNzkwWTATBgcqhkjO\nPQIBBggqhkjOPQMBBwNCAAR5RMn6pUUsQDIRhe0lEtKcBnXfOVhby2gu6lrOtVE6\nYMvSHyUKsOpye4vv5qxJG851ujHdAgchXAAI4WqyEiU5o0IwQDAOBgNVHQ8BAf8E\nBAMCAqQwDwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4EFgQU5KVzmsOvs4GYd9M4lW4q\nwZfQ2UwwCgYIKoZIzj0EAwIDSQAwRgIhAOqpzcIevpC2nUO9yes9VJmF3ij3s7B2\n4rJrJp9ZXJGwAiEA325UlEKQGC/JuVD1HzhgLLQTgfaXB7E4p/JShnIrrfk=\n-----END CERTIFICATE-----\n".to_string()
        },
        age: utils::to_age(Some(&get_time("2021-05-10T21:48:19Z")), Utc::now()),
        k8s_obj: cm_list[4].clone()
      }
    );
    assert_eq!(
      cms[5],
      KubeConfigMap {
        name: "traefik-test".into(),
        namespace: "kube-system".into(),
        data: map_string_object! {"run.sh"=> "@test \"Test Access\" {\n  curl -D - http://traefik/\n}".to_string()},
        age: utils::to_age(Some(&get_time("2021-05-10T21:48:35Z")), Utc::now()),
        k8s_obj: cm_list[5].clone()
      }
    );
  }
}
