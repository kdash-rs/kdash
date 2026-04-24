use std::{borrow::Cow, collections::BTreeMap, io::Cursor, rc::Rc, sync::OnceLock};

use glob_match::glob_match;
use ratatui::{
  layout::{Constraint, Direction, Layout, Position, Rect},
  style::{Color, Modifier, Style},
  symbols,
  text::{Line, Span, Text},
  widgets::{Block, Borders, Paragraph, Row, Table, Wrap},
  Frame,
};

use super::HIGHLIGHT;
use crate::app::{
  key_binding::DEFAULT_KEYBINDING,
  models::{Named, StatefulTable},
  ActiveBlock, App,
};
use crate::event::Key;
use crate::ui::theme::override_color;
// Viewport width thresholds for responsive column display
pub const COMPACT_WIDTH_THRESHOLD: u16 = 120;
pub const WIDE_WIDTH_THRESHOLD: u16 = 180;

/// Which responsive tier the current view is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ViewTier {
  Compact,
  Standard,
  Wide,
}

impl ViewTier {
  pub fn from_width(area_width: u16, force_wide: bool) -> Self {
    if force_wide || area_width >= WIDE_WIDTH_THRESHOLD {
      Self::Wide
    } else if area_width >= COMPACT_WIDTH_THRESHOLD {
      Self::Standard
    } else {
      Self::Compact
    }
  }
}

/// Declarative column definition with per-tier width percentages.
/// A `None` width means the column is hidden at that tier.
pub struct ColumnDef {
  pub label: &'static str,
  pub compact: Option<u16>,
  pub standard: Option<u16>,
  pub wide: Option<u16>,
}

impl ColumnDef {
  /// Column visible at all tiers with different widths.
  pub const fn all(label: &'static str, compact: u16, standard: u16, wide: u16) -> Self {
    Self {
      label,
      compact: Some(compact),
      standard: Some(standard),
      wide: Some(wide),
    }
  }

  /// Column visible only at Standard and Wide tiers.
  pub const fn standard(label: &'static str, standard: u16, wide: u16) -> Self {
    Self {
      label,
      compact: None,
      standard: Some(standard),
      wide: Some(wide),
    }
  }

  /// Column visible only at Wide tier.
  pub const fn wide(label: &'static str, wide: u16) -> Self {
    Self {
      label,
      compact: None,
      standard: None,
      wide: Some(wide),
    }
  }
}

/// Given column definitions and a view tier, return the visible headers and widths.
pub fn responsive_columns(columns: &[ColumnDef], tier: ViewTier) -> (Vec<&str>, Vec<Constraint>) {
  columns
    .iter()
    .filter_map(|col| {
      let w = match tier {
        ViewTier::Wide => col.wide,
        ViewTier::Standard => col.standard,
        ViewTier::Compact => col.compact,
      };
      w.map(|w| (col.label, Constraint::Percentage(w)))
    })
    .unzip()
}

// Utils

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinePart<'a> {
  Default(Cow<'a, str>),
  Help(Cow<'a, str>),
}

// Catppuccin Macchiato (dark)
pub const MACCHIATO_BASE: Color = Color::Rgb(36, 39, 58);
pub const MACCHIATO_BLUE: Color = Color::Rgb(138, 173, 244);
pub const MACCHIATO_GREEN: Color = Color::Rgb(166, 218, 149);
pub const MACCHIATO_RED: Color = Color::Rgb(237, 135, 150);
pub const MACCHIATO_YELLOW: Color = Color::Rgb(238, 212, 159);
pub const MACCHIATO_PEACH: Color = Color::Rgb(245, 169, 127);
pub const MACCHIATO_TEXT: Color = Color::Rgb(202, 211, 245);
pub const MACCHIATO_MAUVE: Color = Color::Rgb(198, 160, 246);
// Catppuccin Latte (light)
pub const LATTE_MAUVE: Color = Color::Rgb(136, 57, 239);
pub const LATTE_TEXT: Color = Color::Rgb(76, 79, 105);
pub const LATTE_BLUE: Color = Color::Rgb(30, 102, 245);
pub const LATTE_MAROON: Color = Color::Rgb(230, 69, 83);
pub const LATTE_GREEN: Color = Color::Rgb(64, 160, 43);
pub const LATTE_RED: Color = Color::Rgb(210, 15, 57);
pub const LATTE_PEACH: Color = Color::Rgb(254, 100, 11);
pub const LATTE_BASE: Color = Color::Rgb(239, 241, 245);
const CATPPUCCIN_MACCHIATO_THEME: &[u8] =
  include_bytes!("../../assets/themes/CatppuccinMacchiato.tmTheme");
const CATPPUCCIN_LATTE_THEME: &[u8] = include_bytes!("../../assets/themes/CatppuccinLatte.tmTheme");

/// Convert a syntect highlight segment into an owned ratatui Span.
fn syntect_to_ratatui_span_owned(
  (style, content): (syntect::highlighting::Style, &str),
) -> Option<Span<'static>> {
  use syntect::highlighting::FontStyle;
  let fg = if style.foreground.a > 0 {
    Some(Color::Rgb(
      style.foreground.r,
      style.foreground.g,
      style.foreground.b,
    ))
  } else {
    None
  };
  let bg = if style.background.a > 0 {
    Some(Color::Rgb(
      style.background.r,
      style.background.g,
      style.background.b,
    ))
  } else {
    None
  };
  let modifier = {
    let fs = style.font_style;
    let mut m = Modifier::empty();
    if fs.contains(FontStyle::BOLD) {
      m |= Modifier::BOLD;
    }
    if fs.contains(FontStyle::ITALIC) {
      m |= Modifier::ITALIC;
    }
    if fs.contains(FontStyle::UNDERLINE) {
      m |= Modifier::UNDERLINED;
    }
    m
  };
  let ratatui_style = Style::default()
    .fg(fg.unwrap_or_default())
    .bg(bg.unwrap_or_default())
    .add_modifier(modifier);
  Some(Span::styled(content.to_owned(), ratatui_style))
}

fn get_syntax_set() -> &'static syntect::parsing::SyntaxSet {
  static SYNTAX_SET: OnceLock<syntect::parsing::SyntaxSet> = OnceLock::new();
  SYNTAX_SET.get_or_init(syntect::parsing::SyntaxSet::load_defaults_newlines)
}

fn get_yaml_syntax_reference() -> &'static syntect::parsing::SyntaxReference {
  static YAML_SYNTAX_REFERENCE: OnceLock<syntect::parsing::SyntaxReference> = OnceLock::new();
  YAML_SYNTAX_REFERENCE.get_or_init(|| {
    get_syntax_set()
      .find_syntax_by_extension("yaml")
      .unwrap()
      .clone()
  })
}

struct YamlThemes {
  dark: syntect::highlighting::Theme,
  light: syntect::highlighting::Theme,
}

fn get_yaml_themes() -> &'static YamlThemes {
  static YAML_THEMES: OnceLock<YamlThemes> = OnceLock::new();
  YAML_THEMES.get_or_init(|| {
    let dark = load_embedded_theme(CATPPUCCIN_MACCHIATO_THEME);
    let light = load_embedded_theme(CATPPUCCIN_LATTE_THEME);
    YamlThemes { dark, light }
  })
}

fn load_embedded_theme(theme_bytes: &[u8]) -> syntect::highlighting::Theme {
  syntect::highlighting::ThemeSet::load_from_reader(&mut Cursor::new(theme_bytes))
    .expect("embedded theme should load")
}

#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Styles {
  Text,
  Failure,
  Warning,
  Success,
  Primary,
  Secondary,
  Help,
  Background,
}

pub fn theme_styles(light: bool) -> BTreeMap<Styles, Style> {
  let mut styles = if light {
    BTreeMap::from([
      (Styles::Text, Style::default().fg(LATTE_TEXT)),
      (Styles::Failure, Style::default().fg(LATTE_RED)),
      (Styles::Warning, Style::default().fg(LATTE_PEACH)),
      (Styles::Success, Style::default().fg(LATTE_GREEN)),
      (Styles::Primary, Style::default().fg(LATTE_MAUVE)),
      (Styles::Secondary, Style::default().fg(LATTE_MAROON)),
      (Styles::Help, Style::default().fg(LATTE_BLUE)),
      (
        Styles::Background,
        Style::default().bg(LATTE_BASE).fg(LATTE_TEXT),
      ),
    ])
  } else {
    BTreeMap::from([
      (Styles::Text, Style::default().fg(MACCHIATO_TEXT)),
      (Styles::Failure, Style::default().fg(MACCHIATO_RED)),
      (Styles::Warning, Style::default().fg(MACCHIATO_PEACH)),
      (Styles::Success, Style::default().fg(MACCHIATO_GREEN)),
      (Styles::Primary, Style::default().fg(MACCHIATO_MAUVE)),
      (Styles::Secondary, Style::default().fg(MACCHIATO_YELLOW)),
      (Styles::Help, Style::default().fg(MACCHIATO_BLUE)),
      (
        Styles::Background,
        Style::default().bg(MACCHIATO_BASE).fg(MACCHIATO_TEXT),
      ),
    ])
  };

  apply_theme_override(&mut styles, Styles::Text, "text", false, light);
  apply_theme_override(&mut styles, Styles::Failure, "failure", false, light);
  apply_theme_override(&mut styles, Styles::Warning, "warning", false, light);
  apply_theme_override(&mut styles, Styles::Success, "success", false, light);
  apply_theme_override(&mut styles, Styles::Primary, "primary", false, light);
  apply_theme_override(&mut styles, Styles::Secondary, "secondary", false, light);
  apply_theme_override(&mut styles, Styles::Help, "help", false, light);
  apply_theme_override(&mut styles, Styles::Background, "background", true, light);

  styles
}

pub fn title_style(txt: &str) -> Span<'_> {
  Span::styled(txt, style_bold())
}

pub fn default_part<'a, S: Into<Cow<'a, str>>>(text: S) -> LinePart<'a> {
  LinePart::Default(text.into())
}

pub fn help_part<'a, S: Into<Cow<'a, str>>>(text: S) -> LinePart<'a> {
  LinePart::Help(text.into())
}

pub fn style_header(light: bool) -> Style {
  style_primary(light).add_modifier(Modifier::REVERSED)
}

pub fn style_bold() -> Style {
  Style::default().add_modifier(Modifier::BOLD)
}

pub fn style_text(light: bool) -> Style {
  *theme_styles(light).get(&Styles::Text).unwrap()
}
pub fn style_logo(light: bool) -> Style {
  style_primary(light)
}
pub fn style_failure(light: bool) -> Style {
  *theme_styles(light).get(&Styles::Failure).unwrap()
}
pub fn style_warning(light: bool) -> Style {
  *theme_styles(light).get(&Styles::Warning).unwrap()
}
pub fn style_caution(light: bool) -> Style {
  style_warning(light)
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

fn line_part_style(part: &LinePart<'_>, light: bool, bold: bool) -> Style {
  let style = match part {
    LinePart::Default(_) => style_text(light),
    LinePart::Help(_) => style_help(light),
  };
  if bold {
    style.add_modifier(Modifier::BOLD)
  } else {
    style
  }
}

fn apply_theme_override(
  styles: &mut BTreeMap<Styles, Style>,
  slot: Styles,
  config_key: &str,
  background: bool,
  light: bool,
) {
  if let Some(color) = override_color(config_key, light) {
    let style = styles.entry(slot).or_default();
    *style = if background {
      style.bg(color)
    } else {
      style.fg(color)
    };
  }
}

pub fn mixed_line<'a, I>(parts: I, light: bool) -> Line<'a>
where
  I: IntoIterator<Item = LinePart<'a>>,
{
  styled_line(parts, light, false)
}

pub fn mixed_bold_line<'a, I>(parts: I, light: bool) -> Line<'a>
where
  I: IntoIterator<Item = LinePart<'a>>,
{
  styled_line(parts, light, true)
}

pub fn help_bold_line<'a, S: Into<Cow<'a, str>>>(text: S, light: bool) -> Line<'a> {
  mixed_bold_line([help_part(text)], light)
}

pub fn key_hints(keys: &[Key]) -> String {
  keys
    .iter()
    .map(ToString::to_string)
    .collect::<Vec<_>>()
    .join("/")
}

pub fn action_hint(action: &str, key: Key) -> String {
  format!("{} {}", action, key)
}

pub fn describe_and_yaml_hint() -> String {
  format!(
    "{} | {} ",
    action_hint("describe", DEFAULT_KEYBINDING.describe_resource.key),
    action_hint("yaml", DEFAULT_KEYBINDING.resource_yaml.key)
  )
}

pub fn describe_yaml_and_logs_hint() -> String {
  format!(
    "{} | {} ",
    describe_and_yaml_hint().trim_end(),
    action_hint("logs", DEFAULT_KEYBINDING.aggregate_logs.key)
  )
}

pub fn describe_yaml_logs_and_esc_hint() -> String {
  format!(
    "{} | back {} ",
    describe_yaml_and_logs_hint().trim_end(),
    DEFAULT_KEYBINDING.esc.key
  )
}

pub fn describe_yaml_and_esc_hint() -> String {
  format!(
    "{} | back {} ",
    describe_and_yaml_hint().trim_end(),
    DEFAULT_KEYBINDING.esc.key
  )
}

pub fn describe_yaml_decode_and_esc_hint() -> String {
  format!(
    "{} | {} | back {} ",
    describe_and_yaml_hint().trim_end(),
    action_hint("decode", DEFAULT_KEYBINDING.decode_secret.key),
    DEFAULT_KEYBINDING.esc.key
  )
}

pub fn wide_hint() -> String {
  format!("wide {}", DEFAULT_KEYBINDING.toggle_wide_columns.key)
}

pub fn filter_cursor_position(area: Rect, prefix_width: usize, filter: &str) -> Position {
  Position {
    x: area.x
      + (prefix_width as u16 + 1 + filter.chars().count() as u16).min(area.width.saturating_sub(2)),
    y: area.y,
  }
}

fn styled_line<'a, I>(parts: I, light: bool, bold: bool) -> Line<'a>
where
  I: IntoIterator<Item = LinePart<'a>>,
{
  Line::from(
    parts
      .into_iter()
      .map(|part| {
        let style = line_part_style(&part, light, bold);
        match part {
          LinePart::Default(text) | LinePart::Help(text) => Span::styled(text, style),
        }
      })
      .collect::<Vec<_>>(),
  )
}

pub fn get_gauge_symbol(enhanced_graphics: bool) -> &'static str {
  if enhanced_graphics {
    symbols::line::THICK_HORIZONTAL
  } else {
    symbols::line::HORIZONTAL
  }
}

pub fn table_header_style(cells: Vec<&str>, light: bool) -> Row<'_> {
  Row::new(cells).style(style_text(light)).bottom_margin(0)
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

pub fn layout_block_default_line(title: Line<'_>) -> Block<'_> {
  Block::default().borders(Borders::ALL).title(title)
}

pub fn layout_block_active_line(title: Line<'_>, light: bool) -> Block<'_> {
  Block::default()
    .borders(Borders::ALL)
    .title(title)
    .style(style_secondary(light))
}

pub fn layout_block_active_span(title: Line<'_>, light: bool) -> Block<'_> {
  layout_block_active_line(title, light)
}

pub fn layout_block_top_border(title: Line<'_>) -> Block<'_> {
  Block::default().borders(Borders::TOP).title(title)
}

enum FilterDisplayState<'a> {
  Inactive,
  EditingEmpty,
  Value { filter: &'a str, active: bool },
}

fn filter_display_state(filter: &str, active: bool) -> FilterDisplayState<'_> {
  if active && filter.is_empty() {
    FilterDisplayState::EditingEmpty
  } else if !filter.is_empty() {
    FilterDisplayState::Value { filter, active }
  } else {
    FilterDisplayState::Inactive
  }
}

fn filter_display_parts(filter: &str, active: bool) -> Vec<LinePart<'_>> {
  let state = filter_display_state(filter, active);
  let inactive_text = action_hint("filter", DEFAULT_KEYBINDING.filter.key);
  let clear_suffix = format!(" | clear {} ", DEFAULT_KEYBINDING.esc.key);
  let edit_suffix = format!(" | edit {} ", DEFAULT_KEYBINDING.filter.key);

  match state {
    FilterDisplayState::Inactive => vec![help_part(inactive_text)],
    FilterDisplayState::EditingEmpty => {
      vec![help_part("[type to filter]"), help_part(clear_suffix)]
    }
    FilterDisplayState::Value {
      filter,
      active: true,
    } => vec![
      default_part(format!("[{}]", filter)),
      help_part(clear_suffix),
    ],
    FilterDisplayState::Value {
      filter,
      active: false,
    } => vec![
      default_part(format!("[{}]", filter)),
      help_part(edit_suffix),
    ],
  }
}

pub fn filter_status_parts(filter: &str, active: bool) -> Vec<LinePart<'_>> {
  filter_display_parts(filter, active)
}

pub fn owned_filter_status_parts(filter: &str, active: bool) -> Vec<LinePart<'static>> {
  filter_display_parts(filter, active)
    .into_iter()
    .map(|part| match part {
      LinePart::Default(text) => default_part(text.into_owned()),
      LinePart::Help(text) => help_part(text.into_owned()),
    })
    .collect()
}

pub fn title_with_dual_style<'a>(part_1: String, part_2: Line<'a>, light: bool) -> Line<'a> {
  let mut spans = vec![Span::styled(
    part_1,
    style_secondary(light).add_modifier(Modifier::BOLD),
  )];
  spans.extend(part_2.spans);
  Line::from(spans)
}

pub fn copy_and_escape_title_line<'a, S: Into<Cow<'a, str>>>(_target: S, light: bool) -> Line<'a> {
  mixed_bold_line(
    [
      help_part(format!(
        "{} | ",
        action_hint("copy", DEFAULT_KEYBINDING.copy_to_clipboard.key)
      )),
      help_part(format!("back {} ", DEFAULT_KEYBINDING.esc.key)),
    ],
    light,
  )
}

pub fn copy_scroll_and_escape_title_line<'a, S: Into<Cow<'a, str>>>(
  _target: S,
  auto_scroll: bool,
  light: bool,
) -> Line<'a> {
  let auto_scroll_action = if auto_scroll {
    "pause scroll"
  } else {
    "resume scroll"
  };
  mixed_bold_line(
    [
      help_part(format!(
        "{} | {} | ",
        action_hint("copy", DEFAULT_KEYBINDING.copy_to_clipboard.key),
        action_hint(auto_scroll_action, DEFAULT_KEYBINDING.log_auto_scroll.key)
      )),
      help_part(format!("back {} ", DEFAULT_KEYBINDING.esc.key)),
    ],
    light,
  )
}

pub fn split_hint_suffix(text: &str) -> (&str, Option<&str>) {
  if let Some(pos) = text.rfind(" <") {
    (&text[..pos], Some(&text[(pos + 1)..]))
  } else {
    (text, None)
  }
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

pub fn loading(f: &mut Frame<'_>, block: Block<'_>, area: Rect, is_loading: bool, light: bool) {
  if is_loading {
    let text = "\n\n Loading ...\n\n".to_owned();
    let text = Text::from(text);
    let text = text.patch_style(style_secondary(light));

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
      ActiveBlock::Describe => draw_describe_block(
        $f,
        $app,
        $area,
        title_with_dual_style(
          get_resource_title($app, $title, get_describe_active($block), $res.items.len()),
          $crate::ui::utils::copy_and_escape_title_line($title, $app.light_theme),
          $app.light_theme,
        ),
      ),
      ActiveBlock::Yaml => draw_yaml_block(
        $f,
        $app,
        $area,
        title_with_dual_style(
          get_resource_title($app, $title, get_describe_active($block), $res.items.len()),
          $crate::ui::utils::copy_and_escape_title_line($title, $app.light_theme),
          $app.light_theme,
        ),
      ),
      ActiveBlock::Pods => $crate::app::pods::draw_block_as_sub($f, $app, $area),
      ActiveBlock::Containers => $crate::app::pods::draw_containers_block($f, $app, $area),
      ActiveBlock::Logs => $crate::app::pods::draw_logs_block($f, $app, $area),
      ActiveBlock::Namespaces => $fn1($app.get_prev_route().active_block, $f, $app, $area),
      _ => $fn2($f, $app, $area),
    };
  };
}

pub struct ResourceTableProps<'a, T> {
  pub title: String,
  pub inline_help: Line<'a>,
  pub resource: &'a mut StatefulTable<T>,
  pub table_headers: Vec<&'a str>,
  pub column_widths: Vec<Constraint>,
}
/// common for all resources
pub fn draw_describe_block(f: &mut Frame<'_>, app: &mut App, area: Rect, title: Line<'_>) {
  draw_yaml_block(f, app, area, title);
}

/// Refreshes the syntax-highlight cache when empty or the theme changed.
/// Returns `false` when there is no content to highlight.
fn ensure_highlight_cache(app: &mut App) -> bool {
  if app.data.describe_out.get_txt().is_empty() {
    return false;
  }
  if app.data.describe_out.highlighted_lines.is_empty()
    || app.data.describe_out.highlight_light_theme != app.light_theme
  {
    let ss = get_syntax_set();
    let syntax = get_yaml_syntax_reference();
    let theme = if app.light_theme {
      &get_yaml_themes().light
    } else {
      &get_yaml_themes().dark
    };
    let mut h = syntect::easy::HighlightLines::new(syntax, theme);
    let txt = app.data.describe_out.get_txt();
    let lines: Vec<_> = syntect::util::LinesWithEndings::from(txt)
      .filter_map(|line| match h.highlight_line(line, ss) {
        Ok(segments) => {
          let line_spans: Vec<_> = segments
            .into_iter()
            .filter_map(syntect_to_ratatui_span_owned)
            .collect();
          Some(ratatui::text::Line::from(line_spans))
        }
        Err(_) => None,
      })
      .collect();
    app.data.describe_out.highlighted_lines = lines;
    app.data.describe_out.highlight_light_theme = app.light_theme;
  }
  true
}

/// common for all resources
pub fn draw_yaml_block(f: &mut Frame<'_>, app: &mut App, area: Rect, title: Line<'_>) {
  let block = layout_block_top_border(title);
  if ensure_highlight_cache(app) {
    let offset = app.data.describe_out.offset;
    let total = app.data.describe_out.highlighted_lines.len();
    // Subtract 2 for the top-border of the block.
    let view_h = area.height.saturating_sub(2) as usize;
    // Take a generous window around the visible region.
    let slice_start = offset.saturating_sub(view_h);
    let slice_end = total.min(offset + view_h * 3);
    let adjusted_offset = (offset - slice_start).min(u16::MAX as usize) as u16;
    let visible_lines = app.data.describe_out.highlighted_lines[slice_start..slice_end].to_vec();
    let paragraph = Paragraph::new(visible_lines)
      .block(block)
      .wrap(Wrap { trim: false })
      .scroll((adjusted_offset, 0));
    f.render_widget(paragraph, area);
  } else {
    loading(f, block, area, app.is_loading(), app.light_theme);
  }
}

fn draw_resource_table<'a, T: Named, F>(
  f: &mut Frame<'_>,
  area: Rect,
  table_props: ResourceTableProps<'a, T>,
  row_cell_mapper: F,
  light_theme: bool,
  is_loading: bool,
  block: Block<'a>,
) where
  F: Fn(&T) -> Row<'a>,
{
  if !table_props.resource.items.is_empty() {
    let filter = table_props.resource.filter.to_lowercase();
    let has_filter = !filter.is_empty();
    let mut filtered_indices: Vec<usize> = Vec::new();

    let filtered_items: Vec<(usize, &T)> = table_props
      .resource
      .items
      .iter()
      .enumerate()
      .filter(|(_, c)| filter.is_empty() || filter_by_name(&filter, *c))
      .inspect(|(idx, _)| {
        if has_filter {
          filtered_indices.push(*idx);
        }
      })
      .collect();

    if has_filter {
      let max = filtered_items.len().saturating_sub(1);
      if let Some(sel) = table_props.resource.state.selected() {
        if sel > max {
          table_props.resource.state.select(Some(max));
        }
      }
    }
    table_props.resource.filtered_indices = filtered_indices;

    // Determine the visible row range to avoid expensive row_cell_mapper
    // calls for off-screen items.  Subtract 3 for header + borders.
    let selected = table_props.resource.state.selected().unwrap_or(0);
    let view_h = area.height.saturating_sub(3) as usize;
    let visible_start = selected.saturating_sub(view_h);
    let visible_end = (selected + view_h * 2).min(filtered_items.len());

    let rows: Vec<Row<'a>> = filtered_items
      .iter()
      .enumerate()
      .map(|(fi, (_orig_idx, item))| {
        if fi >= visible_start && fi < visible_end {
          row_cell_mapper(item)
        } else {
          Row::default()
        }
      })
      .collect();

    let table = Table::new(rows, &table_props.column_widths)
      .header(table_header_style(table_props.table_headers, light_theme))
      .block(block)
      .row_highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT);

    f.render_stateful_widget(table, area, &mut table_props.resource.state);
  } else {
    loading(f, block, area, is_loading, light_theme);
  }
}

/// Builds the help `Line` for a resource block title, weaving filter status
/// into any existing inline help (placing it after a "containers" prefix when present).
fn build_resource_help_line(
  inline_help: Line<'_>,
  filter: &str,
  filter_active: bool,
  light_theme: bool,
) -> Line<'static> {
  let inline_help_text = inline_help
    .spans
    .iter()
    .map(|span| span.content.as_ref())
    .collect::<String>();
  let containers_prefix = format!(
    "{} | ",
    action_hint("containers", DEFAULT_KEYBINDING.submit.key)
  );
  let mut help_parts: Vec<LinePart<'static>> = Vec::new();
  if let Some(rest) = inline_help_text.strip_prefix(&containers_prefix) {
    help_parts.push(help_part(containers_prefix));
    help_parts.extend(owned_filter_status_parts(filter, filter_active));
    if !rest.is_empty() {
      help_parts.push(help_part(" | ".to_string()));
      help_parts.push(help_part(rest.to_string()));
    }
  } else {
    help_parts.extend(owned_filter_status_parts(filter, filter_active));
    if !inline_help_text.is_empty() {
      help_parts.push(help_part(" | ".to_string()));
      help_parts.push(help_part(inline_help_text));
    }
  }
  mixed_bold_line(help_parts, light_theme)
}

/// Draw a kubernetes resource overview tab
pub fn draw_resource_block<'a, T: Named, F>(
  f: &mut Frame<'_>,
  area: Rect,
  table_props: ResourceTableProps<'a, T>,
  row_cell_mapper: F,
  light_theme: bool,
  is_loading: bool,
) where
  F: Fn(&T) -> Row<'a>,
{
  let ResourceTableProps {
    title,
    inline_help,
    resource,
    table_headers,
    column_widths,
  } = table_props;
  let filter = resource.filter.clone();
  let filter_active = resource.filter_active;
  if filter_active {
    let title_width = title.chars().count();
    let title = title_with_dual_style(
      title,
      mixed_bold_line(owned_filter_status_parts(&filter, true), light_theme),
      light_theme,
    );
    let block = layout_block_top_border(title);
    draw_resource_table(
      f,
      area,
      ResourceTableProps {
        title: String::new(),
        inline_help: Line::default(),
        resource,
        table_headers,
        column_widths,
      },
      row_cell_mapper,
      light_theme,
      is_loading,
      block,
    );
    f.set_cursor_position(filter_cursor_position(area, title_width, &filter));
    return;
  }

  let help_line = build_resource_help_line(inline_help, &filter, filter_active, light_theme);
  let title = title_with_dual_style(title, help_line, light_theme);
  let block = layout_block_top_border(title);
  draw_resource_table(
    f,
    area,
    ResourceTableProps {
      title: String::new(),
      inline_help: Line::default(),
      resource,
      table_headers,
      column_widths,
    },
    row_cell_mapper,
    light_theme,
    is_loading,
    block,
  );
}

pub fn draw_route_resource_block<'a, T: Named, F>(
  f: &mut Frame<'_>,
  area: Rect,
  table_props: ResourceTableProps<'a, T>,
  row_cell_mapper: F,
  light_theme: bool,
  is_loading: bool,
) where
  F: Fn(&T) -> Row<'a>,
{
  let ResourceTableProps {
    title,
    inline_help,
    resource,
    table_headers,
    column_widths,
  } = table_props;
  let filter = resource.filter.clone();
  let filter_active = resource.filter_active;
  if filter_active {
    let title_width = title.chars().count();
    let title = title_with_dual_style(
      title,
      mixed_bold_line(owned_filter_status_parts(&filter, true), light_theme),
      light_theme,
    );
    let block = layout_block_active_span(title, light_theme);
    draw_resource_table(
      f,
      area,
      ResourceTableProps {
        title: String::new(),
        inline_help: Line::default(),
        resource,
        table_headers,
        column_widths,
      },
      row_cell_mapper,
      light_theme,
      is_loading,
      block,
    );
    f.set_cursor_position(filter_cursor_position(area, title_width, &filter));
    return;
  }

  let title = title_with_dual_style(title, inline_help, light_theme);
  let block = layout_block_active_span(title, light_theme);
  draw_resource_table(
    f,
    area,
    ResourceTableProps {
      title: String::new(),
      inline_help: Line::default(),
      resource,
      table_headers,
      column_widths,
    },
    row_cell_mapper,
    light_theme,
    is_loading,
    block,
  );
}

pub fn filter_by_resource_name<T: Named>(
  filter: &str,
  res: &T,
  row_cell_mapper: Row<'static>,
) -> Option<Row<'static>> {
  if filter.is_empty() || filter_by_name(filter, res) {
    Some(row_cell_mapper)
  } else {
    None
  }
}

pub fn text_matches_filter(filter: &str, value: &str) -> bool {
  let filter = filter.to_lowercase();
  let value = value.to_lowercase();
  filter.is_empty() || glob_match(&filter, &value) || value.contains(&filter)
}

fn filter_by_name<T: Named>(ft: &str, res: &T) -> bool {
  text_matches_filter(ft, res.get_name())
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
  use ratatui::{
    backend::TestBackend, buffer::Buffer, layout::Position, style::Modifier, widgets::Cell,
    Terminal,
  };

  use super::*;
  use crate::ui::utils::{MACCHIATO_BLUE, MACCHIATO_MAUVE, MACCHIATO_TEXT, MACCHIATO_YELLOW};

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

    impl Named for RenderTest {
      fn get_name(&self) -> &String {
        &self.name
      }
    }
    terminal
      .draw(|f| {
        let size = f.area();
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
            inline_help: help_bold_line("-> yaml <y>", false),
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
        );
      })
      .unwrap();

    let mut expected = Buffer::with_lines(vec![
        "Testfilter </> | -> yaml <y>────────────────────────────────────────────────────────────────────────",
        "   Namespace                     Name                                 Data           Age            ",
        "=> Test ns                       Test 1                               5              65h3m          ",
        "   Test ns                       Test long name that should be trunca 3              65h3m          ",
        "   Test ns long value check that test_long_name_that_should_be_trunca 6              65h3m          ",
        "                                                                                                    ",
      ]);
    // set row styles
    // First row heading style
    for col in 0..=99 {
      match col {
        0..=3 => {
          expected.cell_mut(Position::new(col, 0)).unwrap().set_style(
            Style::default()
              .fg(MACCHIATO_YELLOW)
              .add_modifier(Modifier::BOLD),
          );
        }
        4..=27 => {
          expected.cell_mut(Position::new(col, 0)).unwrap().set_style(
            Style::default()
              .fg(MACCHIATO_BLUE)
              .add_modifier(Modifier::BOLD),
          );
        }
        _ => {}
      }
    }

    // Second row table header style
    for col in 0..=99 {
      expected
        .cell_mut(Position::new(col, 1))
        .unwrap()
        .set_style(Style::default().fg(MACCHIATO_TEXT));
    }
    // first table data row style
    for col in 0..=99 {
      expected.cell_mut(Position::new(col, 2)).unwrap().set_style(
        Style::default()
          .fg(MACCHIATO_MAUVE)
          .add_modifier(Modifier::REVERSED),
      );
    }
    // remaining table data row style
    for row in 3..=4 {
      for col in 0..=99 {
        expected
          .cell_mut(Position::new(col, row))
          .unwrap()
          .set_style(Style::default().fg(MACCHIATO_MAUVE));
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
    impl Named for RenderTest {
      fn get_name(&self) -> &String {
        &self.name
      }
    }

    terminal
      .draw(|f| {
        let size = f.area();
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
        resource.filter = "truncated".to_string();
        draw_resource_block(
          f,
          size,
          ResourceTableProps {
            title: "Test".into(),
            inline_help: help_bold_line("-> yaml <y>", false),
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
        );
      })
      .unwrap();

    let mut expected = Buffer::with_lines(vec![
        "Test[truncated] | edit </>  | -> yaml <y>───────────────────────────────────────────────────────────",
        "   Namespace                     Name                                 Data           Age            ",
        "=> Test ns                       Test long name that should be trunca 3              65h3m          ",
        "   Test ns long value check that test_long_name_that_should_be_trunca 6              65h3m          ",
        "                                                                                                    ",
        "                                                                                                    ",
      ]);
    // set row styles
    // First row heading style
    for col in 0..=99 {
      match col {
        0..=3 => {
          expected.cell_mut(Position::new(col, 0)).unwrap().set_style(
            Style::default()
              .fg(MACCHIATO_YELLOW)
              .add_modifier(Modifier::BOLD),
          );
        }
        4..=14 => {
          expected.cell_mut(Position::new(col, 0)).unwrap().set_style(
            Style::default()
              .fg(MACCHIATO_TEXT)
              .add_modifier(Modifier::BOLD),
          );
        }
        15..=40 => {
          expected.cell_mut(Position::new(col, 0)).unwrap().set_style(
            Style::default()
              .fg(MACCHIATO_BLUE)
              .add_modifier(Modifier::BOLD),
          );
        }
        _ => {}
      }
    }

    // Second row table header style
    for col in 0..=99 {
      expected
        .cell_mut(Position::new(col, 1))
        .unwrap()
        .set_style(Style::default().fg(MACCHIATO_TEXT));
    }
    // first table data row style
    for col in 0..=99 {
      expected.cell_mut(Position::new(col, 2)).unwrap().set_style(
        Style::default()
          .fg(MACCHIATO_MAUVE)
          .add_modifier(Modifier::REVERSED),
      );
    }
    // remaining table data row style
    for row in 3..=3 {
      for col in 0..=99 {
        expected
          .cell_mut(Position::new(col, row))
          .unwrap()
          .set_style(Style::default().fg(MACCHIATO_MAUVE));
      }
    }

    terminal.backend().assert_buffer(&expected);
  }

  #[test]
  fn test_draw_resource_block_filter_glob() {
    let backend = TestBackend::new(100, 6);
    let mut terminal = Terminal::new(backend).unwrap();

    struct RenderTest {
      pub name: String,
      pub namespace: String,
      pub data: i32,
      pub age: String,
    }
    impl Named for RenderTest {
      fn get_name(&self) -> &String {
        &self.name
      }
    }

    terminal
      .draw(|f| {
        let size = f.area();
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
        resource.filter = "*long*truncated*".to_string();
        draw_resource_block(
          f,
          size,
          ResourceTableProps {
            title: "Test".into(),
            inline_help: help_bold_line("-> yaml <y>", false),
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
        );
      })
      .unwrap();

    let mut expected = Buffer::with_lines(vec![
        "Test[*long*truncated*] | edit </>  | -> yaml <y>────────────────────────────────────────────────────",
        "   Namespace                     Name                                 Data           Age            ",
        "=> Test ns                       Test long name that should be trunca 3              65h3m          ",
        "   Test ns long value check that test_long_name_that_should_be_trunca 6              65h3m          ",
        "                                                                                                    ",
        "                                                                                                    ",
      ]);
    // set row styles
    // First row heading style
    for col in 0..=99 {
      match col {
        0..=3 => {
          expected.cell_mut(Position::new(col, 0)).unwrap().set_style(
            Style::default()
              .fg(MACCHIATO_YELLOW)
              .add_modifier(Modifier::BOLD),
          );
        }
        4..=21 => {
          expected.cell_mut(Position::new(col, 0)).unwrap().set_style(
            Style::default()
              .fg(MACCHIATO_TEXT)
              .add_modifier(Modifier::BOLD),
          );
        }
        22..=47 => {
          expected.cell_mut(Position::new(col, 0)).unwrap().set_style(
            Style::default()
              .fg(MACCHIATO_BLUE)
              .add_modifier(Modifier::BOLD),
          );
        }
        _ => {}
      }
    }

    // Second row table header style
    for col in 0..=99 {
      expected
        .cell_mut(Position::new(col, 1))
        .unwrap()
        .set_style(Style::default().fg(MACCHIATO_TEXT));
    }
    // first table data row style
    for col in 0..=99 {
      expected.cell_mut(Position::new(col, 2)).unwrap().set_style(
        Style::default()
          .fg(MACCHIATO_MAUVE)
          .add_modifier(Modifier::REVERSED),
      );
    }
    // remaining table data row style
    for row in 3..=3 {
      for col in 0..=99 {
        expected
          .cell_mut(Position::new(col, row))
          .unwrap()
          .set_style(Style::default().fg(MACCHIATO_MAUVE));
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
  fn test_draw_resource_block_filter_hides_other_hints_when_active() {
    let backend = TestBackend::new(100, 4);
    let mut terminal = Terminal::new(backend).unwrap();

    struct RenderTest {
      pub name: String,
    }

    impl Named for RenderTest {
      fn get_name(&self) -> &String {
        &self.name
      }
    }

    terminal
      .draw(|f| {
        let size = f.area();
        let mut resource: StatefulTable<RenderTest> = StatefulTable::new();
        resource.set_items(vec![RenderTest {
          name: "test".into(),
        }]);
        resource.filter = "pod".into();
        resource.filter_active = true;
        draw_resource_block(
          f,
          size,
          ResourceTableProps {
            title: "Test".into(),
            inline_help: help_bold_line("describe <d> | back <Esc>", false),
            resource: &mut resource,
            table_headers: vec!["Name"],
            column_widths: vec![Constraint::Percentage(100)],
          },
          |c| Row::new(vec![Cell::from(c.name.to_owned())]).style(style_primary(false)),
          false,
          false,
        );
      })
      .unwrap();

    let first_line = (0..terminal.backend().buffer().area.width)
      .map(|col| terminal.backend().buffer()[(col, 0)].symbol())
      .collect::<String>();
    assert!(first_line.contains("[pod]"));
    assert!(first_line.contains("clear <Esc>"));
    assert!(!first_line.contains("describe <d>"));
    assert!(!first_line.contains("back <Esc>"));
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

  #[test]
  fn test_build_resource_help_line() {
    // Case 1: Empty inline_help, empty filter, filter_active=false
    // -> line text should contain the inactive "filter <key>" action hint
    let line = build_resource_help_line(Line::default(), "", false, false);
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    let expected_filter_hint = action_hint("filter", DEFAULT_KEYBINDING.filter.key);
    assert!(
      text.contains(&expected_filter_hint),
      "Case 1: expected '{text}' to contain '{expected_filter_hint}'"
    );

    // Case 2: Non-empty inline_help, empty filter, filter_active=false
    // -> line text should contain the inline help hint after " | "
    let line2 = build_resource_help_line(help_bold_line("-> yaml <y>", false), "", false, false);
    let text2: String = line2.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
      text2.contains("-> yaml <y>"),
      "Case 2: expected '{text2}' to contain '-> yaml <y>'"
    );

    // Case 3: inline_help starting with the containers prefix
    // -> line text should start with the containers hint
    let containers_prefix_str = format!(
      "{} | ",
      action_hint("containers", DEFAULT_KEYBINDING.submit.key)
    );
    let line3 = build_resource_help_line(
      help_bold_line(containers_prefix_str.as_str(), false),
      "",
      false,
      false,
    );
    let text3: String = line3.spans.iter().map(|s| s.content.as_ref()).collect();
    let containers_hint = action_hint("containers", DEFAULT_KEYBINDING.submit.key);
    assert!(
      text3.starts_with(&containers_hint),
      "Case 3: expected '{text3}' to start with '{containers_hint}'"
    );

    // Case 4: Empty inline_help, filter="foo", filter_active=false
    // -> line text should contain "[foo]"
    let line4 = build_resource_help_line(Line::default(), "foo", false, false);
    let text4: String = line4.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
      text4.contains("[foo]"),
      "Case 4: expected '{text4}' to contain '[foo]'"
    );
  }
}
