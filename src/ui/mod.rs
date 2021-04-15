mod contexts;
mod help;
mod overview;
mod utils;

use super::app::{App, RouteId};
use contexts::draw_contexts;
use help::draw_help_menu;
use overview::draw_overview;
use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  text::{Span, Spans, Text},
  widgets::{Block, Borders, Paragraph, Tabs, Wrap},
  Frame,
};

use utils::{
  centered_rect, horizontal_chunks_with_margin, layout_block, style_failure, style_help,
  style_main_background, style_primary, style_secondary, style_success, title_style_primary,
  vertical_chunks,
};

static HIGHLIGHT: &'static str = "=> ";

pub fn draw<B: Backend>(f: &mut Frame<B>, app: &mut App) {
  let block = Block::default().style(style_main_background(app.light_theme));
  f.render_widget(block, f.size());
  let chunks = vertical_chunks(vec![Constraint::Length(3), Constraint::Min(0)], f.size());

  // draw header and logo
  draw_app_header(f, app, chunks[0]);

  match app.get_current_route().id {
    RouteId::HelpMenu => {
      draw_help_menu(f, app, chunks[1]);
    }
    RouteId::Error => {
      if app.api_error.is_empty() {
        draw_overview(f, app, chunks[1]);
      } else {
        draw_error_popup(f, app, chunks[1]);
      }
    }
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
        "<up|down>: scroll context | <enter>: select context | <?> more help",
      )],
      _ => vec![Spans::from(
        "<left|right>: switch resource tabs | <char> select block | <up|down>: scroll | <enter>: select | <?> more help",
      )],
    };
  let paragraph = Paragraph::new(text)
    .style(style_help())
    .block(Block::default())
    .wrap(Wrap { trim: true });
  f.render_widget(paragraph, area);
}

fn draw_error_popup<B: Backend>(f: &mut Frame<B>, app: &mut App, size: Rect) {
  let block = Block::default().title("Error").borders(Borders::ALL);
  let area = centered_rect(60, 20, size);

  let mut text = Text::from(app.api_error.clone());
  text.patch_style(style_failure());

  let paragraph = Paragraph::new(text).style(style_primary()).block(block);
  f.render_widget(paragraph, area);
}
