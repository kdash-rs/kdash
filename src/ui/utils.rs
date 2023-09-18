use std::{collections::BTreeMap, rc::Rc};

use ratatui::{
  backend::Backend,
  layout::{Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  symbols,
  text::{Line, Span, Text},
  widgets::{Block, Borders, Paragraph, Row, Table, Wrap},
  Frame,
};
use serde::Serialize;

use super::HIGHLIGHT;
use crate::app::{
  models::{KubeResource, StatefulTable},
  ActiveBlock, App,
};
// Utils

pub static COPY_HINT: &str = "| copy <c>";
pub static DESCRIBE_AND_YAML_HINT: &str = "| describe <d> | yaml <y> ";
pub static DESCRIBE_YAML_AND_ESC_HINT: &str = "| describe <d> | yaml <y> | back to menu <esc> ";
pub static DESCRIBE_YAML_DECODE_AND_ESC_HINT: &str =
  "| describe <d> | yaml <y> | decode <x> | back to menu <esc> ";

// default colors
pub const COLOR_TEAL: Color = Color::Rgb(35, 50, 55);
pub const COLOR_CYAN: Color = Color::Rgb(0, 230, 230);
pub const COLOR_LIGHT_BLUE: Color = Color::Rgb(138, 196, 255);
pub const COLOR_YELLOW: Color = Color::Rgb(249, 229, 113);
pub const COLOR_GREEN: Color = Color::Rgb(72, 213, 150);
pub const COLOR_RED: Color = Color::Rgb(249, 167, 164);
pub const COLOR_ORANGE: Color = Color::Rgb(255, 170, 66);
pub const COLOR_WHITE: Color = Color::Rgb(255, 255, 255);
// light theme colors
pub const COLOR_MAGENTA: Color = Color::Rgb(139, 0, 139);
pub const COLOR_GRAY: Color = Color::Rgb(91, 87, 87);
pub const COLOR_BLUE: Color = Color::Rgb(0, 82, 163);
pub const COLOR_GREEN_DARK: Color = Color::Rgb(20, 97, 73);
pub const COLOR_RED_DARK: Color = Color::Rgb(173, 25, 20);
pub const COLOR_ORANGE_DARK: Color = Color::Rgb(184, 49, 15);

#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Styles {
  Default,
  Logo,
  Failure,
  Warning,
  Success,
  Primary,
  Secondary,
  Help,
  Background,
}

pub fn theme_styles(light: bool) -> BTreeMap<Styles, Style> {
  if light {
    BTreeMap::from([
      (Styles::Default, Style::default().fg(COLOR_GRAY)),
      (Styles::Logo, Style::default().fg(COLOR_GREEN_DARK)),
      (Styles::Failure, Style::default().fg(COLOR_RED_DARK)),
      (Styles::Warning, Style::default().fg(COLOR_ORANGE_DARK)),
      (Styles::Success, Style::default().fg(COLOR_GREEN_DARK)),
      (Styles::Primary, Style::default().fg(COLOR_BLUE)),
      (Styles::Secondary, Style::default().fg(COLOR_MAGENTA)),
      (Styles::Help, Style::default().fg(COLOR_BLUE)),
      (
        Styles::Background,
        Style::default().bg(COLOR_WHITE).fg(COLOR_GRAY),
      ),
    ])
  } else {
    BTreeMap::from([
      (Styles::Default, Style::default().fg(COLOR_WHITE)),
      (Styles::Logo, Style::default().fg(COLOR_GREEN)),
      (Styles::Failure, Style::default().fg(COLOR_RED)),
      (Styles::Warning, Style::default().fg(COLOR_ORANGE)),
      (Styles::Success, Style::default().fg(COLOR_GREEN)),
      (Styles::Primary, Style::default().fg(COLOR_CYAN)),
      (Styles::Secondary, Style::default().fg(COLOR_YELLOW)),
      (Styles::Help, Style::default().fg(COLOR_LIGHT_BLUE)),
      (
        Styles::Background,
        Style::default().bg(COLOR_TEAL).fg(COLOR_WHITE),
      ),
    ])
  }
}

pub fn title_style(txt: &str) -> Span<'_> {
  Span::styled(txt, style_bold())
}

pub fn title_style_logo(txt: &str, light: bool) -> Span<'_> {
  Span::styled(
    txt,
    style_logo(light)
      .add_modifier(Modifier::BOLD)
      .add_modifier(Modifier::ITALIC),
  )
}

pub fn style_bold() -> Style {
  Style::default().add_modifier(Modifier::BOLD)
}

pub fn style_default(light: bool) -> Style {
  *theme_styles(light).get(&Styles::Default).unwrap()
}
pub fn style_logo(light: bool) -> Style {
  *theme_styles(light).get(&Styles::Logo).unwrap()
}
pub fn style_failure(light: bool) -> Style {
  *theme_styles(light).get(&Styles::Failure).unwrap()
}
pub fn style_warning(light: bool) -> Style {
  *theme_styles(light).get(&Styles::Warning).unwrap()
}
pub fn style_success(light: bool) -> Style {
  *theme_styles(light).get(&Styles::Success).unwrap()
}
pub fn style_primary(light: bool) -> Style {
  *theme_styles(light).get(&Styles::Primary).unwrap()
}
pub fn style_help(light: bool) -> Style {
  *theme_styles(light).get(&Styles::Help).unwrap()
}

pub fn style_secondary(light: bool) -> Style {
  *theme_styles(light).get(&Styles::Secondary).unwrap()
}

pub fn style_main_background(light: bool) -> Style {
  *theme_styles(light).get(&Styles::Background).unwrap()
}

pub fn style_highlight() -> Style {
  Style::default().add_modifier(Modifier::REVERSED)
}

pub fn get_gauge_style(enhanced_graphics: bool) -> symbols::line::Set {
  if enhanced_graphics {
    symbols::line::THICK
  } else {
    symbols::line::NORMAL
  }
}

pub fn table_header_style(cells: Vec<&str>, light: bool) -> Row<'_> {
  Row::new(cells).style(style_default(light)).bottom_margin(0)
}

pub fn horizontal_chunks(constraints: Vec<Constraint>, size: Rect) -> Rc<[Rect]> {
  Layout::default()
    .constraints(<Vec<Constraint> as AsRef<[Constraint]>>::as_ref(
      &constraints,
    ))
    .direction(Direction::Horizontal)
    .split(size)
}

pub fn horizontal_chunks_with_margin(
  constraints: Vec<Constraint>,
  size: Rect,
  margin: u16,
) -> Rc<[Rect]> {
  Layout::default()
    .constraints(<Vec<Constraint> as AsRef<[Constraint]>>::as_ref(
      &constraints,
    ))
    .direction(Direction::Horizontal)
    .margin(margin)
    .split(size)
}

pub fn vertical_chunks(constraints: Vec<Constraint>, size: Rect) -> Rc<[Rect]> {
  Layout::default()
    .constraints(<Vec<Constraint> as AsRef<[Constraint]>>::as_ref(
      &constraints,
    ))
    .direction(Direction::Vertical)
    .split(size)
}

pub fn vertical_chunks_with_margin(
  constraints: Vec<Constraint>,
  size: Rect,
  margin: u16,
) -> Rc<[Rect]> {
  Layout::default()
    .constraints(<Vec<Constraint> as AsRef<[Constraint]>>::as_ref(
      &constraints,
    ))
    .direction(Direction::Vertical)
    .margin(margin)
    .split(size)
}

pub fn layout_block(title: Span<'_>) -> Block<'_> {
  Block::default().borders(Borders::ALL).title(title)
}

pub fn layout_block_default(title: &str) -> Block<'_> {
  layout_block(title_style(title))
}

pub fn layout_block_active(title: &str, light: bool) -> Block<'_> {
  layout_block(title_style(title)).style(style_secondary(light))
}

pub fn layout_block_active_span(title: Line<'_>, light: bool) -> Block<'_> {
  Block::default()
    .borders(Borders::ALL)
    .title(title)
    .style(style_secondary(light))
}

pub fn layout_block_top_border(title: Line<'_>) -> Block<'_> {
  Block::default().borders(Borders::TOP).title(title)
}

pub fn title_with_dual_style<'a>(part_1: String, part_2: String, light: bool) -> Line<'a> {
  Line::from(vec![
    Span::styled(part_1, style_secondary(light).add_modifier(Modifier::BOLD)),
    Span::styled(part_2, style_default(light).add_modifier(Modifier::BOLD)),
  ])
}

/// helper function to create a centered rect using up
/// certain percentage of the available rect `r`
pub fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
  let Rect {
    width: grid_width,
    height: grid_height,
    ..
  } = r;
  let outer_height = (grid_height / 2).saturating_sub(height / 2);

  let popup_layout = Layout::default()
    .direction(Direction::Vertical)
    .constraints(
      [
        Constraint::Length(outer_height),
        Constraint::Length(height),
        Constraint::Length(outer_height),
      ]
      .as_ref(),
    )
    .split(r);

  let outer_width = (grid_width / 2).saturating_sub(width / 2);

  Layout::default()
    .direction(Direction::Horizontal)
    .constraints(
      [
        Constraint::Length(outer_width),
        Constraint::Length(width),
        Constraint::Length(outer_width),
      ]
      .as_ref(),
    )
    .split(popup_layout[1])[1]
}

pub fn loading<B: Backend>(
  f: &mut Frame<'_, B>,
  block: Block<'_>,
  area: Rect,
  is_loading: bool,
  light: bool,
) {
  if is_loading {
    let text = "\n\n Loading ...\n\n".to_owned();
    let mut text = Text::from(text);
    text.patch_style(style_secondary(light));

    // Contains the text
    let paragraph = Paragraph::new(text)
      .style(style_secondary(light))
      .block(block);
    f.render_widget(paragraph, area);
  } else {
    f.render_widget(block, area)
  }
}

// using a macro to reuse code as generics will make handling lifetimes a PITA
#[macro_export]
macro_rules! draw_resource_tab {
  ($title:expr, $block:expr, $f:expr, $app:expr, $area:expr, $fn1:expr, $fn2:expr, $res:expr) => {
    match $block {
      ActiveBlock::Describe | ActiveBlock::Yaml => draw_describe_block(
        $f,
        $app,
        $area,
        title_with_dual_style(
          get_resource_title($app, $title, get_describe_active($block), $res.items.len()),
          format!("{} | {} <esc> ", COPY_HINT, $title),
          $app.light_theme,
        ),
      ),
      ActiveBlock::Namespaces => $fn1($app.get_prev_route().active_block, $f, $app, $area),
      _ => $fn2($f, $app, $area),
    };
  };
}

pub struct ResourceTableProps<'a, T> {
  pub title: String,
  pub inline_help: String,
  pub resource: &'a mut StatefulTable<T>,
  pub table_headers: Vec<&'a str>,
  pub column_widths: Vec<Constraint>,
}
/// common for all resources
pub fn draw_describe_block<B: Backend>(
  f: &mut Frame<'_, B>,
  app: &mut App,
  area: Rect,
  title: Line<'_>,
) {
  let block = layout_block_top_border(title);

  let txt = &app.data.describe_out.get_txt();
  if !txt.is_empty() {
    let mut txt = Text::from(txt.clone());
    txt.patch_style(style_primary(app.light_theme));

    let paragraph = Paragraph::new(txt)
      .block(block)
      .wrap(Wrap { trim: false })
      .scroll((app.data.describe_out.offset, 0));
    f.render_widget(paragraph, area);
  } else {
    loading(f, block, area, app.is_loading, app.light_theme);
  }
}

/// Draw a kubernetes resource overview tab
pub fn draw_resource_block<'a, B, T: KubeResource<U>, F, U: Serialize>(
  f: &mut Frame<'_, B>,
  area: Rect,
  table_props: ResourceTableProps<'a, T>,
  row_cell_mapper: F,
  light_theme: bool,
  is_loading: bool,
  filter: Option<String>,
) where
  B: Backend,
  F: Fn(&T) -> Row<'a>,
{
  let title = title_with_dual_style(table_props.title, table_props.inline_help, light_theme);
  let block = layout_block_top_border(title);

  if !table_props.resource.items.is_empty() {
    let rows = table_props.resource.items.iter().filter_map(|c| {
      // return only rows that match filter if filter is set
      if filter.is_some() && !filter.as_ref().unwrap().is_empty() {
        if c
          .get_name()
          .to_lowercase()
          .contains(&filter.as_ref().unwrap().to_lowercase())
        {
          Some(row_cell_mapper(c))
        } else {
          None
        }
      } else {
        Some(row_cell_mapper(c))
      }
    });

    let table = Table::new(rows)
      .header(table_header_style(table_props.table_headers, light_theme))
      .block(block)
      .highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT)
      .widths(&table_props.column_widths);

    f.render_stateful_widget(table, area, &mut table_props.resource.state);
  } else {
    loading(f, block, area, is_loading, light_theme);
  }
}

pub fn get_cluster_wide_resource_title<S: AsRef<str>>(
  title: S,
  items_len: usize,
  suffix: S,
) -> String {
  format!(" {} [{}] {}", title.as_ref(), items_len, suffix.as_ref())
}

pub fn get_resource_title<S: AsRef<str>>(
  app: &App,
  title: S,
  suffix: S,
  items_len: usize,
) -> String {
  format!(
    " {} {}",
    title_with_ns(
      title.as_ref(),
      app
        .data
        .selected
        .ns
        .as_ref()
        .unwrap_or(&String::from("all")),
      items_len
    ),
    suffix.as_ref(),
  )
}

static DESCRIBE_ACTIVE: &str = "-> Describe ";
static YAML_ACTIVE: &str = "-> YAML ";

pub fn get_describe_active<'a>(block: ActiveBlock) -> &'a str {
  match block {
    ActiveBlock::Describe => DESCRIBE_ACTIVE,
    _ => YAML_ACTIVE,
  }
}

pub fn title_with_ns(title: &str, ns: &str, length: usize) -> String {
  format!("{} (ns: {}) [{}]", title, ns, length)
}

#[cfg(test)]
mod tests {
  use ratatui::{backend::TestBackend, buffer::Buffer, style::Modifier, widgets::Cell, Terminal};

  use super::*;
  use crate::ui::utils::{COLOR_CYAN, COLOR_WHITE, COLOR_YELLOW};

  #[test]
  fn test_draw_resource_block() {
    let backend = TestBackend::new(100, 6);
    let mut terminal = Terminal::new(backend).unwrap();

    struct RenderTest {
      pub name: String,
      pub namespace: String,
      pub data: i32,
      pub age: String,
    }

    impl KubeResource<Option<String>> for RenderTest {
      fn get_name(&self) -> &String {
        &self.name
      }
      fn get_k8s_obj(&self) -> &Option<String> {
        &None
      }
    }
    terminal
      .draw(|f| {
        let size = f.size();
        let mut resource: StatefulTable<RenderTest> = StatefulTable::new();
        resource.set_items(vec![
          RenderTest {
            name: "Test 1".into(),
            namespace: "Test ns".into(),
            age: "65h3m".into(),
            data: 5,
          },
          RenderTest {
            name: "Test long name that should be truncated from view".into(),
            namespace: "Test ns".into(),
            age: "65h3m".into(),
            data: 3,
          },
          RenderTest {
            name: "test_long_name_that_should_be_truncated_from_view".into(),
            namespace: "Test ns long value check that should be truncated".into(),
            age: "65h3m".into(),
            data: 6,
          },
        ]);
        draw_resource_block(
          f,
          size,
          ResourceTableProps {
            title: "Test".into(),
            inline_help: "-> yaml <y>".into(),
            resource: &mut resource,
            table_headers: vec!["Namespace", "Name", "Data", "Age"],
            column_widths: vec![
              Constraint::Percentage(30),
              Constraint::Percentage(40),
              Constraint::Percentage(15),
              Constraint::Percentage(15),
            ],
          },
          |c| {
            Row::new(vec![
              Cell::from(c.namespace.to_owned()),
              Cell::from(c.name.to_owned()),
              Cell::from(c.data.to_string()),
              Cell::from(c.age.to_owned()),
            ])
            .style(style_primary(false))
          },
          false,
          false,
          None,
        );
      })
      .unwrap();

    let mut expected = Buffer::with_lines(vec![
        "Test-> yaml <y>─────────────────────────────────────────────────────────────────────────────────────",
        "   Namespace                      Name                                     Data            Age      ",
        "=> Test ns                        Test 1                                   5               65h3m    ",
        "   Test ns                        Test long name that should be truncated  3               65h3m    ",
        "   Test ns long value check that  test_long_name_that_should_be_truncated_ 6               65h3m    ",
        "                                                                                                    ",
      ]);
    // set row styles
    // First row heading style
    for col in 0..=99 {
      match col {
        0..=3 => {
          expected.get_mut(col, 0).set_style(
            Style::default()
              .fg(COLOR_YELLOW)
              .add_modifier(Modifier::BOLD),
          );
        }
        4..=14 => {
          expected.get_mut(col, 0).set_style(
            Style::default()
              .fg(COLOR_WHITE)
              .add_modifier(Modifier::BOLD),
          );
        }
        _ => {}
      }
    }

    // Second row table header style
    for col in 0..=99 {
      expected
        .get_mut(col, 1)
        .set_style(Style::default().fg(COLOR_WHITE));
    }
    // first table data row style
    for col in 0..=99 {
      expected.get_mut(col, 2).set_style(
        Style::default()
          .fg(COLOR_CYAN)
          .add_modifier(Modifier::REVERSED),
      );
    }
    // remaining table data row style
    for row in 3..=4 {
      for col in 0..=99 {
        expected
          .get_mut(col, row)
          .set_style(Style::default().fg(COLOR_CYAN));
      }
    }

    terminal.backend().assert_buffer(&expected);
  }

  #[test]
  fn test_draw_resource_block_filter() {
    let backend = TestBackend::new(100, 6);
    let mut terminal = Terminal::new(backend).unwrap();

    struct RenderTest {
      pub name: String,
      pub namespace: String,
      pub data: i32,
      pub age: String,
    }
    impl KubeResource<Option<String>> for RenderTest {
      fn get_name(&self) -> &String {
        &self.name
      }
      fn get_k8s_obj(&self) -> &Option<String> {
        &None
      }
    }

    terminal
      .draw(|f| {
        let size = f.size();
        let mut resource: StatefulTable<RenderTest> = StatefulTable::new();
        resource.set_items(vec![
          RenderTest {
            name: "Test 1".into(),
            namespace: "Test ns".into(),
            age: "65h3m".into(),
            data: 5,
          },
          RenderTest {
            name: "Test long name that should be truncated from view".into(),
            namespace: "Test ns".into(),
            age: "65h3m".into(),
            data: 3,
          },
          RenderTest {
            name: "test_long_name_that_should_be_truncated_from_view".into(),
            namespace: "Test ns long value check that should be truncated".into(),
            age: "65h3m".into(),
            data: 6,
          },
        ]);
        draw_resource_block(
          f,
          size,
          ResourceTableProps {
            title: "Test".into(),
            inline_help: "-> yaml <y>".into(),
            resource: &mut resource,
            table_headers: vec!["Namespace", "Name", "Data", "Age"],
            column_widths: vec![
              Constraint::Percentage(30),
              Constraint::Percentage(40),
              Constraint::Percentage(15),
              Constraint::Percentage(15),
            ],
          },
          |c| {
            Row::new(vec![
              Cell::from(c.namespace.to_owned()),
              Cell::from(c.name.to_owned()),
              Cell::from(c.data.to_string()),
              Cell::from(c.age.to_owned()),
            ])
            .style(style_primary(false))
          },
          false,
          false,
          Some("truncated".to_string()),
        );
      })
      .unwrap();

    let mut expected = Buffer::with_lines(vec![
        "Test-> yaml <y>─────────────────────────────────────────────────────────────────────────────────────",
        "   Namespace                      Name                                     Data            Age      ",
        "=> Test ns                        Test long name that should be truncated  3               65h3m    ",
        "   Test ns long value check that  test_long_name_that_should_be_truncated_ 6               65h3m    ",
        "                                                                                                    ",
        "                                                                                                    ",
      ]);
    // set row styles
    // First row heading style
    for col in 0..=99 {
      match col {
        0..=3 => {
          expected.get_mut(col, 0).set_style(
            Style::default()
              .fg(COLOR_YELLOW)
              .add_modifier(Modifier::BOLD),
          );
        }
        4..=14 => {
          expected.get_mut(col, 0).set_style(
            Style::default()
              .fg(COLOR_WHITE)
              .add_modifier(Modifier::BOLD),
          );
        }
        _ => {}
      }
    }

    // Second row table header style
    for col in 0..=99 {
      expected
        .get_mut(col, 1)
        .set_style(Style::default().fg(COLOR_WHITE));
    }
    // first table data row style
    for col in 0..=99 {
      expected.get_mut(col, 2).set_style(
        Style::default()
          .fg(COLOR_CYAN)
          .add_modifier(Modifier::REVERSED),
      );
    }
    // remaining table data row style
    for row in 3..=3 {
      for col in 0..=99 {
        expected
          .get_mut(col, row)
          .set_style(Style::default().fg(COLOR_CYAN));
      }
    }

    terminal.backend().assert_buffer(&expected);
  }

  #[test]
  fn test_get_resource_title() {
    let app = App::default();
    assert_eq!(
      get_resource_title(&app, "Title", "-> hello", 5),
      " Title (ns: all) [5] -> hello"
    );
  }

  #[test]
  fn test_title_with_ns() {
    assert_eq!(title_with_ns("Title", "hello", 3), "Title (ns: hello) [3]");
  }

  #[test]
  fn test_get_cluster_wide_resource_title() {
    assert_eq!(
      get_cluster_wide_resource_title("Cluster Resource", 3, ""),
      " Cluster Resource [3] "
    );
    assert_eq!(
      get_cluster_wide_resource_title("Nodes", 10, "-> hello"),
      " Nodes [10] -> hello"
    );
  }
}
