use super::utils::{self, UNKNOWN};
use k8s_openapi::{
  api::core::v1::{ContainerState, ContainerStateWaiting, Pod, PodStatus},
  chrono::Utc,
};

#[derive(Clone)]
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

#[derive(Clone)]
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
      Some(stat) => {
        let (mut cr, mut rc) = (0, 0);
        let c_stats_len = match stat.container_statuses.as_ref() {
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

        let containers = stat
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

        (get_status(stat, pod), cr, rc, c_stats_len, containers)
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
    Some(phase) => phase.clone(),
    _ => UNKNOWN.into(),
  };
  let status = match &stat.reason {
    Some(r) => {
      if r == "NodeLost" && pod.metadata.deletion_timestamp.is_some() {
        UNKNOWN.into()
      } else {
        status
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
