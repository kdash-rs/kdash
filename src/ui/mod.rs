use rand::RngExt;
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
    default_part, help_part, horizontal_chunks_with_margin, mixed_bold_line, mixed_line,
    split_hint_suffix, style_failure, style_header, style_header_text, style_main_background,
    style_primary, style_secondary, vertical_chunks,
  },
};
use crate::app::{
  contexts::ContextResource, metrics::UtilizationResource, models::AppResource,
  troubleshoot::TroubleshootResource, ActiveBlock, App, RouteId,
};

pub static HIGHLIGHT: &str = "=> ";

pub fn draw(f: &mut Frame<'_>, app: &mut App) {
  let block = Block::default().style(style_main_background(app.light_theme));
  f.render_widget(block, f.area());

  let chunks = if !app.api_error.is_empty() {
    let chunks = vertical_chunks(
      vec![
        Constraint::Length(1), // title
        Constraint::Length(3), // header tabs
        Constraint::Length(3), // error
        Constraint::Min(0),    // main tabs
      ],
      f.area(),
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
    nw_loading_indicator(app.is_loading())
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
      [help_part("<↑↓> scroll | <enter> select | </> filter | <?> help ")],
      app.light_theme,
    )],
    RouteId::Home => vec![mixed_line(
      [help_part(
        "<←→> switch tabs | <char> select block | <↑↓> scroll | <enter> select | </> filter | <?> help ",
      )],
      app.light_theme,
    )],
    RouteId::Utilization => vec![mixed_line(
      [help_part("<↑↓> scroll | </> filter | <g> cycle through grouping | <?> help ")],
      app.light_theme,
    )],
    RouteId::Troubleshoot => vec![mixed_line(
      [help_part("<↑↓> scroll | </> filter | <?> help ")],
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
      [default_part(" Error "), help_part("| close <esc> ")],
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
