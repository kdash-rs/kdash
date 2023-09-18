mod help;
mod overview;
pub mod resource_tabs;
pub mod utils;

use ratatui::{
  backend::Backend,
  layout::{Alignment, Constraint, Rect},
  text::{Line, Span, Text},
  widgets::{Block, Borders, Paragraph, Tabs, Wrap},
  Frame,
};

use self::{
  help::draw_help,
  overview::draw_overview,
  utils::{
    horizontal_chunks_with_margin, layout_block, style_default, style_failure, style_help,
    style_main_background, style_primary, style_secondary, title_style_logo, vertical_chunks,
  },
};
use crate::app::{
  contexts::ContextResource, metrics::UtilizationResource, models::AppResource, ActiveBlock, App,
  RouteId,
};

pub static HIGHLIGHT: &str = "=> ";

pub fn draw<B: Backend>(f: &mut Frame<'_, B>, app: &mut App) {
  let block = Block::default().style(style_main_background(app.light_theme));
  f.render_widget(block, f.size());

  let chunks = if !app.api_error.is_empty() {
    let chunks = vertical_chunks(
      vec![
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(0),
      ],
      f.size(),
    );
    draw_app_error(f, app, chunks[1]);
    chunks
  } else {
    vertical_chunks(vec![Constraint::Length(3), Constraint::Min(0)], f.size())
  };

  // draw header and logo
  draw_app_header(f, app, chunks[0]);

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

fn draw_app_header<B: Backend>(f: &mut Frame<'_, B>, app: &App, area: Rect) {
  let chunks =
    horizontal_chunks_with_margin(vec![Constraint::Length(75), Constraint::Min(0)], area, 1);

  let titles = app
    .main_tabs
    .items
    .iter()
    .map(|t| Line::from(Span::styled(&t.title, style_default(app.light_theme))))
    .collect();
  let tabs = Tabs::new(titles)
    .block(layout_block(title_style_logo(app.title, app.light_theme)))
    .highlight_style(style_secondary(app.light_theme))
    .select(app.main_tabs.index);

  f.render_widget(tabs, area);
  draw_header_text(f, app, chunks[1]);
}

fn draw_header_text<B: Backend>(f: &mut Frame<'_, B>, app: &App, area: Rect) {
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

fn draw_app_error<B: Backend>(f: &mut Frame<'_, B>, app: &App, size: Rect) {
  let block = Block::default()
    .title(" Error | close <esc> ")
    .style(style_failure(app.light_theme))
    .borders(Borders::ALL);

  let mut text = Text::from(app.api_error.clone());
  text.patch_style(style_failure(app.light_theme));

  let paragraph = Paragraph::new(text)
    .style(style_primary(app.light_theme))
    .block(block)
    .wrap(Wrap { trim: true });
  f.render_widget(paragraph, size);
}
