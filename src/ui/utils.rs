use tui::{
  backend::Backend,
  layout::{Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  symbols,
  text::{Span, Text},
  widgets::{Block, Borders, Paragraph, Row},
  Frame,
};
// Utils

pub fn title_style(txt: &str) -> Span {
  Span::styled(txt, style_bold())
}

pub fn title_style_primary(txt: &str) -> Span {
  Span::styled(txt, style_primary_bold())
}

pub fn title_style_secondary(txt: &str) -> Span {
  Span::styled(txt, style_secondary_bold())
}

pub fn style_bold() -> Style {
  Style::default().add_modifier(Modifier::BOLD)
}
pub fn style_success() -> Style {
  Style::default().fg(Color::Green)
}
pub fn style_failure() -> Style {
  Style::default().fg(Color::Red)
}
pub fn style_highlight() -> Style {
  Style::default().add_modifier(Modifier::REVERSED)
}
pub fn style_primary() -> Style {
  Style::default().fg(Color::Cyan)
}
pub fn style_help() -> Style {
  Style::default().fg(Color::Blue)
}
pub fn style_primary_bold() -> Style {
  style_primary().add_modifier(Modifier::BOLD)
}
pub fn style_secondary() -> Style {
  Style::default().fg(Color::Yellow)
}
pub fn style_secondary_bold() -> Style {
  style_secondary().add_modifier(Modifier::BOLD)
}
pub fn style_main_background(light: bool) -> Style {
  match light {
    true => Style::default().bg(Color::White).fg(Color::Magenta),
    false => Style::default().bg(Color::Rgb(35, 50, 55)).fg(Color::White),
  }
}

pub fn get_gauge_style(enhanced_graphics: bool) -> symbols::line::Set {
  if enhanced_graphics {
    symbols::line::THICK
  } else {
    symbols::line::NORMAL
  }
}

pub fn table_header_style(cells: Vec<&str>) -> Row {
  Row::new(cells).style(style_secondary()).bottom_margin(0)
}

pub fn horizontal_chunks(constraints: Vec<Constraint>, size: Rect) -> Vec<Rect> {
  Layout::default()
    .constraints(constraints.as_ref())
    .direction(Direction::Horizontal)
    .split(size)
}

pub fn horizontal_chunks_with_margin(
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

pub fn vertical_chunks(constraints: Vec<Constraint>, size: Rect) -> Vec<Rect> {
  Layout::default()
    .constraints(constraints.as_ref())
    .direction(Direction::Vertical)
    .split(size)
}

pub fn vertical_chunks_with_margin(
  constraints: Vec<Constraint>,
  size: Rect,
  margin: u16,
) -> Vec<Rect> {
  Layout::default()
    .constraints(constraints.as_ref())
    .direction(Direction::Vertical)
    .margin(margin)
    .split(size)
}

pub fn layout_block(title: Span) -> Block {
  Block::default().borders(Borders::ALL).title(title)
}

pub fn layout_block_default(title: &str) -> Block {
  layout_block(title_style(title))
}

pub fn layout_block_top_border(title: &str) -> Block {
  Block::default()
    .borders(Borders::TOP)
    .title(title_style(title))
}

/// helper function to create a centered rect using up
/// certain percentage of the available rect `r`
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
  let popup_layout = Layout::default()
    .direction(Direction::Vertical)
    .constraints(
      [
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
      ]
      .as_ref(),
    )
    .split(r);

  Layout::default()
    .direction(Direction::Horizontal)
    .constraints(
      [
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
      ]
      .as_ref(),
    )
    .split(popup_layout[1])[1]
}

pub fn loading<B: Backend>(f: &mut Frame<B>, block: Block, area: Rect, is_loading: bool) {
  if is_loading {
    let text = "\n\n Loading ...\n\n".to_string();
    let mut text = Text::from(text);
    text.patch_style(style_secondary());

    // Contains the text
    let paragraph = Paragraph::new(text).style(style_success()).block(block);
    f.render_widget(paragraph, area);
  } else {
    f.render_widget(block, area)
  }
}

pub fn draw_placeholder<B: Backend>(f: &mut Frame<B>, area: Rect) {
  let block = layout_block_top_border("TODO Placeholder");

  f.render_widget(block, area);
}
