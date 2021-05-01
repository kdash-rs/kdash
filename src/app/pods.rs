use super::utils::{self, UNKNOWN};
use k8s_openapi::{
  api::core::v1::{ContainerState, ContainerStateWaiting, Pod, PodStatus},
  chrono::Utc,
};

#[derive(Clone, Default, Debug, PartialEq)]
pub struct KubePod {
  pub namespace: String,
  pub name: String,
  pub ready: String,
  pub status: String,
  pub restarts: i32,
  pub cpu: String,
  pub mem: String,
  pub age: String,
  pub containers: Vec<KubeContainer>,
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct KubeContainer {
  pub name: String,
  pub image: String,
  pub ready: String,
  pub status: String,
  pub restarts: i32,
  pub liveliness_probe: bool,
  pub readiness_probe: bool,
  pub ports: String,
  pub age: String,
  pub pod_name: String,
}

impl KubePod {
  pub fn from_api(pod: &Pod) -> Self {
    let age = utils::to_age(pod.metadata.creation_timestamp.as_ref(), Utc::now());
    let pod_name = pod.metadata.name.clone().unwrap_or_default();
    let (status, cr, restarts, c_stats_len, containers) = match &pod.status {
      Some(status) => {
        let (mut cr, mut rc) = (0, 0);
        let c_stats_len = match status.container_statuses.as_ref() {
          Some(c_stats) => {
            c_stats.iter().for_each(|cs| {
              if cs.ready {
                cr += 1;
              }
              rc += cs.restart_count;
            });
            c_stats.len()
          }
          None => 0,
        };

        let containers = status
          .container_statuses
          .as_ref()
          .unwrap_or(&vec![])
          .iter()
          .map(|cs| KubeContainer {
            name: cs.name.clone(),
            pod_name: pod_name.clone(),
            image: cs.image.clone(),
            ready: cs.ready.to_string(),
            status: get_container_state(cs.state.clone()),
            restarts: cs.restart_count,
            liveliness_probe: false, //TODO
            readiness_probe: false,  //TODO
            ports: "".to_owned(),    //TODO
            age: age.clone(),
          })
          .collect();

        (get_status(status, pod), cr, rc, c_stats_len, containers)
      }
      _ => (UNKNOWN.into(), 0, 0, 0, vec![]),
    };

    KubePod {
      name: pod_name,
      namespace: pod.metadata.namespace.clone().unwrap_or_default(),
      ready: format!("{}/{}", cr, c_stats_len),
      restarts,
      // TODO implement pod metrics
      cpu: String::default(),
      mem: String::default(),
      status,
      age,
      containers,
    }
  }
}

fn get_container_state(os: Option<ContainerState>) -> String {
  match os {
    Some(s) => {
      if let Some(sw) = s.waiting {
        sw.reason.unwrap_or_else(|| "Waiting".into())
      } else if let Some(st) = s.terminated {
        st.reason.unwrap_or_else(|| "Terminating".into())
      } else if s.running.is_some() {
        "Running".into()
      } else {
        "<none>".into()
      }
    }
    None => "<none>".into(),
  }
}

fn get_status(stat: &PodStatus, pod: &Pod) -> String {
  let status = match &stat.phase {
    Some(phase) => phase.to_owned(),
    _ => UNKNOWN.into(),
  };
  let status = match &stat.reason {
    Some(reason) => {
      if reason == "NodeLost" && pod.metadata.deletion_timestamp.is_some() {
        UNKNOWN.into()
      } else {
        reason.to_owned()
      }
    }
    None => status,
  };

  // get int container status
  let status = match &stat.init_container_statuses {
    Some(ics) => {
      for (i, cs) in ics.iter().enumerate() {
        let status = match &cs.state {
          Some(s) => {
            if let Some(st) = &s.terminated {
              if st.exit_code == 0 {
                "".into()
              } else if st.reason.as_ref().unwrap_or(&String::default()).is_empty() {
                format!("Init:{}", st.reason.as_ref().unwrap())
              } else if st.signal.unwrap_or_default() != 0 {
                format!("Init:Signal:{}", st.signal.unwrap())
              } else {
                format!("Init:ExitCode:{}", st.exit_code)
              }
            } else if is_pod_init(s.waiting.clone()) {
              format!(
                "Init:{}",
                s.waiting
                  .as_ref()
                  .unwrap()
                  .reason
                  .as_ref()
                  .unwrap_or(&String::default())
              )
            } else {
              format!(
                "Init:{}/{}",
                i,
                pod
                  .spec
                  .as_ref()
                  .and_then(|ps| ps.init_containers.as_ref().map(|pic| pic.len()))
                  .unwrap_or(0)
              )
            }
          }
          None => "".into(),
        };
        if !status.is_empty() {
          return status;
        }
      }
      status
    }
    None => status,
  };

  let (mut status, running) = match &stat.container_statuses {
    Some(css) => {
      let mut running = false;
      let status = css
        .iter()
        .rev()
        .find_map(|cs| {
          cs.state.as_ref().and_then(|s| {
            if cs.ready && s.running.is_some() {
              running = true;
            }
            if s
              .waiting
              .as_ref()
              .and_then(|w| w.reason.as_ref().map(|v| !v.is_empty()))
              .unwrap_or_default()
            {
              s.waiting.as_ref().and_then(|w| w.reason.clone())
            } else if s
              .terminated
              .as_ref()
              .and_then(|w| w.reason.as_ref().map(|v| !v.is_empty()))
              .unwrap_or_default()
            {
              s.terminated.as_ref().and_then(|w| w.reason.clone())
            } else if let Some(st) = &s.terminated {
              if st.signal.unwrap_or_default() != 0 {
                Some(format!("Signal:{}", st.signal.unwrap_or_default()))
              } else {
                Some(format!("ExitCode:{}", st.exit_code))
              }
            } else {
              Some(status.clone())
            }
          })
        })
        .unwrap_or_default();
      (status, running)
    }
    None => (status, false),
  };

  if running && status == "Completed" {
    status = "Running".into();
  }

  if pod.metadata.deletion_timestamp.is_none() {
    return status;
  }

  "Terminating".into()
}

fn is_pod_init(sw: Option<ContainerStateWaiting>) -> bool {
  sw.map(|w| w.reason.unwrap_or_default() != "PodInitializing")
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
  use super::*;
  use k8s_openapi::{
    apimachinery::pkg::apis::meta::v1::Time,
    chrono::{DateTime, TimeZone},
  };
  use kube::api::ObjectList;
  use std::fs;

  #[test]
  fn test_to_mem_percent() {
    fn get_time(s: &str) -> Time {
      Time(to_utc(s))
    }

    fn to_utc(s: &str) -> DateTime<Utc> {
      Utc.datetime_from_str(s, "%Y-%m-%dT%H:%M:%SZ").unwrap()
    }

    let pods_yaml =
      fs::read_to_string("./test_data/pods.yaml").expect("Something went wrong reading pods.yaml");
    assert_ne!(pods_yaml, "".to_string());

    let pods: serde_yaml::Result<ObjectList<Pod>> = serde_yaml::from_str(&*pods_yaml);
    assert_eq!(pods.is_ok(), true);

    let pods: Vec<KubePod> = pods
      .unwrap()
      .iter()
      .map(|it| KubePod::from_api(it))
      .collect::<Vec<_>>();

    assert_eq!(pods.len(), 11);
    assert_eq!(
      pods[0],
      KubePod {
        namespace: "default".into(),
        name: "adservice-f787c8dcd-tb6x2".into(),
        ready: "0/0".into(),
        status: "Pending".into(),
        restarts: 0,
        cpu: "".into(),
        mem: "".into(),
        age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
        containers: vec![]
      }
    );
    assert_eq!(
      pods[1],
      KubePod {
        namespace: "default".into(),
        name: "cartservice-67b89ffc69-s5qp8".into(),
        ready: "0/1".into(),
        status: "CrashLoopBackOff".into(),
        restarts: 896,
        cpu: "".into(),
        mem: "".into(),
        age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
        containers: vec![KubeContainer {
          name: "server".into(),
          image: "gcr.io/google-samples/microservices-demo/cartservice:v0.2.2".into(),
          ready: "false".into(),
          status: "CrashLoopBackOff".into(),
          restarts: 896,
          liveliness_probe: false,
          readiness_probe: false,
          ports: "".into(), //   ports: "7070".into(), // TODO fix
          age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
          pod_name: "cartservice-67b89ffc69-s5qp8".into()
        }]
      }
    );
    assert_eq!(
      pods[3],
      KubePod {
        namespace: "default".into(),
        name: "emailservice-5f8fc7dbb4-5lqdb".into(),
        ready: "1/1".into(),
        status: "Running".into(),
        restarts: 3,
        cpu: "".into(),
        mem: "".into(),
        age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
        containers: vec![KubeContainer {
          name: "server".into(),
          image: "gcr.io/google-samples/microservices-demo/emailservice:v0.2.2".into(),
          ready: "true".into(),
          status: "Running".into(),
          restarts: 3,
          liveliness_probe: false,
          readiness_probe: false,
          ports: "".into(), //   ports: "7070".into(), // TODO fix
          age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
          pod_name: "emailservice-5f8fc7dbb4-5lqdb".into()
        }]
      }
    );
    assert_eq!(
      pods[4],
      KubePod {
        namespace: "default".into(),
        name: "frontend-5c4745dfdb-6k8wf".into(),
        ready: "0/0".into(),
        status: "OutOfcpu".into(),
        restarts: 0,
        cpu: "".into(),
        mem: "".into(),
        age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
        containers: vec![]
      }
    );
    assert_eq!(
      pods[5],
      KubePod {
        namespace: "default".into(),
        name: "frontend-5c4745dfdb-qz7fg".into(),
        ready: "0/0".into(),
        status: "Preempting".into(),
        restarts: 0,
        cpu: "".into(),
        mem: "".into(),
        age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
        containers: vec![]
      }
    );
    assert_eq!(
      pods[6],
      KubePod {
        namespace: "default".into(),
        name: "frontend-5c4745dfdb-6k8wf".into(),
        ready: "0/0".into(),
        status: "Failed".into(),
        restarts: 0,
        cpu: "".into(),
        mem: "".into(),
        age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
        containers: vec![]
      }
    );
    // TODO add tests for init-container-statuses and NodeLost cases
  }
}
