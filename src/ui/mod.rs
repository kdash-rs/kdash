use rand::RngExt;
mod help;
mod overview;
pub mod resource_tabs;
pub mod theme;
pub mod utils;

use ratatui::{
  layout::{Alignment, Constraint, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Borders, Clear, ListItem, Paragraph, Tabs},
  Frame,
};

use self::{
  help::draw_help,
  overview::draw_overview,
  utils::{
    action_hint, centered_rect, default_part, draw_popup_menu, help_part, hint_key_glyph,
    key_hints, mixed_bold_line, mixed_line, split_hint_suffix, style_failure,
    style_main_background, style_secondary, style_text, title_with_dual_style, vertical_chunks,
  },
};
use crate::app::{
  contexts::ContextResource, key_binding::DEFAULT_KEYBINDING, metrics::UtilizationResource,
  models::AppResource, troubleshoot::TroubleshootResource, ActiveBlock, App, RouteId,
};
use crate::event::Key;

pub static HIGHLIGHT: &str = "=> ";

pub fn draw(f: &mut Frame<'_>, app: &mut App) {
  let block = Block::default().style(style_main_background(app.palette));
  f.render_widget(block, f.area());

  // Errors and status both surface as floating toasts (drawn last), so the
  // layout no longer reshuffles for them.
  let chunks = vertical_chunks(
    vec![
      Constraint::Length(1), // title
      Constraint::Length(3), // header tabs
      Constraint::Min(0),    // main tabs
    ],
    f.area(),
  );

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
      let active_block = app.get_current_route().active_block;
      if active_block == ActiveBlock::Troubleshoot {
        TroubleshootResource::render(active_block, f, app, last_chunk);
      } else {
        let outer_block = Block::default()
          .borders(Borders::ALL)
          .style(style_secondary(app.palette));
        let inner = outer_block.inner(last_chunk);
        f.render_widget(outer_block, last_chunk);
        TroubleshootResource::render(active_block, f, app, inner);
      }
    }
    _ => {
      draw_overview(f, app, last_chunk);
    }
  }

  // Transient overlays are drawn last so they sit above the current view.
  if app.action_menu.is_some() {
    draw_action_menu(f, app);
  }
  if app.input_modal.is_some() {
    draw_input_modal(f, app);
  }
  if app.modal.is_some() {
    draw_modal(f, app);
  }

  // Toasts float over everything, bottom-centred: the persistent error toast
  // (dismissed with Esc) on the bottom row, the transient status toast above it.
  draw_toasts(f, app);
}

fn draw_modal(f: &mut Frame<'_>, app: &App) {
  let palette = app.palette;
  let Some(modal) = app.modal.as_ref() else {
    return;
  };

  let width: u16 = 64;
  // Pre-wrap the prompt to the inner width so the box height (and therefore the
  // confirm/cancel hint line) always fits regardless of prompt length.
  let inner_width = width.saturating_sub(2).max(1) as usize;
  let mut lines: Vec<Line<'_>> = textwrap::wrap(&modal.prompt, inner_width)
    .into_iter()
    .map(|line| Line::from(line.into_owned()))
    .collect();
  lines.push(Line::from(""));
  lines.push(mixed_line(
    [help_part(format!(
      "{}/{}:confirm · {}/{}:cancel ",
      Key::Char('y').symbol(),
      DEFAULT_KEYBINDING.submit.key.symbol(),
      Key::Char('n').symbol(),
      DEFAULT_KEYBINDING.esc.key.symbol(),
    ))],
    palette,
  ));

  let height = (lines.len() as u16).saturating_add(2);
  let area = centered_rect(width, height, f.area());

  let block = Block::default()
    .title(mixed_bold_line(
      [default_part(format!(" {} ", modal.title))],
      palette,
    ))
    .borders(Borders::ALL)
    .style(style_failure(palette));

  let paragraph = Paragraph::new(lines)
    .block(block)
    .style(style_text(palette));

  f.render_widget(Clear, area);
  f.render_widget(paragraph, area);
}

fn draw_input_modal(f: &mut Frame<'_>, app: &App) {
  let palette = app.palette;
  let Some(input) = app.input_modal.as_ref() else {
    return;
  };

  let width: u16 = 60;
  let inner_width = width.saturating_sub(2).max(1) as usize;

  // Prompt (wrapped), the live buffer with a cursor block, an optional inline
  // error, then the submit/cancel hint — so the box always sizes to its content.
  let mut lines: Vec<Line<'_>> = textwrap::wrap(&input.prompt, inner_width)
    .into_iter()
    .map(|line| Line::from(line.into_owned()))
    .collect();
  lines.push(mixed_line(
    [default_part(format!("> {}_", input.buffer))],
    palette,
  ));
  if let Some(err) = &input.error {
    lines.push(Line::styled(err.clone(), style_failure(palette)));
  }
  lines.push(Line::from(""));
  lines.push(mixed_line(
    [help_part(format!(
      "{}:submit · {}:cancel ",
      DEFAULT_KEYBINDING.submit.key.symbol(),
      DEFAULT_KEYBINDING.esc.key.symbol(),
    ))],
    palette,
  ));

  let height = (lines.len() as u16).saturating_add(2);
  let area = centered_rect(width, height, f.area());

  let block = Block::default()
    .title(mixed_bold_line(
      [default_part(format!(" {} ", input.title))],
      palette,
    ))
    .borders(Borders::ALL)
    .style(style_secondary(palette));

  let paragraph = Paragraph::new(lines)
    .block(block)
    .style(style_text(palette));

  f.render_widget(Clear, area);
  f.render_widget(paragraph, area);
}

fn draw_action_menu(f: &mut Frame<'_>, app: &mut App) {
  let palette = app.palette;
  let block = app.get_current_route().active_block;
  let Some(menu) = app.action_menu.as_mut() else {
    return;
  };

  let items: Vec<ListItem<'_>> = menu
    .items
    .iter()
    .map(|action| {
      let key_hint = action
        .hotkey(block)
        .map(|key| key.symbol())
        .unwrap_or_default();
      ListItem::new(mixed_line(
        [
          default_part(format!("{}  ", action.label())),
          help_part(key_hint),
        ],
        palette,
      ))
    })
    .collect();

  let area = centered_rect(40, (items.len() as u16).saturating_add(2), f.area());
  let title = title_with_dual_style(
    " Actions ".to_string(),
    mixed_bold_line(
      [help_part(format!(
        "· {}:close ",
        DEFAULT_KEYBINDING.esc.key.symbol()
      ))],
      palette,
    ),
    palette,
  );
  draw_popup_menu(f, area, title, items, &mut menu.state, palette);
}

fn draw_app_title(f: &mut Frame<'_>, app: &App, area: Rect) {
  let p = app.palette;
  // Mauve (accent) title bar; text sits in the base colour for contrast.
  f.render_widget(Block::default().style(Style::default().bg(p.accent)), area);
  let fg = Style::default().fg(p.on_accent);
  let sep = || Span::styled(" · ", fg);

  // Left: identity — brand · version · connection · theme.
  let mut left = vec![
    Span::styled(" KDash", fg.add_modifier(Modifier::BOLD)),
    Span::styled(format!(" v{}", env!("CARGO_PKG_VERSION")), fg),
    sep(),
  ];
  match &app.data.active_context {
    Some(ctx) => {
      // Green connected dot, unless the theme has no distinct success colour
      // (Mono) — then fall back to the contrasting on-accent colour.
      let conn = if p.success == p.accent {
        fg
      } else {
        Style::default().fg(p.success)
      };
      left.push(Span::styled("● ", conn));
      left.push(Span::styled(ctx.name.clone(), fg));
    }
    None => left.push(Span::styled("○ disconnected", fg)),
  }
  left.push(sep());
  left.push(Span::styled(format!("◐ {}", p.name), fg));
  let spinner = nw_loading_indicator(app.is_loading());
  if !spinner.is_empty() {
    left.push(Span::styled(format!("  {}", spinner), fg));
  }
  f.render_widget(
    Paragraph::new(Line::from(left)).alignment(Alignment::Left),
    area,
  );

  // Right: every hint — the active route's contextual hints plus the
  // always-on global strip.
  f.render_widget(
    Paragraph::new(title_hint_line(app)).alignment(Alignment::Right),
    area,
  );
}

/// All keybinding hints for the title row: route-contextual hints first,
/// then the always-on global strip (help, tab cycling, theme, quit).
fn title_hint_line(app: &App) -> Line<'static> {
  let kb = &DEFAULT_KEYBINDING;
  let scroll = format!("{}:scroll", key_hints(&[kb.up.key, kb.down.key]));
  // `filter` / `group` are intentionally omitted — they're already surfaced in
  // each view's own panel title, so repeating them here would be redundant.
  let route = match app.get_current_route().id {
    RouteId::Contexts => format!("{} · {}", scroll, action_hint("select", kb.submit.key)),
    RouteId::Home => format!(
      "char:block · {} · {}",
      scroll,
      action_hint("select", kb.submit.key),
    ),
    RouteId::Utilization | RouteId::Troubleshoot => scroll.clone(),
    RouteId::HelpMenu => String::new(),
  };
  let tabs = format!(
    "{}/{}/{}/{}:tabs",
    kb.cycle_main_views.key.symbol(),
    kb.cycle_main_views_prev.key.symbol(),
    kb.right.key.symbol(),
    kb.left.key.symbol(),
  );
  let global = format!(
    "{} · {} · {} · {}",
    action_hint("help", kb.help.key),
    tabs,
    action_hint("theme", kb.toggle_theme.key),
    action_hint("quit", kb.quit.alt.unwrap_or(kb.quit.key)),
  );
  let text = if route.is_empty() {
    format!("{} ", global)
  } else {
    format!("{} · {} ", route, global)
  };
  Line::from(Span::styled(
    text,
    Style::default()
      .fg(app.palette.on_accent)
      .add_modifier(Modifier::BOLD),
  ))
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
  let titles: Vec<Line<'_>> = app
    .main_tabs
    .items
    .iter()
    .enumerate()
    .map(|(i, t)| {
      let (label, hint) = split_hint_suffix(&t.title);
      if i == app.main_tabs.index {
        Line::from(label.to_string())
      } else if let Some(hint) = hint {
        mixed_line(
          [help_part(format!("{}:{}", hint_key_glyph(hint), label))],
          app.palette,
        )
      } else {
        mixed_line([help_part(label.to_string())], app.palette)
      }
    })
    .collect();
  let tabs = Tabs::new(titles)
    .block(Block::default().borders(Borders::ALL))
    .highlight_style(style_secondary(app.palette))
    .select(app.main_tabs.index);

  f.render_widget(tabs, area);
}

/// Render one bottom-centred toast bar. `rows_from_bottom` lets stacked toasts
/// sit on consecutive rows (`2` is the bottom row). Truncates rather than wraps
/// so a toast never grows past a single line.
fn draw_toast_bar(f: &mut Frame<'_>, body: &str, bg: Color, fg: Color, rows_from_bottom: u16) {
  let area = f.area();
  let max_inner = area.width.saturating_sub(4) as usize;
  if body.is_empty() || max_inner < 8 {
    return;
  }
  let body: String = if body.chars().count() > max_inner {
    let mut truncated: String = body.chars().take(max_inner.saturating_sub(1)).collect();
    truncated.push('…');
    truncated
  } else {
    body.to_string()
  };
  let text = format!(" {} ", body);
  let w = text.chars().count() as u16;
  let rect = Rect::new(
    area.x + area.width.saturating_sub(w) / 2,
    area.y + area.height.saturating_sub(rows_from_bottom),
    w,
    1,
  );
  f.render_widget(Clear, rect);
  f.render_widget(
    Paragraph::new(text).style(Style::default().bg(bg).fg(fg).add_modifier(Modifier::BOLD)),
    rect,
  );
}

/// LlamaStash-style toasts, bottom-centred. The error toast is persistent
/// (dismissed with Esc) and sits on the bottom row; the transient status toast
/// (auto-expired by `StatusMessage`'s TTL) stacks just above it.
fn draw_toasts(f: &mut Frame<'_>, app: &App) {
  let p = app.palette;
  let mut row = 2;
  if !app.api_error.is_empty() {
    let body = format!(
      "{} · {}:dismiss",
      app.api_error,
      DEFAULT_KEYBINDING.esc.key.symbol()
    );
    draw_toast_bar(f, &body, p.error, p.on_accent, row);
    row += 1;
  }
  if !app.status_message.is_empty() {
    draw_toast_bar(f, app.status_message.text(), p.accent, p.on_accent, row);
  }
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
    ui::theme::{palette_for, ThemeName},
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

    let p = palette_for(ThemeName::Macchiato);
    assert_eq!(buffer[(1, 13)].fg, p.secondary);
    assert!(buffer[(1, 13)].modifier.contains(Modifier::BOLD));

    assert_eq!(buffer[(1, 17)].fg, p.secondary);
    assert!(buffer[(1, 17)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(22, 17)].fg, p.muted);
    assert!(buffer[(22, 17)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(1, 18)].fg, p.label);

    assert_eq!(buffer[(1, 19)].fg, p.error);
    assert!(buffer[(1, 19)].modifier.contains(Modifier::REVERSED));
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
  fn test_draw_renders_status_toast() {
    let mut app = App::default();
    app.set_status_message("Saved recent errors to /tmp/kdash-errors.log");

    let lines = render_lines(&mut app, 120, 20);
    let joined = lines.join("\n");

    assert!(joined.contains("Saved recent errors to /tmp/kdash-errors.log"));
  }

  #[test]
  fn test_draw_renders_error_toast_with_dismiss_hint() {
    let mut app = App::default();
    app.api_error = "Kubernetes API unavailable".into();

    let lines = render_lines(&mut app, 120, 20);
    let joined = lines.join("\n");

    assert!(joined.contains("Kubernetes API unavailable"));
    // Persistent error toast advertises its Esc dismissal.
    assert!(joined.contains("Esc:dismiss"));
  }

  #[test]
  fn test_draw_modal_shows_confirm_hint_even_with_long_prompt() {
    let mut app = App::default();
    app.route_home();
    app.open_modal(crate::app::actions::Modal::confirm(
      "Confirm delete",
      "Delete configmap 'app-config' in namespace 'kdash-test'? This cannot be undone.",
      crate::network::IoEvent::GetPods,
    ));

    let lines = render_lines(&mut app, 120, 30);
    let joined = lines.join("\n");

    assert!(joined.contains("Confirm delete"));
    // The prompt is shown (a token that won't straddle a wrap boundary).
    assert!(joined.contains("undone"));
    // The wrapping prompt must not push the confirm/cancel hint out of the box.
    assert!(joined.contains("confirm"));
    assert!(joined.contains("cancel"));
  }

  #[test]
  fn test_draw_input_modal_shows_prompt_buffer_and_hints() {
    use crate::app::actions::{InputAction, InputModal};

    let mut app = App::default();
    app.route_home();
    app.open_input_modal(InputModal {
      title: "Scale".into(),
      prompt: "New replica count for deployment 'web':".into(),
      buffer: "3".into(),
      error: Some("Enter a non-negative whole number".into()),
      action: InputAction::Scale {
        block: ActiveBlock::Deployments,
        name: "web".into(),
        namespace: Some("default".into()),
        kind: "deployment".into(),
      },
    });

    let lines = render_lines(&mut app, 120, 30);
    let joined = lines.join("\n");

    assert!(joined.contains("Scale"));
    assert!(joined.contains("replica count"));
    // The live buffer is shown with a cursor block.
    assert!(joined.contains("> 3_"));
    // The inline error and submit/cancel hints are visible.
    assert!(joined.contains("non-negative"));
    assert!(joined.contains("submit"));
    assert!(joined.contains("cancel"));
  }

  #[test]
  fn test_draw_action_menu_lists_actions_for_block() {
    let mut app = App::default();
    app.route_home();
    app.open_action_menu(ActiveBlock::Pods);

    let lines = render_lines(&mut app, 120, 30);
    let joined = lines.join("\n");

    assert!(joined.contains("Actions"));
    assert!(joined.contains("Describe"));
    assert!(joined.contains("Delete"));
  }

  #[test]
  fn test_draw_contexts_route_renders_context_header_hints() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::Contexts, ActiveBlock::Contexts);

    let lines = render_lines(&mut app, 140, 20);
    let joined = lines.join("\n");

    assert!(joined.contains("scroll"));
    assert!(joined.contains("/:filter"));
    assert!(joined.contains("?:help"));
  }

  #[test]
  fn test_draw_utilization_route_renders_grouping_hint() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::Utilization, ActiveBlock::Utilization);

    let lines = render_lines(&mut app, 140, 20);
    let joined = lines.join("\n");

    assert!(joined.contains("g:group"));
    assert!(joined.contains("/:filter"));
  }

  #[test]
  fn test_draw_troubleshoot_route_renders_troubleshoot_header_hints() {
    let mut app = App::default();
    app.push_navigation_stack(RouteId::Troubleshoot, ActiveBlock::Troubleshoot);

    let lines = render_lines(&mut app, 140, 20);
    let joined = lines.join("\n");

    assert!(joined.contains("Troubleshoot"));
    assert!(joined.contains("/:filter"));
    assert!(joined.contains("?:help"));
  }

  #[test]
  fn test_draw_troubleshoot_container_subview_keeps_outer_border_and_shell_hint() {
    let mut app = App::default();
    app.route_troubleshoot();
    app.push_navigation_stack(RouteId::Troubleshoot, ActiveBlock::Containers);

    let mut pod = KubePod::default();
    pod.name = "pod-1".into();
    pod.namespace = "team-a".into();
    app.data.pods.set_items(vec![pod]);

    let mut container = crate::app::pods::KubeContainer::default();
    container.name = "app".into();
    container.image = "nginx:latest".into();
    container.ready = "true".into();
    container.status = "Running".into();
    container.age = "5m".into();
    app.data.containers.set_items(vec![container]);

    let lines = render_lines(&mut app, 120, 14);
    let joined = lines.join("\n");

    assert!(lines[4].starts_with('┌'));
    assert!(lines.last().is_some_and(|line| line.ends_with('┘')));
    assert!(joined.contains("s:shell"));
    assert!(joined.contains("⏎:logs"));
  }

  fn seeded_overview_app() -> App {
    let mut app = App::default();
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
