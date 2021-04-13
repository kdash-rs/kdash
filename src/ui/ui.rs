use crate::app::{App, RouteId};
use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  text::{Span, Spans, Text},
  widgets::{Block, Borders, LineGauge, Paragraph, Row, Table, Tabs, Wrap},
  Frame,
};

use super::get_help_docs;
use super::utils::{
  get_gauge_style, horizontal_chunks, horizontal_chunks_with_margin, layout_block,
  layout_block_default, layout_block_top_border, style_failure, style_help, style_highlight,
  style_primary, style_secondary, style_success, table_header_style, title_style_primary,
  title_style_secondary, vertical_chunks, vertical_chunks_with_margin,
};
use crate::banner::BANNER;

static HIGHLIGHT: &'static str = "=> ";

pub fn draw<B: Backend>(f: &mut Frame<B>, app: &mut App) {
  let chunks = vertical_chunks(vec![Constraint::Length(3), Constraint::Min(0)], f.size());

  // draw header and logo
  draw_app_header(f, app, chunks[0]);

  match app.get_current_route().id {
    RouteId::HelpMenu => {
      draw_help_menu(f, app, chunks[1]);
    }
    //   RouteId::Error => {
    //     draw_error_screen(f, app);
    //   }
    RouteId::Contexts => {
      draw_contexts(f, app, chunks[1]);
    }
    _ => {
      draw_overview(f, app, chunks[1]);
    }
  }
}

fn draw_app_header<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let chunks =
    horizontal_chunks_with_margin(vec![Constraint::Length(75), Constraint::Min(0)], area, 1);

  let titles = app
    .main_tabs
    .titles
    .iter()
    .map(|t| Spans::from(Span::styled(*t, style_success())))
    .collect();
  let tabs = Tabs::new(titles)
    .block(layout_block(title_style_primary(app.title)))
    .highlight_style(style_secondary())
    .select(app.main_tabs.index);

  f.render_widget(tabs, area);
  draw_header_text(f, app, chunks[1]);
}

fn draw_header_text<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let text = match app.get_current_route().id {
    RouteId::Contexts => vec![Spans::from(
      "<up|down>: scroll context | <enter>: change context | <?> more help",
    )],
    _ => vec![Spans::from(
      "<left|right>: switch resource tabs | <up|down>: scroll selected block | <?> more help",
    )],
  };
  let paragraph = Paragraph::new(text)
    .style(style_help())
    .block(Block::default())
    .wrap(Wrap { trim: true });
  f.render_widget(paragraph, area);
}

fn draw_overview<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let chunks = vertical_chunks(vec![Constraint::Length(9), Constraint::Min(10)], area);

  draw_status(f, app, chunks[0]);
  draw_active_context_tabs(f, app, chunks[1]);
}

fn draw_logo<B: Backend>(f: &mut Frame<B>, area: Rect) {
  // Banner text with correct styling
  let text = format!("{}\nv{} with ♥ in Rust", BANNER, env!("CARGO_PKG_VERSION"));
  let mut text = Text::from(text);
  text.patch_style(style_success());

  // Contains the banner
  let paragraph = Paragraph::new(text)
    .style(style_success())
    .block(Block::default().borders(Borders::ALL));
  f.render_widget(paragraph, area);
}

fn draw_status<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let chunks = horizontal_chunks(
    vec![
      Constraint::Length(30),
      Constraint::Min(10),
      Constraint::Length(40),
      Constraint::Length(30),
    ],
    area,
  );

  draw_cli_status(f, app, chunks[0]);
  draw_context_info(f, app, chunks[1]);
  draw_namespaces(f, app, chunks[2]);
  draw_logo(f, chunks[3])
}

fn draw_cli_status<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let rows = app.clis.iter().map(|s| {
    let style = if s.status == true {
      style_success()
    } else {
      style_failure()
    };
    Row::new(vec![s.name.as_ref(), s.version.as_ref()]).style(style)
  });

  let table = Table::new(rows)
    .block(layout_block_default("CLI Info"))
    .widths(&[Constraint::Percentage(50), Constraint::Percentage(50)]);
  f.render_widget(table, area);
}

fn draw_active_context_tabs<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let chunks =
    vertical_chunks_with_margin(vec![Constraint::Length(2), Constraint::Min(0)], area, 1);

  let titles = app
    .context_tabs
    .titles
    .iter()
    .map(|t| Spans::from(Span::styled(*t, style_success())))
    .collect();
  let tabs = Tabs::new(titles)
    .block(layout_block_default("Resources"))
    .highlight_style(style_secondary())
    .select(app.context_tabs.index);

  f.render_widget(tabs, area);
  // render tab content
  match app.context_tabs.index {
    0 => draw_pods(f, app, chunks[1]),
    1 => draw_services(f, app, chunks[1]),
    2 => draw_nodes(f, app, chunks[1]),
    _ => {}
  };
}

fn draw_context_info<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let chunks = vertical_chunks_with_margin(
    vec![
      Constraint::Length(3),
      Constraint::Min(2),
      Constraint::Min(2),
    ],
    area,
    1,
  );

  let block = layout_block_default("Context Info");

  f.render_widget(block, area);

  let text;
  match &app.active_context {
    Some(active_context) => {
      text = vec![
        Spans::from(vec![
          Span::styled("Context: ", style_secondary()),
          Span::styled(&active_context.name, style_primary()),
        ]),
        Spans::from(vec![
          Span::styled("Cluster: ", style_secondary()),
          Span::styled(&active_context.cluster, style_primary()),
        ]),
        Spans::from(vec![
          Span::styled("User: ", style_secondary()),
          Span::styled(&active_context.user, style_primary()),
        ]),
      ];
    }
    None => {
      text = vec![Spans::from(Span::styled(
        "Context information not found",
        style_failure(),
      ))]
    }
  }

  let paragraph = Paragraph::new(text).block(Block::default());
  f.render_widget(paragraph, chunks[0]);

  let cpu_gauge = LineGauge::default()
    .block(Block::default().title(title_style_secondary("CPU:")))
    .gauge_style(style_primary())
    .line_set(get_gauge_style(app.enhanced_graphics))
    .ratio(app.progress);
  f.render_widget(cpu_gauge, chunks[1]);

  let mem_gauge = LineGauge::default()
    .block(Block::default().title(title_style_secondary("Memory:")))
    .gauge_style(style_primary())
    .line_set(get_gauge_style(app.enhanced_graphics))
    .ratio(app.progress);
  f.render_widget(mem_gauge, chunks[2]);
}

fn draw_namespaces<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let block = layout_block_default("Namespaces <n>");

  let rows = app
    .namespaces
    .items
    .iter()
    .map(|c| Row::new(vec![c.name.as_ref(), c.status.as_ref()]).style(style_primary()));

  let table = Table::new(rows)
    .header(table_header_style(vec!["Name", "Status"]))
    .block(block)
    .highlight_style(style_highlight())
    .highlight_symbol(HIGHLIGHT)
    .widths(&[Constraint::Percentage(80), Constraint::Percentage(20)]);

  f.render_stateful_widget(table, area, &mut app.namespaces.state);
}

fn draw_nodes<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let block = layout_block_top_border("Nodes");

  let rows = app
    .nodes
    .items
    .iter()
    .map(|c| Row::new(vec![c.name.as_ref(), c.status.as_ref()]).style(style_primary()));

  let table = Table::new(rows)
    .header(table_header_style(vec!["Name", "Status"]))
    .block(block)
    .highlight_style(style_highlight())
    .highlight_symbol(HIGHLIGHT)
    .widths(&[Constraint::Percentage(85), Constraint::Percentage(15)]);

  f.render_stateful_widget(table, area, &mut app.nodes.state);
}

fn draw_pods<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let title = format!("Pods (all) [{}]", app.pods.items.len());
  let block = layout_block_top_border(title.as_str());

  let rows = app.pods.items.iter().map(|c| {
    Row::new(vec![
      c.namespace.as_ref(),
      c.name.as_ref(),
      c.ready.as_ref(),
      c.status.as_ref(),
      "",
    ])
    .style(style_primary())
  });

  let table = Table::new(rows)
    .header(table_header_style(vec![
      "Namespace",
      "Name",
      "Ready",
      "Status",
      "Restarts",
    ]))
    .block(block)
    .highlight_style(style_highlight())
    .highlight_symbol(HIGHLIGHT)
    .widths(&[
      Constraint::Percentage(30),
      Constraint::Percentage(40),
      Constraint::Percentage(10),
      Constraint::Percentage(10),
      Constraint::Percentage(10),
    ]);

  f.render_stateful_widget(table, area, &mut app.pods.state);
}

fn draw_services<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let block = layout_block_top_border("Services (ns: all)");

  let rows = app
    .services
    .items
    .iter()
    .map(|c| Row::new(vec![c.name.as_ref(), c.type_.as_ref()]).style(style_primary()));

  let table = Table::new(rows)
    .header(table_header_style(vec!["Name", "Type"]))
    .block(block)
    .highlight_style(style_highlight())
    .highlight_symbol(HIGHLIGHT)
    .widths(&[Constraint::Percentage(85), Constraint::Percentage(15)]);

  f.render_stateful_widget(table, area, &mut app.services.state);
}

fn draw_contexts<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let rows = app.contexts.items.iter().map(|c| {
    let style = if c.is_active == true {
      style_success()
    } else {
      style_primary()
    };
    Row::new(vec![c.name.as_ref(), c.cluster.as_ref(), c.user.as_ref()]).style(style)
  });

  let table = Table::new(rows)
    .header(table_header_style(vec!["Context", "Cluster", "User"]))
    .block(layout_block_default("Contexts"))
    .widths(&[
      Constraint::Percentage(34),
      Constraint::Percentage(33),
      Constraint::Percentage(33),
    ])
    .highlight_style(style_highlight())
    .highlight_symbol(HIGHLIGHT);

  f.render_stateful_widget(table, area, &mut app.contexts.state);
}

fn draw_help_menu<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let chunks = vertical_chunks(vec![Constraint::Percentage(100)], area);

  // Create a one-column table to avoid flickering due to non-determinism when
  // resolving constraints on widths of table columns.
  let format_row =
    |r: Vec<String>| -> Vec<String> { vec![format!("{:50}{:40}{:20}", r[0], r[1], r[2])] };

  let header = ["Description", "Event", "Context"];
  let header = format_row(header.iter().map(|s| s.to_string()).collect());

  let help_docs = get_help_docs();
  let help_docs = help_docs
    .into_iter()
    .map(format_row)
    .collect::<Vec<Vec<String>>>();
  let help_docs = &help_docs[app.help_menu_offset as usize..];

  let rows = help_docs
    .iter()
    .map(|item| Row::new(item.clone()).style(style_primary()));

  let help_menu = Table::new(rows)
    .header(Row::new(header).style(style_secondary()).bottom_margin(0))
    .block(layout_block_default("Help (press <Esc> to go back)"))
    .widths(&[Constraint::Max(110)]);
  f.render_widget(help_menu, chunks[0]);
}
