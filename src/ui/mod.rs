use rand::Rng;
mod help;
mod overview;
pub mod resource_tabs;
pub mod utils;

use ratatui::{
  layout::{Alignment, Constraint, Rect},
  style::Modifier,
  text::{Line, Span, Text},
  widgets::{Block, Borders, Paragraph, Tabs, Wrap},
  Frame,
};

use self::{
  help::draw_help,
  overview::draw_overview,
  utils::{
    horizontal_chunks_with_margin, style_default, style_failure, style_header, style_header_text,
    style_help, style_main_background, style_primary, style_secondary, vertical_chunks,
  },
};
use crate::app::{
  contexts::ContextResource, metrics::UtilizationResource, models::AppResource, ActiveBlock, App,
  RouteId,
};

pub static HIGHLIGHT: &str = "=> ";

pub fn draw(f: &mut Frame<'_>, app: &mut App) {
  let block = Block::default().style(style_main_background(app.light_theme));
  f.render_widget(block, f.size());

  let chunks = if !app.api_error.is_empty() {
    let chunks = vertical_chunks(
      vec![
        Constraint::Length(1), // title
        Constraint::Length(3), // header tabs
        Constraint::Length(3), // error
        Constraint::Min(0),    // main tabs
      ],
      f.size(),
    );
    draw_app_error(f, app, chunks[2]);
    chunks
  } else {
    vertical_chunks(
      vec![
        Constraint::Length(1), // title
        Constraint::Length(3), // header tabs
        Constraint::Min(0),    // main tabs
      ],
      f.size(),
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
    _ => {
      draw_overview(f, app, last_chunk);
    }
  }
}

fn draw_app_title(f: &mut Frame<'_>, app: &App, area: Rect) {
  let title = Paragraph::new(Span::styled(
    app.title,
    style_header_text(app.light_theme).add_modifier(Modifier::BOLD),
  ))
  .style(style_header())
  .block(Block::default())
  .alignment(Alignment::Left);
  f.render_widget(title, area);

  let text = format!(
    "v{} with ♥ in Rust {} ",
    env!("CARGO_PKG_VERSION"),
    nw_loading_indicator(app.is_loading)
  );

  let meta = Paragraph::new(Span::styled(text, style_header_text(app.light_theme)))
    .style(style_header())
    .block(Block::default())
    .alignment(Alignment::Right);
  f.render_widget(meta, area);
}

// loading animation frames
const FRAMES: &[&str] = &["⠋⠴", "⠦⠙", "⠏⠼", "⠧⠹", "⠯⠽"];

fn nw_loading_indicator<'a>(loading: bool) -> &'a str {
  if loading {
    FRAMES[rand::thread_rng().gen_range(0..FRAMES.len())]
  } else {
    ""
  }
}

fn draw_app_header(f: &mut Frame<'_>, app: &App, area: Rect) {
  let chunks =
    horizontal_chunks_with_margin(vec![Constraint::Length(60), Constraint::Min(0)], area, 1);

  let titles: Vec<_> = app
    .main_tabs
    .items
    .iter()
    .map(|t| Line::from(Span::styled(&t.title, style_default(app.light_theme))))
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
    RouteId::Contexts => vec![Line::from("<↑↓> scroll | <enter> select | <?> help ")],
    RouteId::Home => vec![Line::from(
      "<←→> switch tabs | <char> select block | <↑↓> scroll | <enter> select | <?> help ",
    )],
    RouteId::Utilization => vec![Line::from(
      "<↑↓> scroll | <g> cycle through grouping | <?> help ",
    )],
    RouteId::HelpMenu => vec![],
  };
  let paragraph = Paragraph::new(text)
    .style(style_help(app.light_theme))
    .block(Block::default())
    .alignment(Alignment::Right);
  f.render_widget(paragraph, area);
}

fn draw_app_error(f: &mut Frame<'_>, app: &App, size: Rect) {
  let block = Block::default()
    .title(" Error | close <esc> ")
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
