use crate::app::App;
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

pub fn draw<B: Backend>(f: &mut Frame<B>, app: &mut App) {
  let chunks = Layout::default()
    .constraints([Constraint::Length(3), Constraint::Min(10)].as_ref())
    .split(f.size());
  // draw tabs and help
  draw_header(f, app, chunks[0]);
  // render tab content
  match app.tabs.index {
    0 => draw_overview(f, app, chunks[1]),
    1 => draw_logs(f, app, chunks[1]),
    _ => {}
  };
}

fn draw_header<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let chunks = Layout::default()
    .constraints([Constraint::Length(75), Constraint::Min(0)].as_ref())
    .direction(Direction::Horizontal)
    .split(area);

  let titles = app
    .tabs
    .titles
    .iter()
    .map(|t| Spans::from(Span::styled(*t, style_success())))
    .collect();
  let tabs = Tabs::new(titles)
    .block(
      Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(app.title, style_primary_bold())),
    )
    .highlight_style(style_secondary())
    .select(app.tabs.index);

  f.render_widget(tabs, chunks[0]);

  draw_help(f, chunks[1])
}

fn draw_overview<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let chunks = Layout::default()
    .constraints([Constraint::Length(9), Constraint::Min(10)].as_ref())
    .direction(Direction::Vertical)
    .split(area);

  draw_status(f, app, chunks[0]);
  draw_active_context(f, app, chunks[1]);
}

fn draw_status<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let chunks = Layout::default()
    .constraints([Constraint::Length(30), Constraint::Min(10)].as_ref())
    .direction(Direction::Horizontal)
    .split(area);

  draw_cli_status(f, app, chunks[0]);
  draw_contexts(f, app, chunks[1]);
}

fn draw_cli_status<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let rows = app.clis.iter().map(|s| {
    let style = if s.status == true {
      style_success()
    } else {
      style_failure()
    };
    Row::new(vec![s.name.as_ref(), s.version.as_ref()]).style(style)
  });

  let table = Table::new(rows)
    .block(
      Block::default()
        .title(title_style("CLI Info"))
        .borders(Borders::ALL),
    )
    .widths(&[Constraint::Percentage(50), Constraint::Percentage(50)]);
  f.render_widget(table, area);
}

fn draw_contexts<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let rows = app.contexts.items.iter().map(|c| {
    let style = if c.is_active == true {
      style_success()
    } else {
      style_primary()
    };
    Row::new(vec![c.name.as_ref(), c.cluster.as_ref(), c.user.as_ref()]).style(style)
  });

  let table = Table::new(rows)
    .header(
      Row::new(vec!["Context", "Cluster", "User"])
        .style(style_secondary())
        .bottom_margin(0),
    )
    .block(
      Block::default()
        .title(title_style("Contexts"))
        .borders(Borders::ALL),
    )
    .widths(&[
      Constraint::Percentage(34),
      Constraint::Percentage(33),
      Constraint::Percentage(33),
    ])
    .highlight_style(style_highlight())
    .highlight_symbol("=> ");

  f.render_stateful_widget(table, area, &mut app.contexts.state);
}

fn draw_active_context<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let block = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .title(title_style_spl("Current Context"));

  f.render_widget(block, area);

  let chunks = Layout::default()
    .constraints([Constraint::Length(10), Constraint::Min(10)].as_ref())
    .horizontal_margin(1)
    .vertical_margin(1)
    .split(area);

  let top_chunks = Layout::default()
    .constraints(
      [
        Constraint::Percentage(35),
        Constraint::Percentage(35),
        Constraint::Percentage(30),
      ]
      .as_ref(),
    )
    .direction(Direction::Horizontal)
    .split(chunks[0]);

  let bottom_chunks = Layout::default()
    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
    .direction(Direction::Horizontal)
    .split(chunks[1]);

  draw_context_info(f, app, top_chunks[0]);
  draw_nodes(f, app, top_chunks[1]);
  draw_namespaces(f, app, top_chunks[2]);
  draw_pods(f, app, bottom_chunks[0]);
  draw_services(f, app, bottom_chunks[1]);
}

fn draw_context_info<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let chunks = Layout::default()
    .constraints(
      [
        Constraint::Length(4),
        Constraint::Min(2),
        Constraint::Min(2),
      ]
      .as_ref(),
    )
    .margin(1)
    .split(area);
  let block = Block::default()
    .borders(Borders::ALL)
    .title(title_style("Info"));

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
    .block(Block::default().title("CPU:"))
    .gauge_style(style_secondary())
    .line_set(get_gauge_style(app.enhanced_graphics))
    .ratio(app.progress);
  f.render_widget(cpu_gauge, chunks[1]);

  let mem_gauge = LineGauge::default()
    .block(Block::default().title("Memory:"))
    .gauge_style(style_secondary())
    .line_set(get_gauge_style(app.enhanced_graphics))
    .ratio(app.progress);
  f.render_widget(mem_gauge, chunks[2]);
}

fn draw_nodes<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let block = Block::default()
    .borders(Borders::ALL)
    .title(title_style("Nodes"));

  let rows = app
    .nodes
    .iter()
    .map(|c| Row::new(vec![c.name.as_ref(), c.status.as_ref()]).style(style_primary()));

  let table = Table::new(rows)
    .header(
      Row::new(vec!["Name", "Status"])
        .style(style_secondary())
        .bottom_margin(0),
    )
    .block(block)
    .widths(&[Constraint::Percentage(85), Constraint::Percentage(15)]);

  f.render_widget(table, area);
}

fn draw_namespaces<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let block = Block::default()
    .borders(Borders::ALL)
    .title(title_style("Namespaces"));

  let rows = app
    .namespaces
    .iter()
    .map(|c| Row::new(vec![c.name.as_ref(), c.status.as_ref()]).style(style_primary()));

  let table = Table::new(rows)
    .header(
      Row::new(vec!["Name", "Status"])
        .style(style_secondary())
        .bottom_margin(0),
    )
    .block(block)
    .widths(&[Constraint::Percentage(85), Constraint::Percentage(15)]);

  f.render_widget(table, area);
}

fn draw_pods<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let block = Block::default()
    .borders(Borders::ALL)
    .title(title_style("Pods"));

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
    .header(
      Row::new(vec!["Namespace", "Name", "Ready", "Status", "Restarts"])
        .style(style_secondary())
        .bottom_margin(0),
    )
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

fn draw_services<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let block = Block::default()
    .borders(Borders::ALL)
    .title(title_style("Services"));

  let rows = app
    .services
    .iter()
    .map(|c| Row::new(vec![c.name.as_ref(), c.type_.as_ref()]).style(style_primary()));

  let table = Table::new(rows)
    .header(
      Row::new(vec!["Name", "Type"])
        .style(style_secondary())
        .bottom_margin(0),
    )
    .block(block)
    .widths(&[Constraint::Percentage(85), Constraint::Percentage(15)]);

  f.render_widget(table, area);
}

fn draw_help<B>(f: &mut Frame<B>, area: Rect)
where
  B: Backend,
{
  let text = vec![Spans::from(
    "Use left/right keys to switch tabs. up/down keys to select context. Press '?' for more help.",
  )];
  let block = Block::default()
    .borders(Borders::ALL)
    .title(title_style("Help"));
  let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
  f.render_widget(paragraph, area);
}

fn draw_logs<B>(f: &mut Frame<B>, _app: &mut App, area: Rect)
where
  B: Backend,
{
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

fn title_style(txt: &'static str) -> Span {
  Span::styled(txt, Style::default().add_modifier(Modifier::BOLD))
}

fn title_style_spl(txt: &'static str) -> Span {
  Span::styled(
    txt,
    Style::default()
      .fg(Color::Green)
      .add_modifier(Modifier::BOLD),
  )
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
  Style::default()
    .fg(Color::Cyan)
    .add_modifier(Modifier::BOLD)
}
fn style_secondary() -> Style {
  Style::default().fg(Color::Yellow)
}

fn get_gauge_style(enhanced_graphics: bool) -> symbols::line::Set {
  if enhanced_graphics {
    symbols::line::THICK
  } else {
    symbols::line::NORMAL
  }
}
