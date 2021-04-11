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
    .map(|t| Spans::from(Span::styled(*t, Style::default().fg(Color::Green))))
    .collect();
  let tabs = Tabs::new(titles)
    .block(
      Block::default().borders(Borders::ALL).title(Span::styled(
        app.title,
        Style::default()
          .fg(Color::Cyan)
          .add_modifier(Modifier::BOLD),
      )),
    )
    .highlight_style(Style::default().fg(Color::Yellow))
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
  let up_style = Style::default().fg(Color::Green);
  let failure_style = Style::default().fg(Color::Red);
  let rows = app.clis.iter().map(|s| {
    let style = if s.status == true {
      up_style
    } else {
      failure_style
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
  let normal_style = Style::default().fg(Color::Cyan);
  let active_style = Style::default().fg(Color::Green);
  let rows = app.contexts.items.iter().map(|c| {
    let style = if c.is_active == true {
      active_style
    } else {
      normal_style
    };
    Row::new(vec![c.name.as_ref(), c.cluster.as_ref(), c.user.as_ref()]).style(style)
  });

  let table = Table::new(rows)
    .header(
      Row::new(vec!["Context", "Cluster", "User"])
        .style(Style::default().fg(Color::Yellow))
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
    .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
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
    .vertical_margin(2)
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
          Span::from("Context: "),
          Span::styled(active_context.name, Style::default().fg(Color::Yellow)),
        ]),
        Spans::from(vec![
          Span::raw("Cluster: "),
          Span::styled(active_context.cluster, Style::default().fg(Color::Yellow)),
        ]),
        Spans::from(vec![
          Span::raw("User: "),
          Span::styled(active_context.user, Style::default().fg(Color::Yellow)),
        ]),
      ];
    }
    None => {
      text = vec![Spans::from(Span::styled(
        "Context information not found",
        Style::default().fg(Color::Red),
      ))]
    }
  }

  let paragraph = Paragraph::new(text).block(Block::default());
  f.render_widget(paragraph, chunks[0]);

  let cpu_gauge = LineGauge::default()
    .block(Block::default().title("CPU:"))
    .gauge_style(Style::default().fg(Color::Yellow))
    .line_set(get_gauge_style(app.enhanced_graphics))
    .ratio(app.progress);
  f.render_widget(cpu_gauge, chunks[1]);

  let mem_gauge = LineGauge::default()
    .block(Block::default().title("Memory:"))
    .gauge_style(Style::default().fg(Color::Yellow))
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

  f.render_widget(block, area);
}

fn draw_namespaces<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let block = Block::default()
    .borders(Borders::ALL)
    .title(title_style("Namespaces"));

  f.render_widget(block, area);
}

fn draw_pods<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let block = Block::default()
    .borders(Borders::ALL)
    .title(title_style("Pods"));

  f.render_widget(block, area);
}

fn draw_services<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let block = Block::default()
    .borders(Borders::ALL)
    .title(title_style("Services"));

  f.render_widget(block, area);
}

fn draw_charts<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let constraints = if app.show_chart {
    vec![Constraint::Percentage(50), Constraint::Percentage(50)]
  } else {
    vec![Constraint::Percentage(100)]
  };
  let chunks = Layout::default()
    .constraints(constraints)
    .direction(Direction::Horizontal)
    .split(area);
  {
    let chunks = Layout::default()
      .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
      .split(chunks[0]);
    {
      let chunks = Layout::default()
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .direction(Direction::Horizontal)
        .split(chunks[0]);

      // Draw tasks
      let tasks: Vec<ListItem> = app
        .tasks
        .items
        .iter()
        .map(|i| ListItem::new(vec![Spans::from(Span::raw(*i))]))
        .collect();
      let tasks = List::new(tasks)
        .block(Block::default().borders(Borders::ALL).title("List"))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");
      f.render_stateful_widget(tasks, chunks[0], &mut app.tasks.state);

      // Draw logs
      let info_style = Style::default().fg(Color::Blue);
      let warning_style = Style::default().fg(Color::Yellow);
      let error_style = Style::default().fg(Color::Magenta);
      let critical_style = Style::default().fg(Color::Red);
      let logs: Vec<ListItem> = app
        .logs
        .items
        .iter()
        .map(|&(evt, level)| {
          let s = match level {
            "ERROR" => error_style,
            "CRITICAL" => critical_style,
            "WARNING" => warning_style,
            _ => info_style,
          };
          let content = vec![Spans::from(vec![
            Span::styled(format!("{:<9}", level), s),
            Span::raw(evt),
          ])];
          ListItem::new(content)
        })
        .collect();
      let logs = List::new(logs).block(Block::default().borders(Borders::ALL).title("List"));
      f.render_stateful_widget(logs, chunks[1], &mut app.logs.state);
    }

    let barchart = BarChart::default()
      .block(Block::default().borders(Borders::ALL).title("Bar chart"))
      .data(&app.barchart)
      .bar_width(3)
      .bar_gap(2)
      .bar_set(if app.enhanced_graphics {
        symbols::bar::NINE_LEVELS
      } else {
        symbols::bar::THREE_LEVELS
      })
      .value_style(
        Style::default()
          .fg(Color::Black)
          .bg(Color::Green)
          .add_modifier(Modifier::ITALIC),
      )
      .label_style(Style::default().fg(Color::Yellow))
      .bar_style(Style::default().fg(Color::Green));
    f.render_widget(barchart, chunks[1]);
  }
  if app.show_chart {
    let x_labels = vec![
      Span::styled(
        format!("{}", app.signals.window[0]),
        Style::default().add_modifier(Modifier::BOLD),
      ),
      Span::raw(format!(
        "{}",
        (app.signals.window[0] + app.signals.window[1]) / 2.0
      )),
      Span::styled(
        format!("{}", app.signals.window[1]),
        Style::default().add_modifier(Modifier::BOLD),
      ),
    ];
    let datasets = vec![
      Dataset::default()
        .name("data2")
        .marker(symbols::Marker::Dot)
        .style(Style::default().fg(Color::Cyan))
        .data(&app.signals.sin1.points),
      Dataset::default()
        .name("data3")
        .marker(if app.enhanced_graphics {
          symbols::Marker::Braille
        } else {
          symbols::Marker::Dot
        })
        .style(Style::default().fg(Color::Yellow))
        .data(&app.signals.sin2.points),
    ];
    let chart = Chart::new(datasets)
      .block(
        Block::default()
          .title(Span::styled(
            "Chart",
            Style::default()
              .fg(Color::Cyan)
              .add_modifier(Modifier::BOLD),
          ))
          .borders(Borders::ALL),
      )
      .x_axis(
        Axis::default()
          .title("X Axis")
          .style(Style::default().fg(Color::Gray))
          .bounds(app.signals.window)
          .labels(x_labels),
      )
      .y_axis(
        Axis::default()
          .title("Y Axis")
          .style(Style::default().fg(Color::Gray))
          .bounds([-20.0, 20.0])
          .labels(vec![
            Span::styled("-20", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("0"),
            Span::styled("20", Style::default().add_modifier(Modifier::BOLD)),
          ]),
      );
    f.render_widget(chart, chunks[1]);
  }
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

fn get_gauge_style(enhanced_graphics: bool) -> symbols::line::Set {
  if enhanced_graphics {
    symbols::line::THICK
  } else {
    symbols::line::NORMAL
  }
}
