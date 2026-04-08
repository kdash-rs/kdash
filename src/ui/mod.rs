use rand::RngExt;
mod help;
mod overview;
pub mod resource_tabs;
pub mod theme;
pub mod utils;

use ratatui::{
  layout::{Alignment, Constraint, Rect},
  style::Modifier,
  text::{Line, Text},
  widgets::{Block, Borders, Paragraph, Tabs, Wrap},
  Frame,
};

use self::{
  help::draw_help,
  overview::draw_overview,
  utils::{
    action_hint, default_part, help_part, horizontal_chunks_with_margin, key_hints,
    mixed_bold_line, mixed_line, split_hint_suffix, style_failure, style_header,
    style_main_background, style_primary, style_secondary, style_success, vertical_chunks,
  },
};
use crate::app::{
  contexts::ContextResource, key_binding::DEFAULT_KEYBINDING, metrics::UtilizationResource,
  models::AppResource, troubleshoot::TroubleshootResource, ActiveBlock, App, RouteId,
};

pub static HIGHLIGHT: &str = "=> ";

pub fn draw(f: &mut Frame<'_>, app: &mut App) {
  let block = Block::default().style(style_main_background(app.light_theme));
  f.render_widget(block, f.area());

  let chunks = if !app.api_error.is_empty() || !app.status_message.is_empty() {
    let chunks = vertical_chunks(
      vec![
        Constraint::Length(1), // title
        Constraint::Length(3), // header tabs
        Constraint::Length(3), // banner
        Constraint::Min(0),    // main tabs
      ],
      f.area(),
    );
    if !app.api_error.is_empty() {
      draw_app_error(f, app, chunks[2]);
    } else {
      draw_app_status(f, app, chunks[2]);
    }
    chunks
  } else {
    vertical_chunks(
      vec![
        Constraint::Length(1), // title
        Constraint::Length(3), // header tabs
        Constraint::Min(0),    // main tabs
      ],
      f.area(),
    )
  };

  draw_app_title(f, app, chunks[0]);
  // draw header tabs amd text
  draw_app_header(f, app, chunks[1]);

  let last_chunk = chunks[chunks.len() - 1];
  match app.get_current_route().id {
    RouteId::HelpMenu => {
      draw_help(f, app, last_chunk);
    }
    RouteId::Contexts => {
      ContextResource::render(ActiveBlock::Contexts, f, app, last_chunk);
    }
    RouteId::Utilization => {
      UtilizationResource::render(ActiveBlock::Utilization, f, app, last_chunk);
    }
    RouteId::Troubleshoot => {
      // Only render the active troubleshoot block to avoid unnecessary checks and rendering
      TroubleshootResource::render(app.get_current_route().active_block, f, app, last_chunk);
    }
    _ => {
      draw_overview(f, app, last_chunk);
    }
  }
}

fn draw_app_title(f: &mut Frame<'_>, app: &App, area: Rect) {
  let title = Paragraph::new(app.title)
    .style(style_header(app.light_theme).add_modifier(Modifier::BOLD))
    .block(Block::default())
    .alignment(Alignment::Left);
  f.render_widget(title, area);

  let text = format!(
    "v{} with ♥ in Rust {} ",
    env!("CARGO_PKG_VERSION"),
    nw_loading_indicator(app.is_loading())
  );

  let meta = Paragraph::new(text)
    .style(style_header(app.light_theme))
    .block(Block::default())
    .alignment(Alignment::Right);
  f.render_widget(meta, area);
}

// loading animation frames
const FRAMES: &[&str] = &["⠋⠴", "⠦⠙", "⠏⠼", "⠧⠹", "⠯⠽"];

fn nw_loading_indicator<'a>(loading: bool) -> &'a str {
  if loading {
    FRAMES[rand::rng().random_range(0..FRAMES.len())]
  } else {
    ""
  }
}

fn draw_app_header(f: &mut Frame<'_>, app: &App, area: Rect) {
  let chunks =
    horizontal_chunks_with_margin(vec![Constraint::Length(75), Constraint::Min(0)], area, 1);

  let titles: Vec<Line<'_>> = app
    .main_tabs
    .items
    .iter()
    .enumerate()
    .map(|(i, t)| {
      let (label, hint) = split_hint_suffix(&t.title);
      if i == app.main_tabs.index {
        Line::from(label.to_string())
      } else {
        let mut parts = vec![default_part(label.to_string())];
        if let Some(hint) = hint {
          parts.push(help_part(format!(" {}", hint)));
        }
        mixed_line(parts, app.light_theme)
      }
    })
    .collect();
  let tabs = Tabs::new(titles)
    .block(Block::default().borders(Borders::ALL))
    .highlight_style(style_secondary(app.light_theme))
    .select(app.main_tabs.index);

  f.render_widget(tabs, area);
  draw_header_text(f, app, chunks[1]);
}

fn draw_header_text(f: &mut Frame<'_>, app: &App, area: Rect) {
  let text = match app.get_current_route().id {
    RouteId::Contexts => vec![mixed_line(
      [help_part(format!(
        "{} scroll | {} select | {} | {} ",
        key_hints(&[DEFAULT_KEYBINDING.up.key, DEFAULT_KEYBINDING.down.key]),
        DEFAULT_KEYBINDING.submit.key,
        action_hint("filter", DEFAULT_KEYBINDING.filter.key),
        action_hint("help", DEFAULT_KEYBINDING.help.key)
      ))],
      app.light_theme,
    )],
    RouteId::Home => vec![mixed_line(
      [help_part(format!(
        "{} switch tabs | <char> select block | {} scroll | {} select | {} | {} ",
        key_hints(&[
          DEFAULT_KEYBINDING.cycle_main_views.key,
          DEFAULT_KEYBINDING.left.key,
          DEFAULT_KEYBINDING.right.key
        ]),
        key_hints(&[DEFAULT_KEYBINDING.up.key, DEFAULT_KEYBINDING.down.key]),
        DEFAULT_KEYBINDING.submit.key,
        action_hint("filter", DEFAULT_KEYBINDING.filter.key),
        action_hint("help", DEFAULT_KEYBINDING.help.key)
      ))],
      app.light_theme,
    )],
    RouteId::Utilization => vec![mixed_line(
      [help_part(format!(
        "{} scroll | {} | {} | {} ",
        key_hints(&[DEFAULT_KEYBINDING.up.key, DEFAULT_KEYBINDING.down.key]),
        action_hint("filter", DEFAULT_KEYBINDING.filter.key),
        action_hint("cycle grouping", DEFAULT_KEYBINDING.cycle_group_by.key),
        action_hint("help", DEFAULT_KEYBINDING.help.key)
      ))],
      app.light_theme,
    )],
    RouteId::Troubleshoot => vec![mixed_line(
      [help_part(format!(
        "{} scroll | {} | {} ",
        key_hints(&[DEFAULT_KEYBINDING.up.key, DEFAULT_KEYBINDING.down.key]),
        action_hint("filter", DEFAULT_KEYBINDING.filter.key),
        action_hint("help", DEFAULT_KEYBINDING.help.key)
      ))],
      app.light_theme,
    )],
    RouteId::HelpMenu => vec![],
  };
  let paragraph = Paragraph::new(text)
    .block(Block::default())
    .alignment(Alignment::Right);
  f.render_widget(paragraph, area);
}

fn draw_app_error(f: &mut Frame<'_>, app: &App, size: Rect) {
  let block = Block::default()
    .title(mixed_bold_line(
      [
        default_part(" Error "),
        help_part(format!("| close {} ", DEFAULT_KEYBINDING.esc.key)),
      ],
      app.light_theme,
    ))
    .style(style_failure(app.light_theme))
    .borders(Borders::ALL);

  let text = Text::from(app.api_error.clone());
  let text = text.patch_style(style_failure(app.light_theme));

  let paragraph = Paragraph::new(text)
    .style(style_primary(app.light_theme))
    .block(block)
    .wrap(Wrap { trim: true });
  f.render_widget(paragraph, size);
}

fn draw_app_status(f: &mut Frame<'_>, app: &App, size: Rect) {
  let block = Block::default()
    .title(mixed_bold_line(
      [
        default_part(" Info "),
        help_part(format!("| close {} ", DEFAULT_KEYBINDING.esc.key)),
      ],
      app.light_theme,
    ))
    .style(style_success(app.light_theme))
    .borders(Borders::ALL);

  let text = Text::from(app.status_message.text().to_owned());
  let text = text.patch_style(style_success(app.light_theme));

  let paragraph = Paragraph::new(text)
    .style(style_primary(app.light_theme))
    .block(block)
    .wrap(Wrap { trim: true });
  f.render_widget(paragraph, size);
}

#[cfg(test)]
mod tests {
  use std::iter;

  use k8s_openapi::api::{
    apps::v1::{DaemonSet, Deployment, ReplicaSet},
    batch::v1::Job,
    core::v1::{ConfigMap, Node, Pod, Service},
  };
  use k8s_openapi::apimachinery::pkg::apis::meta::v1::ListMeta;
  use kube::{api::ObjectList, core::TypeMeta};
  use ratatui::{backend::TestBackend, style::Modifier, Terminal};

  use super::*;
  use crate::{
    app::{
      configmaps::KubeConfigMap, contexts::KubeContext, daemonsets::KubeDaemonSet,
      deployments::KubeDeployment, jobs::KubeJob, metrics::KubeNodeMetrics, nodes::KubeNode,
      ns::KubeNs, pods::KubePod, replicasets::KubeReplicaSet, svcs::KubeSvc, Cli,
    },
    ui::utils::{MACCHIATO_BLUE, MACCHIATO_RED, MACCHIATO_TEXT, MACCHIATO_YELLOW},
  };

  const OVERVIEW_FIXTURE: &str = include_str!("../../test_data/ui-overview-test.txt");

  #[test]
  fn test_draw_overview_full_screen_fixture() {
    let backend = TestBackend::new(180, 51);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
      .draw(|f| {
        let mut app = seeded_overview_app();
        draw(f, &mut app);
      })
      .unwrap();

    let buffer = terminal.backend().buffer();
    let lines = buffer_lines(buffer);
    let expected_lines: Vec<String> = OVERVIEW_FIXTURE
      .lines()
      .map(|line| line.to_string())
      .collect();

    assert_eq!(lines, expected_lines);

    assert_eq!(buffer[(1, 13)].fg, MACCHIATO_YELLOW);
    assert!(buffer[(1, 13)].modifier.contains(Modifier::BOLD));

    assert_eq!(buffer[(1, 16)].fg, MACCHIATO_BLUE);
    assert_eq!(buffer[(1, 19)].fg, MACCHIATO_TEXT);

    assert_eq!(buffer[(1, 20)].fg, MACCHIATO_RED);
    assert!(buffer[(1, 20)].modifier.contains(Modifier::REVERSED));
  }

  #[test]
  fn test_nw_loading_indicator_is_empty_when_not_loading() {
    assert_eq!(nw_loading_indicator(false), "");
  }

  #[test]
  fn test_nw_loading_indicator_uses_known_spinner_frames_when_loading() {
    assert!(FRAMES.contains(&nw_loading_indicator(true)));
  }

  #[test]
  fn test_draw_renders_status_banner() {
    let mut app = App::default();
    app.set_status_message("Saved recent errors to /tmp/kdash-errors.log");

    let lines = render_lines(&mut app, 120, 20);
    let joined = lines.join("\n");

    assert!(joined.contains("Info"));
    assert!(joined.contains("Saved recent errors to /tmp/kdash-errors.log"));
  }

  #[test]
  fn test_draw_renders_error_banner() {
    let mut app = App::default();
    app.api_error = "Kubernetes API unavailable".into();

    let lines = render_lines(&mut app, 120, 20);
    let joined = lines.join("\n");

    assert!(joined.contains("Error"));
    assert!(joined.contains("Kubernetes API unavailable"));
  }

  #[test]
  fn test_draw_contexts_route_renders_context_header_hints() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::Contexts, ActiveBlock::Contexts);

    let lines = render_lines(&mut app, 140, 20);
    let joined = lines.join("\n");

    assert!(joined.contains("scroll"));
    assert!(joined.contains("filter </>"));
    assert!(joined.contains("help <?>"));
  }

  #[test]
  fn test_draw_utilization_route_renders_grouping_hint() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::Utilization, ActiveBlock::Utilization);

    let lines = render_lines(&mut app, 140, 20);
    let joined = lines.join("\n");

    assert!(joined.contains("cycle grouping <g>"));
    assert!(joined.contains("filter </>"));
  }

  #[test]
  fn test_draw_troubleshoot_route_renders_troubleshoot_header_hints() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::Troubleshoot, ActiveBlock::Troubleshoot);

    let lines = render_lines(&mut app, 140, 20);
    let joined = lines.join("\n");

    assert!(joined.contains("Troubleshoot"));
    assert!(joined.contains("filter </>"));
    assert!(joined.contains("help <?>"));
  }

  fn seeded_overview_app() -> App {
    let mut app = App::default();
    app.title = "KDash - A simple Kubernetes dashboard";
    app.enhanced_graphics = true;

    app.data.namespaces.set_items(vec![
      kube_ns("default", "Active"),
      kube_ns("kdash-demo", "Active"),
      kube_ns("kdash-log-test", "Active"),
      kube_ns("kdash-rbac-test", "Active"),
      kube_ns("kube-node-lease", "Active"),
      kube_ns("kube-public", "Active"),
    ]);

    app.data.active_context = Some(KubeContext {
      name: "k3d-mycluster".into(),
      cluster: "k3d-mycluster".into(),
      user: Some("admin@k3d-mycluster".into()),
      namespace: Some("default".into()),
      is_active: true,
    });

    app.data.node_metrics = vec![KubeNodeMetrics {
      name: "k3d-mycluster-agent-0".into(),
      cpu_percent: 0.0,
      mem_percent: 1.0,
      ..KubeNodeMetrics::default()
    }];

    app.data.clis = vec![
      Cli {
        name: "kubectl client".into(),
        version: "v1.35.3".into(),
        status: true,
      },
      Cli {
        name: "kubectl server".into(),
        version: "v1.33.6+k3s1".into(),
        status: true,
      },
      Cli {
        name: "docker".into(),
        version: "v29.3.1".into(),
        status: true,
      },
      Cli {
        name: "docker-compose".into(),
        version: "v5.1.1".into(),
        status: true,
      },
      Cli {
        name: "kind".into(),
        version: "v0.31.0".into(),
        status: true,
      },
      Cli {
        name: "helm".into(),
        version: "Not found".into(),
        status: false,
      },
      Cli {
        name: "istioctl".into(),
        version: "Not found".into(),
        status: false,
      },
    ];

    app.data.pods.set_items(seeded_pods());
    app
      .data
      .services
      .set_items(repeat_items(4, KubeSvc::from(Service::default())));
    app.data.nodes.set_items(vec![kube_node()]);
    app
      .data
      .config_maps
      .set_items(repeat_items(15, KubeConfigMap::from(ConfigMap::default())));
    app
      .data
      .replica_sets
      .set_items(repeat_items(6, KubeReplicaSet::from(ReplicaSet::default())));
    app
      .data
      .deployments
      .set_items(repeat_items(6, KubeDeployment::from(Deployment::default())));
    app
      .data
      .jobs
      .set_items(repeat_items(2, KubeJob::from(Job::default())));
    app
      .data
      .daemon_sets
      .set_items(repeat_items(1, KubeDaemonSet::from(DaemonSet::default())));

    app
  }

  fn repeat_items<T: Clone>(count: usize, item: T) -> Vec<T> {
    iter::repeat_n(item, count).collect()
  }

  fn kube_ns(name: &str, status: &str) -> KubeNs {
    let mut ns = KubeNs::default();
    ns.name = name.into();
    ns.status = status.into();
    ns
  }

  fn kube_pod(namespace: &str, name: &str, ready: (i32, i32), status: &str, age: &str) -> KubePod {
    kube_pod_with_restarts(namespace, name, ready, status, age, 0)
  }

  fn kube_pod_with_restarts(
    namespace: &str,
    name: &str,
    ready: (i32, i32),
    status: &str,
    age: &str,
    restarts: i32,
  ) -> KubePod {
    let mut pod = KubePod::default();
    pod.namespace = namespace.into();
    pod.name = name.into();
    pod.ready = ready;
    pod.status = status.into();
    pod.age = age.into();
    pod.restarts = restarts;
    pod
  }

  fn kube_node() -> KubeNode {
    let seed_app = tokio::sync::Mutex::new(App::default());
    let mut guard = seed_app.blocking_lock();
    let pods = ObjectList::<Pod> {
      types: TypeMeta {
        api_version: "v1".into(),
        kind: "List".into(),
      },
      metadata: ListMeta::default(),
      items: vec![],
    };
    KubeNode::from_api_with_pods(&Node::default(), &pods, &mut guard)
  }

  fn seeded_pods() -> Vec<KubePod> {
    vec![
      kube_pod("default", "bad-image", (0, 1), "ImagePullBackOff", "4d21h"),
      kube_pod(
        "default",
        "kdash-test-multi-6bccdcf865-hrr8t",
        (2, 2),
        "Running",
        "4d4h",
      ),
      kube_pod(
        "default",
        "kdash-test-multi-6bccdcf865-lsc59",
        (2, 2),
        "Running",
        "4d4h",
      ),
      kube_pod(
        "default",
        "kdash-test-multi-6bccdcf865-s2qbp",
        (2, 2),
        "Running",
        "4d4h",
      ),
      kube_pod(
        "default",
        "kdash-test-nginx-776f75c995-27pf7",
        (1, 1),
        "Running",
        "4d4h",
      ),
      kube_pod(
        "default",
        "kdash-test-nginx-776f75c995-5fqvt",
        (1, 1),
        "Running",
        "4d4h",
      ),
      kube_pod(
        "default",
        "kdash-test-nginx-776f75c995-sh92q",
        (1, 1),
        "Running",
        "4d4h",
      ),
      kube_pod("default", "pending-pod", (0, 1), "Pending", "4d21h"),
      kube_pod(
        "kdash-demo",
        "bad-image",
        (0, 1),
        "ImagePullBackOff",
        "4d21h",
      ),
      kube_pod("kdash-demo", "pending-pod", (0, 1), "Pending", "4d21h"),
      kube_pod(
        "kdash-log-test",
        "kdash-log-fast",
        (1, 1),
        "Running",
        "23h34m",
      ),
      kube_pod(
        "kdash-log-test",
        "kdash-log-stream",
        (1, 1),
        "Running",
        "23h38m",
      ),
      kube_pod(
        "kdash-rbac-test",
        "kdash-rbac-demo",
        (1, 1),
        "Running",
        "2d1h",
      ),
      kube_pod(
        "kube-system",
        "coredns-6d668d687-wqqjq",
        (1, 1),
        "Running",
        "5d3h",
      ),
      kube_pod(
        "kube-system",
        "helm-install-traefik-crd-r5h8c",
        (0, 1),
        "Completed",
        "5d3h",
      ),
      kube_pod_with_restarts(
        "kube-system",
        "helm-install-traefik-vhdr6",
        (0, 1),
        "Completed",
        "5d3h",
        1,
      ),
      kube_pod(
        "kube-system",
        "local-path-provisioner-869c44bfbd-pfxd6",
        (1, 1),
        "Running",
        "5d3h",
      ),
      kube_pod(
        "kube-system",
        "metrics-server-7bfffcd44-ftxdj",
        (1, 1),
        "Running",
        "5d3h",
      ),
      kube_pod(
        "kube-system",
        "svclb-traefik-207900ce-62q7c",
        (2, 2),
        "Running",
        "5d3h",
      ),
      kube_pod(
        "kube-system",
        "traefik-865bd56545-4htnx",
        (1, 1),
        "Running",
        "5d3h",
      ),
    ]
  }

  fn buffer_lines(buffer: &ratatui::buffer::Buffer) -> Vec<String> {
    (0..buffer.area.height)
      .map(|row| {
        (0..buffer.area.width)
          .map(|col| buffer[(col, row)].symbol())
          .collect::<String>()
      })
      .collect()
  }

  fn render_lines(app: &mut App, width: u16, height: u16) -> Vec<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|f| draw(f, app)).unwrap();

    buffer_lines(terminal.backend().buffer())
  }
}
