use super::utils::{self, UNKNOWN};
use k8s_openapi::{
  api::core::v1::{Service, ServicePort},
  chrono::Utc,
};

#[derive(Clone)]
pub struct KubeSvs {
  pub namespace: String,
  pub name: String,
  pub type_: String,
  pub cluster_ip: String,
  pub external_ip: String,
  pub ports: String,
  pub age: String,
}

impl KubeSvs {
  pub fn from_api(service: &Service) -> Self {
    let (type_, cluster_ip, external_ip, ports) = match &service.spec {
      Some(spec) => {
        let type_ = match &spec.type_ {
          Some(type_) => type_.clone(),
          _ => UNKNOWN.into(),
        };

        let external_ips = match type_.as_str() {
          "ClusterIP" | "NodePort" => spec.external_ips.clone(),
          "LoadBalancer" => get_lb_ext_ips(service, spec.external_ips.clone()),
          "ExternalName" => Some(vec![spec.external_name.clone().unwrap_or_default()]),
          _ => None,
        }
        .unwrap_or_else(|| {
          if type_ == "LoadBalancer" {
            vec!["<pending>".into()]
          } else {
            vec![String::default()]
          }
        });

        (
          type_,
          spec.cluster_ip.as_ref().unwrap_or(&"None".into()).clone(),
          external_ips.join(","),
          get_ports(spec.ports.clone()).join(" "),
        )
      }
      _ => (
        UNKNOWN.into(),
        String::default(),
        String::default(),
        String::default(),
      ),
    };

    KubeSvs {
      name: service.metadata.name.clone().unwrap_or_default(),
      type_,
      namespace: service.metadata.namespace.clone().unwrap_or_default(),
      cluster_ip,
      external_ip,
      ports,
      age: utils::to_age(service.metadata.creation_timestamp.as_ref(), Utc::now()),
    }
  }
}

fn get_ports(sports: Option<Vec<ServicePort>>) -> Vec<String> {
  match sports {
    Some(ports) => ports
      .iter()
      .map(|s| {
        let mut port = String::new();
        if s.name.is_some() {
          port = format!("{}:", s.name.clone().unwrap());
        }
        port = format!("{}{}►{}", port, s.port, s.node_port.unwrap_or(0));
        if s.protocol.is_some() && s.protocol.clone().unwrap() == "TCP" {
          port = format!("{}/{}", port, s.protocol.clone().unwrap());
        }
        port
      })
      .collect(),
    None => vec![],
  }
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
          .map(|it| {
            if it.ip.is_some() {
              it.ip.clone().unwrap_or_default()
            } else if it.hostname.is_some() {
              it.hostname.clone().unwrap_or_default()
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
  if external_ips.is_some() && !lb_ips.is_empty() {
    lb_ips.extend(external_ips.unwrap_or_default());
    Some(lb_ips)
  } else if !lb_ips.is_empty() {
    Some(lb_ips)
  } else {
    None
  }
}
