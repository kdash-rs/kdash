use async_trait::async_trait;
use k8s_openapi::{
  api::core::v1::{Service, ServicePort},
  chrono::Utc,
};
use ratatui::{
  backend::Backend,
  layout::{Constraint, Rect},
  widgets::{Cell, Row},
  Frame,
};

use super::{
  models::{AppResource, KubeResource},
  utils::{self, UNKNOWN},
  ActiveBlock, App,
};
use crate::{
  draw_resource_tab,
  network::Network,
  ui::utils::{
    draw_describe_block, draw_resource_block, get_describe_active, get_resource_title,
    style_primary, title_with_dual_style, ResourceTableProps, COPY_HINT, DESCRIBE_AND_YAML_HINT,
  },
};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeSvc {
  pub namespace: String,
  pub name: String,
  pub type_: String,
  pub cluster_ip: String,
  pub external_ip: String,
  pub ports: String,
  pub age: String,
  k8s_obj: Service,
}

impl From<Service> for KubeSvc {
  fn from(service: Service) -> Self {
    let (type_, cluster_ip, external_ip, ports) = match &service.spec {
      Some(spec) => {
        let type_ = match &spec.type_ {
          Some(type_) => type_.clone(),
          _ => UNKNOWN.into(),
        };

        let external_ips = match type_.as_str() {
          "ClusterIP" | "NodePort" => spec.external_ips.clone(),
          "LoadBalancer" => get_lb_ext_ips(&service, spec.external_ips.clone()),
          "ExternalName" => Some(vec![spec.external_name.clone().unwrap_or_default()]),
          _ => None,
        };

        (
          type_,
          spec.cluster_ip.as_ref().unwrap_or(&"None".into()).clone(),
          external_ips.unwrap_or_default().join(","),
          get_ports(&spec.ports).unwrap_or_default(),
        )
      }
      _ => (
        UNKNOWN.into(),
        String::default(),
        String::default(),
        String::default(),
      ),
    };

    KubeSvc {
      name: service.metadata.name.clone().unwrap_or_default(),
      type_,
      namespace: service.metadata.namespace.clone().unwrap_or_default(),
      cluster_ip,
      external_ip,
      ports,
      age: utils::to_age(service.metadata.creation_timestamp.as_ref(), Utc::now()),
      k8s_obj: utils::sanitize_obj(service),
    }
  }
}

impl KubeResource<Service> for KubeSvc {
  fn get_name(&self) -> &String {
    &self.name
  }
  fn get_k8s_obj(&self) -> &Service {
    &self.k8s_obj
  }
}

static SERVICES_TITLE: &str = "Services";

pub struct SvcResource {}

#[async_trait]
impl AppResource for SvcResource {
  fn render<B: Backend>(block: ActiveBlock, f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      SERVICES_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_block,
      app.data.services
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeSvc> = nw.get_namespaced_resources(Service::into).await;
    let mut app = nw.app.lock().await;
    app.data.services.set_items(items);
  }
}

fn draw_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, SERVICES_TITLE, "", app.data.services.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_AND_YAML_HINT.into(),
      resource: &mut app.data.services,
      table_headers: vec![
        "Namespace",
        "Name",
        "Type",
        "Cluster IP",
        "External IP",
        "Ports",
        "Age",
      ],
      column_widths: vec![
        Constraint::Percentage(10),
        Constraint::Percentage(25),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.type_.to_owned()),
        Cell::from(c.cluster_ip.to_owned()),
        Cell::from(c.external_ip.to_owned()),
        Cell::from(c.ports.to_owned()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary(app.light_theme))
    },
    app.light_theme,
    app.is_loading,
    app.data.selected.filter.to_owned(),
  );
}

fn get_ports(s_ports: &Option<Vec<ServicePort>>) -> Option<String> {
  s_ports.as_ref().map(|ports| {
    ports
      .iter()
      .map(|s_port| {
        let mut port = String::new();
        if let Some(name) = s_port.name.clone() {
          port = format!("{}:", name);
        }
        port = format!("{}{}►{}", port, s_port.port, s_port.node_port.unwrap_or(0));
        if let Some(protocol) = s_port.protocol.clone() {
          if protocol != "TCP" {
            port = format!("{}/{}", port, s_port.protocol.clone().unwrap());
          }
        }
        port
      })
      .collect::<Vec<_>>()
      .join(" ")
  })
}

fn get_lb_ext_ips(service: &Service, external_ips: Option<Vec<String>>) -> Option<Vec<String>> {
  let mut lb_ips = match &service.status {
    Some(ss) => match &ss.load_balancer {
      Some(lb) => {
        let ing = &lb.ingress;
        ing
          .clone()
          .unwrap_or_default()
          .iter()
          .map(|lb_ing| {
            if lb_ing.ip.is_some() {
              lb_ing.ip.clone().unwrap_or_default()
            } else if lb_ing.hostname.is_some() {
              lb_ing.hostname.clone().unwrap_or_default()
            } else {
              String::default()
            }
          })
          .collect::<Vec<String>>()
      }
      None => vec![],
    },
    None => vec![],
  };
  if external_ips.is_none() && !lb_ips.is_empty() {
    lb_ips.extend(external_ips.unwrap_or_default());
    Some(lb_ips)
  } else if !lb_ips.is_empty() {
    Some(lb_ips)
  } else {
    Some(vec!["<pending>".into()])
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_utils::*;

  #[test]
  fn test_services_from_api() {
    let (svcs, svc_list): (Vec<KubeSvc>, Vec<_>) = convert_resource_from_file("svcs");

    assert_eq!(svcs.len(), 5);
    assert_eq!(
      svcs[0],
      KubeSvc {
        name: "kubernetes".into(),
        namespace: "default".into(),
        age: utils::to_age(Some(&get_time("2021-05-10T21:48:03Z")), Utc::now()),
        k8s_obj: svc_list[0].clone(),
        type_: "ClusterIP".into(),
        cluster_ip: "10.43.0.1".into(),
        external_ip: "".into(),
        ports: "https:443►0".into(),
      }
    );
    assert_eq!(
      svcs[1],
      KubeSvc {
        name: "kube-dns".into(),
        namespace: "kube-system".into(),
        age: utils::to_age(Some(&get_time("2021-05-10T21:48:03Z")), Utc::now()),
        k8s_obj: svc_list[1].clone(),
        type_: "ClusterIP".into(),
        cluster_ip: "10.43.0.10".into(),
        external_ip: "".into(),
        ports: "dns:53►0/UDP dns-tcp:53►0 metrics:9153►0".into(),
      }
    );
    assert_eq!(
      svcs[2],
      KubeSvc {
        name: "metrics-server".into(),
        namespace: "kube-system".into(),
        age: utils::to_age(Some(&get_time("2021-05-10T21:48:03Z")), Utc::now()),
        k8s_obj: svc_list[2].clone(),
        type_: "ClusterIP".into(),
        cluster_ip: "10.43.93.186".into(),
        external_ip: "".into(),
        ports: "443►0".into(),
      }
    );
    assert_eq!(
      svcs[3],
      KubeSvc {
        name: "traefik-prometheus".into(),
        namespace: "kube-system".into(),
        age: utils::to_age(Some(&get_time("2021-05-10T21:48:35Z")), Utc::now()),
        k8s_obj: svc_list[3].clone(),
        type_: "ClusterIP".into(),
        cluster_ip: "10.43.9.106".into(),
        external_ip: "".into(),
        ports: "metrics:9100►0".into(),
      }
    );
    assert_eq!(
      svcs[4],
      KubeSvc {
        name: "traefik".into(),
        namespace: "kube-system".into(),
        age: utils::to_age(Some(&get_time("2021-05-10T21:48:35Z")), Utc::now()),
        k8s_obj: svc_list[4].clone(),
        type_: "LoadBalancer".into(),
        cluster_ip: "10.43.235.227".into(),
        external_ip: "172.20.0.2".into(),
        ports: "http:80►30723 https:443►31954".into(),
      }
    );
  }
}
