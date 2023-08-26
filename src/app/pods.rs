use async_trait::async_trait;
use k8s_openapi::{
  api::core::v1::{
    Container, ContainerPort, ContainerState, ContainerStateWaiting, ContainerStatus, Pod, PodSpec,
    PodStatus,
  },
  chrono::Utc,
};
use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  style::Style,
  widgets::{Cell, Row},
  Frame,
};

use super::{
  models::{AppResource, KubeResource},
  utils::{self, UNKNOWN},
  ActiveBlock, App,
};
use crate::{
  network::Network,
  ui::utils::{
    draw_describe_block, draw_resource_block, get_describe_active, get_resource_title,
    layout_block_top_border, loading, style_failure, style_primary, style_secondary, style_success,
    title_with_dual_style, ResourceTableProps, COPY_HINT, DESCRIBE_AND_YAML_HINT,
  },
};

#[derive(Clone, Default, Debug, PartialEq)]
pub struct KubePod {
  pub namespace: String,
  pub name: String,
  pub ready: (i32, i32),
  pub status: String,
  pub restarts: i32,
  pub cpu: String,
  pub mem: String,
  pub age: String,
  pub containers: Vec<KubeContainer>,
  k8s_obj: Pod,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
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
  pub init: bool,
}

impl From<Pod> for KubePod {
  fn from(pod: Pod) -> Self {
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

        let mut containers: Vec<KubeContainer> = pod
          .spec
          .as_ref()
          .unwrap_or(&PodSpec::default())
          .containers
          .iter()
          .map(|c| {
            KubeContainer::from_api(
              c,
              pod_name.to_owned(),
              age.to_owned(),
              &status.container_statuses,
              false,
            )
          })
          .collect();

        let mut init_containers: Vec<KubeContainer> = pod
          .spec
          .as_ref()
          .unwrap_or(&PodSpec::default())
          .init_containers
          .as_ref()
          .unwrap_or(&vec![])
          .iter()
          .map(|c| {
            KubeContainer::from_api(
              c,
              pod_name.to_owned(),
              age.to_owned(),
              &status.init_container_statuses,
              true,
            )
          })
          .collect();

        // merge containers and init-containers into single array
        containers.append(&mut init_containers);

        (get_status(status, &pod), cr, rc, c_stats_len, containers)
      }
      _ => (UNKNOWN.into(), 0, 0, 0, vec![]),
    };

    KubePod {
      name: pod_name,
      namespace: pod.metadata.namespace.clone().unwrap_or_default(),
      ready: (cr, c_stats_len as i32),
      restarts,
      // TODO implement pod metrics
      cpu: String::default(),
      mem: String::default(),
      status,
      age,
      containers,
      k8s_obj: utils::sanitize_obj(pod),
    }
  }
}

impl KubeResource<Pod> for KubePod {
  fn get_k8s_obj(&self) -> &Pod {
    &self.k8s_obj
  }
}

static PODS_TITLE: &str = "Pods";
pub struct PodResource {}

#[async_trait]
impl AppResource for PodResource {
  fn render<B: Backend>(block: ActiveBlock, f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
    match block {
      ActiveBlock::Containers => draw_containers_block(f, app, area),
      ActiveBlock::Describe | ActiveBlock::Yaml => draw_describe_block(
        f,
        app,
        area,
        title_with_dual_style(
          get_resource_title(
            app,
            PODS_TITLE,
            get_describe_active(block),
            app.data.pods.items.len(),
          ),
          format!("{} | {} <esc> ", COPY_HINT, PODS_TITLE),
          app.light_theme,
        ),
      ),
      ActiveBlock::Logs => draw_logs_block(f, app, area),
      ActiveBlock::Namespaces => Self::render(app.get_prev_route().active_block, f, app, area),
      _ => draw_block(f, app, area),
    }
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubePod> = nw.get_namespaced_resources(Pod::into).await;

    let mut app = nw.app.lock().await;
    if app.data.selected.pod.is_some() {
      let containers = &items.iter().find_map(|pod| {
        if pod.name == app.data.selected.pod.clone().unwrap() {
          Some(&pod.containers)
        } else {
          None
        }
      });
      if containers.is_some() {
        app.data.containers.set_items(containers.unwrap().clone());
      }
    }
    app.data.pods.set_items(items);
  }
}

fn draw_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, PODS_TITLE, "", app.data.pods.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: format!("| Containers <enter> {}", DESCRIBE_AND_YAML_HINT),
      resource: &mut app.data.pods,
      table_headers: vec!["Namespace", "Name", "Ready", "Status", "Restarts", "Age"],
      column_widths: vec![
        Constraint::Percentage(25),
        Constraint::Percentage(35),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      let style = get_resource_row_style(c.status.as_str(), c.ready, app.light_theme);
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(format!("{}/{}", c.ready.0, c.ready.1)),
        Cell::from(c.status.to_owned()),
        Cell::from(c.restarts.to_string()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style)
    },
    app.light_theme,
    app.is_loading,
  );
}

fn draw_containers_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let title = get_container_title(app, app.data.containers.items.len(), "");

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: format!("| Logs <enter> | {} <esc> ", PODS_TITLE),
      resource: &mut app.data.containers,
      table_headers: vec![
        "Name",
        "Image",
        "Init",
        "Ready",
        "State",
        "Restarts",
        "Probes(L/R)",
        "Ports",
        "Age",
      ],
      column_widths: vec![
        Constraint::Percentage(20),
        Constraint::Percentage(25),
        Constraint::Percentage(5),
        Constraint::Percentage(5),
        Constraint::Percentage(10),
        Constraint::Percentage(5),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
      ],
    },
    |c| {
      let style = get_resource_row_style(c.status.as_str(), (0, 0), app.light_theme);
      Row::new(vec![
        Cell::from(c.name.to_owned()),
        Cell::from(c.image.to_owned()),
        Cell::from(c.init.to_string()),
        Cell::from(c.ready.to_owned()),
        Cell::from(c.status.to_owned()),
        Cell::from(c.restarts.to_string()),
        Cell::from(format!("{}/{}", c.liveliness_probe, c.readiness_probe,)),
        Cell::from(c.ports.to_owned()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style)
    },
    app.light_theme,
    app.is_loading,
  );
}

fn get_container_title<S: AsRef<str>>(app: &App, container_len: usize, suffix: S) -> String {
  let title = get_resource_title(
    app,
    PODS_TITLE,
    format!("-> Containers [{}] {}", container_len, suffix.as_ref()).as_str(),
    app.data.pods.items.len(),
  );
  title
}

fn draw_logs_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let selected_container = app.data.selected.container.clone();
  let container_name = selected_container.unwrap_or_default();

  let title = title_with_dual_style(
    get_container_title(
      app,
      app.data.containers.items.len(),
      format!("-> Logs ({}) ", container_name),
    ),
    "| copy <c> | Containers <esc> ".into(),
    app.light_theme,
  );

  let block = layout_block_top_border(title);

  if container_name == app.data.logs.id {
    app.data.logs.render_list(
      f,
      area,
      block,
      style_primary(app.light_theme),
      app.log_auto_scroll,
    );
  } else {
    loading(f, block, area, app.is_loading, app.light_theme);
  }
}

fn get_resource_row_style(status: &str, ready: (i32, i32), light: bool) -> Style {
  if status == "Running" && ready.0 == ready.1 {
    style_primary(light)
  } else if status == "Completed" {
    style_success(light)
  } else if [
    "ContainerCreating",
    "PodInitializing",
    "Pending",
    "Initialized",
  ]
  .contains(&status)
  {
    style_secondary(light)
  } else {
    style_failure(light)
  }
}

impl KubeContainer {
  pub fn from_api(
    container: &Container,
    pod_name: String,
    age: String,
    c_stats_ref: &Option<Vec<ContainerStatus>>,
    init: bool,
  ) -> Self {
    let (mut ready, mut status, mut restarts) = ("false".to_string(), "<none>".to_string(), 0);
    if let Some(c_stats) = c_stats_ref {
      if let Some(c_stat) = c_stats.iter().find(|cs| cs.name == container.name) {
        ready = c_stat.ready.to_string();
        status = get_container_state(c_stat.state.clone());
        restarts = c_stat.restart_count;
      }
    }

    KubeContainer {
      name: container.name.clone(),
      pod_name,
      image: container.image.clone().unwrap_or_default(),
      ready,
      status,
      restarts,
      liveliness_probe: container.liveness_probe.is_some(),
      readiness_probe: container.readiness_probe.is_some(),
      ports: get_container_ports(&container.ports).unwrap_or_default(),
      age,
      init,
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
        let c_status = match &cs.state {
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
        if !c_status.is_empty() {
          return c_status;
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

fn get_container_ports(ports_ref: &Option<Vec<ContainerPort>>) -> Option<String> {
  ports_ref.as_ref().map(|ports| {
    ports
      .iter()
      .map(|c_port| {
        let mut port = String::new();
        if let Some(name) = c_port.name.clone() {
          port = format!("{}:", name);
        }
        port = format!("{}{}", port, c_port.container_port);
        if let Some(protocol) = c_port.protocol.clone() {
          if protocol != "TCP" {
            port = format!("{}/{}", port, c_port.protocol.clone().unwrap());
          }
        }
        port
      })
      .collect::<Vec<_>>()
      .join(", ")
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_utils::*;

  #[test]
  fn test_get_container_title() {
    let app = App::default();
    assert_eq!(
      get_container_title(&app, 3, "hello"),
      " Pods (ns: all) [0] -> Containers [3] hello"
    );
  }

  #[test]
  fn test_pod_from_api() {
    let (pods, pods_list): (Vec<KubePod>, Vec<_>) = convert_resource_from_file("pods");

    assert_eq!(pods.len(), 13);
    assert_eq!(
      pods[0],
      KubePod {
        namespace: "default".into(),
        name: "adservice-f787c8dcd-tb6x2".into(),
        ready: (0, 0),
        status: "Pending".into(),
        restarts: 0,
        cpu: "".into(),
        mem: "".into(),
        age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
        containers: vec![KubeContainer {
          name: "server".into(),
          image: "gcr.io/google-samples/microservices-demo/adservice:v0.2.2".into(),
          ready: "false".into(),
          status: "<none>".into(),
          restarts: 0,
          liveliness_probe: true,
          readiness_probe: true,
          ports: "9555".into(),
          age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
          pod_name: "adservice-f787c8dcd-tb6x2".into(),
          init: false,
        }],
        k8s_obj: pods_list[0].clone()
      }
    );
    assert_eq!(
      pods[1],
      KubePod {
        namespace: "default".into(),
        name: "cartservice-67b89ffc69-s5qp8".into(),
        ready: (0, 1),
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
          liveliness_probe: true,
          readiness_probe: true,
          ports: "7070".into(),
          age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
          pod_name: "cartservice-67b89ffc69-s5qp8".into(),
          init: false,
        }],
        k8s_obj: pods_list[1].clone()
      }
    );
    assert_eq!(
      pods[3],
      KubePod {
        namespace: "default".into(),
        name: "emailservice-5f8fc7dbb4-5lqdb".into(),
        ready: (1, 1),
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
          liveliness_probe: true,
          readiness_probe: true,
          ports: "8080".into(),
          age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
          pod_name: "emailservice-5f8fc7dbb4-5lqdb".into(),
          init: false,
        }],
        k8s_obj: pods_list[3].clone()
      }
    );
    assert_eq!(
      pods[4],
      KubePod {
        namespace: "default".into(),
        name: "frontend-5c4745dfdb-6k8wf".into(),
        ready: (0, 0),
        status: "OutOfcpu".into(),
        restarts: 0,
        cpu: "".into(),
        mem: "".into(),
        age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
        containers: vec![KubeContainer {
          name: "server".into(),
          image: "gcr.io/google-samples/microservices-demo/frontend:v0.2.2".into(),
          ready: "false".into(),
          status: "<none>".into(),
          restarts: 0,
          liveliness_probe: true,
          readiness_probe: true,
          ports: "8080".into(),
          age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
          pod_name: "frontend-5c4745dfdb-6k8wf".into(),
          init: false,
        }],
        k8s_obj: pods_list[4].clone()
      }
    );
    assert_eq!(
      pods[5],
      KubePod {
        namespace: "default".into(),
        name: "frontend-5c4745dfdb-qz7fg".into(),
        ready: (0, 0),
        status: "Preempting".into(),
        restarts: 0,
        cpu: "".into(),
        mem: "".into(),
        age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
        containers: vec![KubeContainer {
          name: "server".into(),
          image: "gcr.io/google-samples/microservices-demo/frontend:v0.2.2".into(),
          ready: "false".into(),
          status: "<none>".into(),
          restarts: 0,
          liveliness_probe: false,
          readiness_probe: true,
          ports: "8080/HTTP".into(),
          age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
          pod_name: "frontend-5c4745dfdb-qz7fg".into(),
          init: false,
        }],
        k8s_obj: pods_list[5].clone()
      }
    );
    assert_eq!(
      pods[6],
      KubePod {
        namespace: "default".into(),
        name: "frontend-5c4745dfdb-6k8wf".into(),
        ready: (0, 0),
        status: "Failed".into(),
        restarts: 0,
        cpu: "".into(),
        mem: "".into(),
        age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
        containers: vec![KubeContainer {
          name: "server".into(),
          image: "gcr.io/google-samples/microservices-demo/frontend:v0.2.2".into(),
          ready: "false".into(),
          status: "<none>".into(),
          restarts: 0,
          liveliness_probe: true,
          readiness_probe: true,
          ports: "8080, 8081/UDP, Foo:8082/UDP, 8083".into(),
          age: utils::to_age(Some(&get_time("2021-04-27T10:13:58Z")), Utc::now()),
          pod_name: "frontend-5c4745dfdb-6k8wf".into(),
          init: false,
        }],
        k8s_obj: pods_list[6].clone()
      }
    );
    assert_eq!(
      pods[11],
      KubePod {
        namespace: "default".into(),
        name: "pod-init-container".into(),
        ready: (0, 1),
        status: "Init:1/2".into(),
        restarts: 0,
        cpu: "".into(),
        mem: "".into(),
        age: utils::to_age(Some(&get_time("2021-06-18T08:57:56Z")), Utc::now()),
        containers: vec![
          KubeContainer {
            name: "main-busybox".into(),
            image: "busybox".into(),
            ready: "false".into(),
            status: "PodInitializing".into(),
            restarts: 0,
            liveliness_probe: false,
            readiness_probe: false,
            ports: "".into(),
            age: utils::to_age(Some(&get_time("2021-06-18T08:57:56Z")), Utc::now()),
            pod_name: "pod-init-container".into(),
            init: false,
          },
          KubeContainer {
            name: "init-busybox1".into(),
            image: "busybox".into(),
            ready: "true".into(),
            status: "Completed".into(),
            restarts: 0,
            liveliness_probe: false,
            readiness_probe: false,
            ports: "".into(),
            age: utils::to_age(Some(&get_time("2021-06-18T08:57:56Z")), Utc::now()),
            pod_name: "pod-init-container".into(),
            init: true,
          },
          KubeContainer {
            name: "init-busybox2".into(),
            image: "busybox".into(),
            ready: "false".into(),
            status: "Running".into(),
            restarts: 0,
            liveliness_probe: false,
            readiness_probe: false,
            ports: "".into(),
            age: utils::to_age(Some(&get_time("2021-06-18T08:57:56Z")), Utc::now()),
            pod_name: "pod-init-container".into(),
            init: true,
          }
        ],
        k8s_obj: pods_list[11].clone()
      }
    );
    assert_eq!(
      pods[12],
      KubePod {
        namespace: "default".into(),
        name: "pod-init-container-2".into(),
        ready: (0, 1),
        status: "Completed".into(),
        restarts: 0,
        cpu: "".into(),
        mem: "".into(),
        age: utils::to_age(Some(&get_time("2021-06-18T09:26:11Z")), Utc::now()),
        containers: vec![
          KubeContainer {
            name: "main-busybox".into(),
            image: "busybox".into(),
            ready: "false".into(),
            status: "Completed".into(),
            restarts: 0,
            liveliness_probe: false,
            readiness_probe: false,
            ports: "".into(),
            age: utils::to_age(Some(&get_time("2021-06-18T09:26:11Z")), Utc::now()),
            pod_name: "pod-init-container-2".into(),
            init: false,
          },
          KubeContainer {
            name: "init-busybox1".into(),
            image: "busybox".into(),
            ready: "true".into(),
            status: "Completed".into(),
            restarts: 0,
            liveliness_probe: false,
            readiness_probe: false,
            ports: "".into(),
            age: utils::to_age(Some(&get_time("2021-06-18T09:26:11Z")), Utc::now()),
            pod_name: "pod-init-container-2".into(),
            init: true,
          },
          KubeContainer {
            name: "init-busybox2".into(),
            image: "busybox".into(),
            ready: "true".into(),
            status: "Completed".into(),
            restarts: 0,
            liveliness_probe: false,
            readiness_probe: false,
            ports: "".into(),
            age: utils::to_age(Some(&get_time("2021-06-18T09:26:11Z")), Utc::now()),
            pod_name: "pod-init-container-2".into(),
            init: true,
          }
        ],
        k8s_obj: pods_list[12].clone()
      }
    );
    // TODO add tests for NodeLost case
  }
}
