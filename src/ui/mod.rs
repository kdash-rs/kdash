mod contexts;
mod help;
mod overview;
mod tree;
mod utilization;
mod utils;

use self::contexts::draw_contexts;
use self::help::draw_help;
use self::overview::draw_overview;
use self::utilization::draw_utilization;
use self::utils::{
  horizontal_chunks_with_margin, layout_block, style_default, style_failure, style_help,
  style_main_background, style_primary, style_secondary, title_style_logo, vertical_chunks,
};
use super::app::{App, RouteId};

use tui::{
  backend::Backend,
  layout::{Alignment, Constraint, Rect},
  text::{Span, Spans, Text},
  widgets::{Block, Borders, Paragraph, Tabs, Wrap},
  Frame,
};

static HIGHLIGHT: &str = "=> ";

pub fn draw<B: Backend>(f: &mut Frame<B>, app: &mut App) {
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
      draw_contexts(f, app, last_chunk);
    }
    RouteId::Utilization => {
      draw_utilization(f, app, last_chunk);
    }
    _ => {
      draw_overview(f, app, last_chunk);
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
    .map(|t| Spans::from(Span::styled(t, style_default(app.light_theme))))
    .collect();
  let tabs = Tabs::new(titles)
    .block(layout_block(title_style_logo(app.title)))
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
    RouteId::Home => vec![Spans::from(
      "<left|right>: switch resource tabs | <char> select block | <up|down>: scroll | <enter>: select | <?> more help",
    )],
    RouteId::Utilization => vec![Spans::from(
      "<up|down>: scroll | <g>: cycle through grouping | <?> more help",
    )],
    _ => vec![Spans::from("<?> more help")],
  };
  let paragraph = Paragraph::new(text)
    .style(style_help())
    .block(Block::default())
    .alignment(Alignment::Right)
    .wrap(Wrap { trim: true });
  f.render_widget(paragraph, area);
}

fn draw_app_error<B: Backend>(f: &mut Frame<B>, app: &mut App, size: Rect) {
  let block = Block::default()
    .title("Error | close <esc>")
    .style(style_failure())
    .borders(Borders::ALL);

  let mut text = Text::from(app.api_error.clone());
  text.patch_style(style_failure());

  let paragraph = Paragraph::new(text)
    .style(style_primary())
    .block(block)
    .wrap(Wrap { trim: true });
  f.render_widget(paragraph, size);
}
