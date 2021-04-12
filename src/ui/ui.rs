use crate::app::{App, RouteId};
use duct::cmd;
use tui::{
  backend::Backend,
  layout::{Alignment, Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  symbols,
  text::{Span, Spans},
  widgets::canvas::{Canvas, Line, Map, MapResolution, Rectangle},
  widgets::{
    Axis, BarChart, Block, BorderType, Borders, Cell, Chart, Dataset, Gauge, LineGauge, List,
    ListItem, Paragraph, Row, Sparkline, Table, Tabs, Wrap,
  },
  Frame,
};

use super::get_help_docs;

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
  draw_logo(f, chunks[1]);
}

fn draw_logo<B: Backend>(f: &mut Frame<B>, area: Rect) {
  let text = vec![Spans::from(
    "Use left/right keys to switch tabs. up/down keys to select context. Press '?' for more help.",
  )];
  let block = Block::default();
  let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
  f.render_widget(paragraph, area);
}

fn draw_overview<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let chunks = vertical_chunks(vec![Constraint::Length(9), Constraint::Min(10)], area);

  draw_status(f, app, chunks[0]);
  draw_active_context_tabs(f, app, chunks[1]);
}

fn draw_help<B: Backend>(f: &mut Frame<B>, area: Rect) {
  let text = vec![Spans::from(
    "Use left/right keys to switch tabs. up/down keys to select context. Press '?' for more help.",
  )];
  let block = layout_block_default("Help (?)");
  let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
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
  draw_help(f, chunks[3])
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
  match app.active_context.clone() {
    Some(active_context) => {
      text = vec![
        Spans::from(vec![
          Span::styled("Context: ", style_secondary()),
          Span::styled(active_context.name, style_primary()),
        ]),
        Spans::from(vec![
          Span::styled("Cluster: ", style_secondary()),
          Span::styled(active_context.cluster, style_primary()),
        ]),
        Spans::from(vec![
          Span::styled("User: ", style_secondary()),
          Span::styled(active_context.user, style_primary()),
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
  let block = layout_block_default("Namespaces (n)");

  let rows = app
    .namespaces
    .iter()
    .map(|c| Row::new(vec![c.name.as_ref(), c.status.as_ref()]).style(style_primary()));

  let table = Table::new(rows)
    .header(table_header_style(vec!["Name", "Status"]))
    .block(block)
    .widths(&[Constraint::Percentage(85), Constraint::Percentage(15)]);

  f.render_widget(table, area);
}

fn draw_nodes<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let block = layout_block_top_border("Nodes");

  let rows = app
    .nodes
    .iter()
    .map(|c| Row::new(vec![c.name.as_ref(), c.status.as_ref()]).style(style_primary()));

  let table = Table::new(rows)
    .header(table_header_style(vec!["Name", "Status"]))
    .block(block)
    .widths(&[Constraint::Percentage(85), Constraint::Percentage(15)]);

  f.render_widget(table, area);
}

fn draw_pods<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let block = layout_block_top_border("Pods (ns: all)");

  let rows = app.pods.iter().map(|c| {
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
    .widths(&[
      Constraint::Percentage(30),
      Constraint::Percentage(40),
      Constraint::Percentage(10),
      Constraint::Percentage(10),
      Constraint::Percentage(10),
    ]);

  f.render_widget(table, area);
}

fn draw_services<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
  let block = layout_block_top_border("Services (ns: all)");

  let rows = app
    .services
    .iter()
    .map(|c| Row::new(vec![c.name.as_ref(), c.type_.as_ref()]).style(style_primary()));

  let table = Table::new(rows)
    .header(table_header_style(vec!["Name", "Type"]))
    .block(block)
    .widths(&[Constraint::Percentage(85), Constraint::Percentage(15)]);

  f.render_widget(table, area);
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
    .highlight_symbol("=> ");

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

fn draw_logs<B: Backend>(f: &mut Frame<B>, _app: &mut App, area: Rect) {
  let chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
    .split(area);
  let colors = [
    Color::Reset,
    Color::Black,
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::Gray,
    Color::DarkGray,
    Color::LightRed,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightBlue,
    Color::LightMagenta,
    Color::LightCyan,
    Color::White,
  ];
  let items: Vec<Row> = colors
    .iter()
    .map(|c| {
      let cells = vec![
        Cell::from(Span::raw(format!("{:?}: ", c))),
        Cell::from(Span::styled("Foreground", Style::default().fg(*c))),
        Cell::from(Span::styled("Background", Style::default().bg(*c))),
      ];
      Row::new(cells)
    })
    .collect();
  let table = Table::new(items)
    .block(Block::default().title("Colors").borders(Borders::ALL))
    .widths(&[
      Constraint::Ratio(1, 3),
      Constraint::Ratio(1, 3),
      Constraint::Ratio(1, 3),
    ]);
  f.render_widget(table, chunks[0]);
}

// Utils

fn title_style<'a>(txt: &'a str) -> Span<'a> {
  Span::styled(txt, style_bold())
}

fn title_style_primary<'a>(txt: &'a str) -> Span<'a> {
  Span::styled(txt, style_primary_bold())
}

fn title_style_secondary<'a>(txt: &'a str) -> Span<'a> {
  Span::styled(txt, style_secondary_bold())
}

fn title_style_success<'a>(txt: &'a str) -> Span<'a> {
  Span::styled(txt, style_success().add_modifier(Modifier::BOLD))
}

fn style_bold() -> Style {
  Style::default().add_modifier(Modifier::BOLD)
}
fn style_success() -> Style {
  Style::default().fg(Color::Green)
}
fn style_failure() -> Style {
  Style::default().fg(Color::Red)
}
fn style_highlight() -> Style {
  Style::default().add_modifier(Modifier::REVERSED)
}
fn style_primary() -> Style {
  Style::default().fg(Color::Cyan)
}
fn style_primary_bold() -> Style {
  style_primary().add_modifier(Modifier::BOLD)
}
fn style_secondary() -> Style {
  Style::default().fg(Color::Yellow)
}
fn style_secondary_bold() -> Style {
  style_secondary().add_modifier(Modifier::BOLD)
}

fn get_gauge_style(enhanced_graphics: bool) -> symbols::line::Set {
  if enhanced_graphics {
    symbols::line::THICK
  } else {
    symbols::line::NORMAL
  }
}

fn table_header_style<'a>(cells: Vec<&'a str>) -> Row<'a> {
  Row::new(cells).style(style_secondary()).bottom_margin(0)
}

fn horizontal_chunks(constraints: Vec<Constraint>, size: Rect) -> Vec<Rect> {
  Layout::default()
    .constraints(constraints.as_ref())
    .direction(Direction::Horizontal)
    .split(size)
}

fn horizontal_chunks_with_margin(
  constraints: Vec<Constraint>,
  size: Rect,
  margin: u16,
) -> Vec<Rect> {
  Layout::default()
    .constraints(constraints.as_ref())
    .direction(Direction::Horizontal)
    .margin(margin)
    .split(size)
}

fn vertical_chunks(constraints: Vec<Constraint>, size: Rect) -> Vec<Rect> {
  Layout::default()
    .constraints(constraints.as_ref())
    .direction(Direction::Vertical)
    .split(size)
}

fn vertical_chunks_with_margin(constraints: Vec<Constraint>, size: Rect, margin: u16) -> Vec<Rect> {
  Layout::default()
    .constraints(constraints.as_ref())
    .direction(Direction::Vertical)
    .margin(margin)
    .split(size)
}

fn layout_block<'a>(title: Span<'a>) -> Block<'a> {
  Block::default().borders(Borders::ALL).title(title)
}

fn layout_block_default<'a>(title: &'a str) -> Block<'a> {
  layout_block(title_style(title))
}

fn layout_block_top_border<'a>(title: &'a str) -> Block<'a> {
  Block::default()
    .borders(Borders::TOP)
    .title(title_style(title))
}
