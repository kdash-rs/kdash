use crate::app::App;
use duct::cmd;
use tui::{
  backend::Backend,
  layout::{Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  symbols,
  text::{Span, Spans},
  widgets::canvas::{Canvas, Line, Map, MapResolution, Rectangle},
  widgets::{
    Axis, BarChart, Block, Borders, Cell, Chart, Dataset, Gauge, LineGauge, List, ListItem,
    Paragraph, Row, Sparkline, Table, Tabs, Wrap,
  },
  Frame,
};

pub fn draw<B: Backend>(f: &mut Frame<B>, app: &mut App) {
  let chunks = Layout::default()
    .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
    .split(f.size());
  // draw tabs and help
  draw_header(f, app, chunks[0]);
  // render tab content
  match app.tabs.index {
    0 => draw_overview(f, app, chunks[1]),
    1 => draw_contexts(f, app, chunks[1]),
    _ => {}
  };
}

fn draw_header<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let chunks = Layout::default()
    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
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
    .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
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
    .constraints(
      [
        Constraint::Percentage(30),
        Constraint::Percentage(35),
        Constraint::Percentage(35),
      ]
      .as_ref(),
    )
    .direction(Direction::Horizontal)
    .split(area);

  let up_style = Style::default().fg(Color::Green);
  let failure_style = Style::default().fg(Color::Red);
  let cli_rows = app.CLIs.iter().map(|s| {
    let style = if s.status == true {
      up_style
    } else {
      failure_style
    };
    Row::new(vec![s.name.as_ref(), s.version.as_ref()]).style(style)
  });

  let cli_table = Table::new(cli_rows)
    .block(
      Block::default()
        .title(title_style("CLIs"))
        .borders(Borders::ALL),
    )
    .widths(&[Constraint::Length(15), Constraint::Length(15)]);
  f.render_widget(cli_table, chunks[0]);

  // TODO temp solution
  // let node_out = vec![Spans::from(cmd!("kubectl", "top", "node").read().unwrap())];
  let node_out = vec![Spans::from("test")];

  let nodes = Block::default()
    .borders(Borders::ALL)
    .title(title_style("Contexts"));

  let nodes_text = Paragraph::new(node_out)
    .block(nodes)
    .wrap(Wrap { trim: true });
  f.render_widget(nodes_text, chunks[1]);

  // TODO temp solution
  // let ns_out = vec![Spans::from(cmd!("kubectl", "get", "ns").read().unwrap())];
  let ns_out = vec![Spans::from("test")];

  let ns = Block::default()
    .borders(Borders::ALL)
    .title(title_style("Namespaces"));

  let ns_text = Paragraph::new(ns_out).block(ns).wrap(Wrap { trim: true });
  f.render_widget(ns_text, chunks[2]);

  // let ns_rows = app
  //     .CLIs
  //     .iter()
  //     .map(|s| Row::new(vec![s.name, s.version]).style(up_style));
  // let ns_table = Table::new(ns_rows)
  //     .block(
  //         Block::default()
  //             .title(title_style("Namespaces"))
  //             .borders(Borders::ALL),
  //     )
  //     .widths(&[Constraint::Length(15), Constraint::Length(15)]);
  // f.render_widget(ns_table, chunks[2]);
}

fn draw_active_context<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let chunks = Layout::default()
    .constraints([Constraint::Length(9), Constraint::Min(8)].as_ref())
    .split(area);
  draw_gauges(f, app, chunks[0]);
  draw_charts(f, app, chunks[1]);
}

fn draw_gauges<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
  B: Backend,
{
  let chunks = Layout::default()
    .constraints(
      [
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Length(1),
      ]
      .as_ref(),
    )
    .margin(1)
    .split(area);
  let block = Block::default()
    .borders(Borders::ALL)
    .title(title_style("Current Context"));

  f.render_widget(block, area);

  let label = format!("{:.2}%", app.progress * 100.0);
  let gauge = Gauge::default()
    .block(Block::default().title("Gauge:"))
    .gauge_style(
      Style::default()
        .fg(Color::Magenta)
        .bg(Color::Black)
        .add_modifier(Modifier::ITALIC | Modifier::BOLD),
    )
    .label(label)
    .ratio(app.progress);
  f.render_widget(gauge, chunks[0]);

  //   let sparkline = Sparkline::default()
  //     .block(Block::default().title("Sparkline:"))
  //     .style(Style::default().fg(Color::Green))
  //     .data(&app.sparkline.points)
  //     .bar_set(if app.enhanced_graphics {
  //       symbols::bar::NINE_LEVELS
  //     } else {
  //       symbols::bar::THREE_LEVELS
  //     });
  //   f.render_widget(sparkline, chunks[1]);

  let line_gauge = LineGauge::default()
    .block(Block::default().title("LineGauge:"))
    .gauge_style(Style::default().fg(Color::Magenta))
    .line_set(if app.enhanced_graphics {
      symbols::line::THICK
    } else {
      symbols::line::NORMAL
    })
    .ratio(app.progress);
  f.render_widget(line_gauge, chunks[2]);
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
    "Use left/right arrow keys to switch tabs. Press 'q' to quit. Press '?' for more help.",
  )];
  let block = Block::default()
    .borders(Borders::ALL)
    .title(title_style("Help"));
  let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
  f.render_widget(paragraph, area);
}

fn draw_contexts<B>(f: &mut Frame<B>, _app: &mut App, area: Rect)
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
