use k8s_openapi::{
  api::networking::v1::{Ingress, IngressBackend, IngressRule, IngressStatus},
  chrono::Utc,
};

use super::{
  models::KubeResource,
  utils::{self, UNKNOWN},
};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeIngress {
  pub namespace: String,
  pub name: String,
  pub ingress_class: String,
  pub address: String,
  pub paths: String,
  pub default_backend: String,
  pub age: String,
  k8s_obj: Ingress,
}

impl From<Ingress> for KubeIngress {
  fn from(ingress: Ingress) -> Self {
    let (ingress_class, rules, default_backend) = match &ingress.spec {
      Some(spec) => {
        let class_name = match &spec.ingress_class_name {
          Some(c) => c.clone(),
          None => UNKNOWN.into(),
        };
        (
          class_name,
          get_rules(&spec.rules),
          format_backend(&spec.default_backend),
        )
      }
      None => (String::default(), None, String::default()),
    };
    let name = match &ingress.metadata.name {
      Some(n) => n.clone(),
      None => UNKNOWN.into(),
    };
    let namespace = match &ingress.metadata.namespace {
      Some(n) => n.clone(),
      None => UNKNOWN.into(),
    };
    let paths = match rules {
      Some(r) => r,
      None => String::default(),
    };
    Self {
      name,
      namespace,
      ingress_class,
      address: get_addresses(&ingress.status),
      paths,
      default_backend,
      age: utils::to_age(ingress.metadata.creation_timestamp.as_ref(), Utc::now()),
      k8s_obj: utils::sanitize_obj(ingress),
    }
  }
}

impl KubeResource<Ingress> for KubeIngress {
  fn get_k8s_obj(&self) -> &Ingress {
    &self.k8s_obj
  }
}

fn format_backend(backend: &Option<IngressBackend>) -> String {
  match backend {
    Some(backend) => {
      if let Some(resource) = &backend.resource {
        return resource.name.to_string();
      }
      if let Some(service) = &backend.service {
        match &service.port {
          Some(port) => {
            if let Some(name) = &port.name {
              format!("{}:{}", service.name, name)
            } else if let Some(number) = &port.number {
              return format!("{}:{}", service.name, number);
            } else {
              return String::default();
            }
          }
          None => String::default(),
        }
      } else {
        String::default()
      }
    }
    None => String::default(),
  }
}

fn get_rules(i_rules: &Option<Vec<IngressRule>>) -> Option<String> {
  i_rules.as_ref().map(|rules| {
    rules
      .iter()
      .map(|i_rule| {
        let mut rule = i_rule.host.clone().unwrap_or("*".to_string());
        if let Some(http) = &i_rule.http {
          http.paths.iter().for_each(|path| {
            rule = format!(
              "{}{}►{}",
              rule,
              &path.path.clone().unwrap_or("/*".to_string()),
              format_backend(&Some(path.backend.clone()))
            );
          });
        }
        rule
      })
      .collect::<Vec<_>>()
      .join(" ")
  })
}

fn get_addresses(i_status: &Option<IngressStatus>) -> String {
  match i_status {
    Some(status) => match &status.load_balancer {
      Some(lb) => match &lb.ingress {
        Some(ingress) => ingress
          .iter()
          .map(|i| {
            if let Some(h) = &i.hostname {
              h.to_string()
            } else if let Some(ip) = &i.ip {
              ip.to_string()
            } else {
              "<pending>".to_string()
            }
          })
          .collect::<Vec<_>>()
          .join(" "),
        None => String::default(),
      },
      None => String::default(),
    },
    None => String::default(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_utils::*;

  #[test]
  fn test_ingresses_from_api() {
    let (ingresses, ingress_list): (Vec<KubeIngress>, Vec<_>) =
      convert_resource_from_file("ingress");

    assert_eq!(ingresses.len(), 3);
    assert_eq!(
      ingresses[0],
      KubeIngress {
        name: "ingdefault".into(),
        namespace: "default".into(),
        age: utils::to_age(Some(&get_time("2023-05-24T16:14:32Z")), Utc::now()),
        k8s_obj: ingress_list[0].clone(),
        ingress_class: "default".into(),
        address: "".into(),
        paths: "foo.com/►svc:8080".into(),
        default_backend: "defaultsvc:http".into(),
      }
    );
    assert_eq!(
      ingresses[1],
      KubeIngress {
        name: "test".into(),
        namespace: "default".into(),
        age: utils::to_age(Some(&get_time("2023-05-24T16:20:48Z")), Utc::now()),
        k8s_obj: ingress_list[1].clone(),
        ingress_class: "nginx".into(),
        address: "192.168.49.2".into(),
        paths: "".into(),
        default_backend: "test:5701".into(),
      }
    );
    assert_eq!(
      ingresses[2],
      KubeIngress {
        name: "test-ingress".into(),
        namespace: "dev".into(),
        age: utils::to_age(Some(&get_time("2023-05-24T16:22:23Z")), Utc::now()),
        k8s_obj: ingress_list[2].clone(),
        ingress_class: "nginx".into(),
        address: "192.168.49.2".into(),
        paths: "demo.apps.mlopshub.com/►hello-service:80".into(),
        default_backend: "".into(),
      }
    );
  }
}
