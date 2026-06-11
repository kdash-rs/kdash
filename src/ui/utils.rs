use std::{borrow::Cow, io::Cursor, rc::Rc, sync::OnceLock};

use glob_match::glob_match;
use ratatui::{
  layout::{Constraint, Direction, Layout, Position, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span, Text},
  widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Row, Table, Wrap},
  Frame,
};

use super::HIGHLIGHT;
use crate::app::{
  key_binding::DEFAULT_KEYBINDING,
  models::{Named, StatefulTable},
  ActiveBlock, App,
};
use crate::event::Key;
use crate::ui::theme::Palette;
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

/// Title span: panel titles render in the `secondary` slot, bold.
pub fn title_style(txt: &str, palette: Palette) -> Span<'_> {
  Span::styled(txt, style_secondary(palette).add_modifier(Modifier::BOLD))
}

pub fn default_part<'a, S: Into<Cow<'a, str>>>(text: S) -> LinePart<'a> {
  LinePart::Default(text.into())
}

pub fn help_part<'a, S: Into<Cow<'a, str>>>(text: S) -> LinePart<'a> {
  LinePart::Help(text.into())
}

pub fn style_text(palette: Palette) -> Style {
  Style::default().fg(palette.fg)
}
pub fn style_logo(palette: Palette) -> Style {
  style_primary(palette)
}
pub fn style_failure(palette: Palette) -> Style {
  Style::default().fg(palette.error)
}
pub fn style_warning(palette: Palette) -> Style {
  Style::default().fg(palette.warning)
}
pub fn style_caution(palette: Palette) -> Style {
  style_warning(palette)
}
pub fn style_success(palette: Palette) -> Style {
  Style::default().fg(palette.success)
}
/// Primary action / panel-border colour.
pub fn style_primary(palette: Palette) -> Style {
  Style::default().fg(palette.accent)
}
/// Help / hint / divider text.
pub fn style_help(palette: Palette) -> Style {
  Style::default().fg(palette.muted)
}
/// Panel titles.
pub fn style_secondary(palette: Palette) -> Style {
  Style::default().fg(palette.secondary)
}
/// Field labels and table column headers.
pub fn style_label(palette: Palette) -> Style {
  Style::default().fg(palette.label)
}

pub fn style_main_background(palette: Palette) -> Style {
  Style::default().bg(palette.bg).fg(palette.fg)
}

pub fn style_highlight() -> Style {
  Style::default().add_modifier(Modifier::REVERSED)
}

fn line_part_style(part: &LinePart<'_>, palette: Palette, bold: bool) -> Style {
  let style = match part {
    LinePart::Default(_) => style_text(palette),
    LinePart::Help(_) => style_help(palette),
  };
  if bold {
    style.add_modifier(Modifier::BOLD)
  } else {
    style
  }
}

pub fn mixed_line<'a, I>(parts: I, palette: Palette) -> Line<'a>
where
  I: IntoIterator<Item = LinePart<'a>>,
{
  styled_line(parts, palette, false)
}

pub fn mixed_bold_line<'a, I>(parts: I, palette: Palette) -> Line<'a>
where
  I: IntoIterator<Item = LinePart<'a>>,
{
  styled_line(parts, palette, true)
}

pub fn help_bold_line<'a, S: Into<Cow<'a, str>>>(text: S, palette: Palette) -> Line<'a> {
  mixed_bold_line([help_part(text)], palette)
}

pub fn key_hints(keys: &[Key]) -> String {
  keys.iter().map(Key::symbol).collect::<Vec<_>>().join("/")
}

/// A hint chip in LlamaStash's `key:label` form (e.g. `d:describe`).
pub fn action_hint(action: &str, key: Key) -> String {
  format!("{}:{}", key.symbol(), action)
}

pub fn describe_and_yaml_hint() -> String {
  format!(
    "{} · {} · {} ",
    action_hint("describe", DEFAULT_KEYBINDING.describe_resource.key),
    action_hint("yaml", DEFAULT_KEYBINDING.resource_yaml.key),
    action_hint("menu", DEFAULT_KEYBINDING.open_action_menu.key)
  )
}

pub fn describe_yaml_and_logs_hint() -> String {
  format!(
    "{} · {} ",
    describe_and_yaml_hint().trim_end(),
    action_hint("logs", DEFAULT_KEYBINDING.aggregate_logs.key)
  )
}

pub fn describe_yaml_logs_and_esc_hint() -> String {
  format!(
    "{} · {}:back ",
    describe_yaml_and_logs_hint().trim_end(),
    DEFAULT_KEYBINDING.esc.key.symbol()
  )
}

pub fn describe_yaml_and_esc_hint() -> String {
  format!(
    "{} · {}:back ",
    describe_and_yaml_hint().trim_end(),
    DEFAULT_KEYBINDING.esc.key.symbol()
  )
}

pub fn describe_yaml_decode_and_esc_hint() -> String {
  format!(
    "{} · {} · {}:back ",
    describe_and_yaml_hint().trim_end(),
    action_hint("decode", DEFAULT_KEYBINDING.decode_secret.key),
    DEFAULT_KEYBINDING.esc.key.symbol()
  )
}

pub fn wide_hint() -> String {
  format!(
    "{}:wide",
    DEFAULT_KEYBINDING.toggle_wide_columns.key.symbol()
  )
}

pub fn filter_cursor_position(area: Rect, prefix_width: usize, filter: &str) -> Position {
  Position {
    x: area.x
      + (prefix_width as u16 + 1 + filter.chars().count() as u16).min(area.width.saturating_sub(2)),
    y: area.y,
  }
}

fn styled_line<'a, I>(parts: I, palette: Palette, bold: bool) -> Line<'a>
where
  I: IntoIterator<Item = LinePart<'a>>,
{
  Line::from(
    parts
      .into_iter()
      .map(|part| {
        let style = line_part_style(&part, palette, bold);
        match part {
          LinePart::Default(text) | LinePart::Help(text) => Span::styled(text, style),
        }
      })
      .collect::<Vec<_>>(),
  )
}

/// Gauge fill colour tier, matching LlamaStash: green below 60%, amber
/// 60–85%, red 85%+.
pub fn gauge_fill_style(ratio: f64, palette: Palette) -> Style {
  if ratio >= 0.85 {
    style_failure(palette)
  } else if ratio >= 0.6 {
    style_caution(palette)
  } else {
    style_success(palette)
  }
}

/// LlamaStash-style block bar `████░░░░` of `width` cells: `█` fill, `░`
/// trough. The fill colour owns the whole span — the 25%-density trough
/// glyph naturally reads as a dimmer shade. ASCII fallback when unicode
/// symbols are disabled.
fn gauge_bar_span(pct: f64, width: usize, fill: Style, enhanced_graphics: bool) -> Span<'static> {
  if width == 0 {
    return Span::raw("");
  }
  let pct = if pct.is_finite() {
    pct.clamp(0.0, 100.0)
  } else {
    0.0
  };
  let (fill_char, trough_char) = if enhanced_graphics {
    ('█', '░')
  } else {
    ('#', '.')
  };
  let filled = (((pct / 100.0) * width as f64).round() as usize).min(width);
  let mut bar = String::with_capacity(width * 3);
  for _ in 0..filled {
    bar.push(fill_char);
  }
  for _ in 0..width - filled {
    bar.push(trough_char);
  }
  Span::styled(bar, fill)
}

/// One LlamaStash-style gauge line `LABEL ████░░░░ 42%`: label column, bar,
/// then the value. The bar takes whatever width is left after the label and
/// value (minimum 4 cells); values over 100% show as-is with a full bar.
pub fn gauge_line<'a>(
  label: String,
  pct: f64,
  value: String,
  total_width: u16,
  palette: Palette,
  enhanced_graphics: bool,
) -> Line<'a> {
  let reserve = label.chars().count() + value.chars().count() + 1;
  let bar_width = (total_width as usize).saturating_sub(reserve).max(4);
  let ratio = if pct.is_finite() {
    (pct / 100.0).clamp(0.0, 1.0)
  } else {
    0.0
  };
  Line::from(vec![
    Span::styled(label, style_label(palette)),
    gauge_bar_span(
      pct,
      bar_width,
      gauge_fill_style(ratio, palette),
      enhanced_graphics,
    ),
    Span::styled(format!(" {value}"), style_text(palette)),
  ])
}

pub fn table_header_style(cells: Vec<&str>, palette: Palette) -> Row<'_> {
  Row::new(cells).style(style_label(palette)).bottom_margin(0)
}

pub fn horizontal_chunks(constraints: Vec<Constraint>, size: Rect) -> Rc<[Rect]> {
  Layout::default()
    .constraints(<Vec<Constraint> as AsRef<[Constraint]>>::as_ref(
      &constraints,
    ))
    .direction(Direction::Horizontal)
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

/// Border colour for an unfocused panel — the `accent`/primary tone.
fn border_style(palette: Palette) -> Style {
  Style::default().fg(palette.focus_border(false))
}

/// Border colour for the focused/active panel — the `highlight` tone (falls
/// back to accent for themes without a highlight, e.g. mono).
fn active_border_style(palette: Palette) -> Style {
  Style::default().fg(palette.focus_border(true))
}

pub fn layout_block(title: Span<'_>, palette: Palette) -> Block<'_> {
  Block::default()
    .borders(Borders::ALL)
    .border_style(border_style(palette))
    .title(title)
}

pub fn layout_block_default<'a>(title: &'a str, palette: Palette) -> Block<'a> {
  layout_block(title_style(title, palette), palette)
}

pub fn layout_block_default_line(title: Line<'_>, palette: Palette) -> Block<'_> {
  Block::default()
    .borders(Borders::ALL)
    .border_style(border_style(palette))
    .title(title)
}

pub fn layout_block_active_line(title: Line<'_>, palette: Palette) -> Block<'_> {
  Block::default()
    .borders(Borders::ALL)
    .border_style(active_border_style(palette))
    .title(title)
}

pub fn layout_block_active_span(title: Line<'_>, palette: Palette) -> Block<'_> {
  layout_block_active_line(title, palette)
}

pub fn layout_block_top_border(title: Line<'_>, palette: Palette) -> Block<'_> {
  Block::default()
    .borders(Borders::TOP)
    .border_style(border_style(palette))
    .title(title)
}

/// Shared centred popup menu (the `m` action menu and the more/dynamic resource
/// pickers). Consistent styling: secondary/yellow title + border, body text in
/// `fg`, bold secondary highlight on the selection. The caller supplies the
/// pre-centred `area`, the title line (label + its own hint), and the items.
pub fn draw_popup_menu(
  f: &mut Frame<'_>,
  area: Rect,
  title: Line<'_>,
  items: Vec<ListItem<'_>>,
  state: &mut ListState,
  palette: Palette,
) {
  let block = Block::default()
    .borders(Borders::ALL)
    .border_style(style_secondary(palette))
    .title(title);
  f.render_widget(Clear, area);
  f.render_stateful_widget(
    List::new(items)
      .block(block)
      .style(style_text(palette))
      .highlight_style(style_secondary(palette).add_modifier(Modifier::BOLD))
      .highlight_symbol(HIGHLIGHT),
    area,
    state,
  );
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
  let clear_suffix = format!(" · {}:clear ", DEFAULT_KEYBINDING.esc.key.symbol());
  let edit_suffix = format!(" · {}:edit ", DEFAULT_KEYBINDING.filter.key.symbol());

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

pub fn title_with_dual_style<'a>(part_1: String, part_2: Line<'a>, palette: Palette) -> Line<'a> {
  let mut spans = vec![Span::styled(
    part_1,
    style_secondary(palette).add_modifier(Modifier::BOLD),
  )];
  spans.extend(part_2.spans);
  Line::from(spans)
}

pub fn copy_and_escape_title_line<'a, S: Into<Cow<'a, str>>>(
  _target: S,
  palette: Palette,
) -> Line<'a> {
  mixed_bold_line(
    [
      help_part(format!(
        "{} · ",
        action_hint("copy", DEFAULT_KEYBINDING.copy_to_clipboard.key)
      )),
      help_part(format!("{}:back ", DEFAULT_KEYBINDING.esc.key.symbol())),
    ],
    palette,
  )
}

pub fn copy_scroll_and_escape_title_line<'a, S: Into<Cow<'a, str>>>(
  _target: S,
  auto_scroll: bool,
  palette: Palette,
) -> Line<'a> {
  let auto_scroll_action = if auto_scroll {
    "pause scroll"
  } else {
    "resume scroll"
  };
  mixed_bold_line(
    [
      help_part(format!(
        "{} · {} · ",
        action_hint("copy", DEFAULT_KEYBINDING.copy_to_clipboard.key),
        action_hint(auto_scroll_action, DEFAULT_KEYBINDING.log_auto_scroll.key)
      )),
      help_part(format!("{}:back ", DEFAULT_KEYBINDING.esc.key.symbol())),
    ],
    palette,
  )
}

pub fn split_hint_suffix(text: &str) -> (&str, Option<&str>) {
  if let Some(pos) = text.rfind(" <") {
    (&text[..pos], Some(&text[(pos + 1)..]))
  } else {
    (text, None)
  }
}

/// Strip the `<>` that `Key`'s Display adds, leaving the bare glyph for
/// `key:label` hints. Tab jump-keys are all single chars, so exact here.
pub fn hint_key_glyph(hint: &str) -> &str {
  hint.trim_start_matches('<').trim_end_matches('>')
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

pub fn loading(
  f: &mut Frame<'_>,
  block: Block<'_>,
  area: Rect,
  is_loading: bool,
  palette: Palette,
) {
  if is_loading {
    let text = "\n\n Loading ...\n\n".to_owned();
    let text = Text::from(text);
    let text = text.patch_style(style_secondary(palette));

    // Contains the text
    let paragraph = Paragraph::new(text)
      .style(style_secondary(palette))
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
          $crate::ui::utils::copy_and_escape_title_line($title, $app.palette),
          $app.palette,
        ),
      ),
      ActiveBlock::Yaml => draw_yaml_block(
        $f,
        $app,
        $area,
        title_with_dual_style(
          get_resource_title($app, $title, get_describe_active($block), $res.items.len()),
          $crate::ui::utils::copy_and_escape_title_line($title, $app.palette),
          $app.palette,
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
    || app.data.describe_out.highlight_is_dark != app.is_dark()
  {
    let ss = get_syntax_set();
    let syntax = get_yaml_syntax_reference();
    let theme = if app.is_dark() {
      &get_yaml_themes().dark
    } else {
      &get_yaml_themes().light
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
    app.data.describe_out.highlight_is_dark = app.is_dark();
  }
  true
}

/// Compute the (start, end, scroll-within-slice) window into a buffer of
/// `total` highlighted lines for the given `offset` and visible row count.
/// Clamps `offset` to a valid index and ensures `start <= end <= total`.
fn highlight_window(offset: usize, total: usize, view_h: usize) -> (usize, usize, u16) {
  // Caller guarantees total > 0; clamp here defensively too.
  let view_h = view_h.max(1);
  let effective_offset = offset.min(total.saturating_sub(1));
  let slice_start = effective_offset.saturating_sub(view_h);
  let slice_end = total.min(effective_offset + view_h * 3);
  let adjusted_offset = (effective_offset - slice_start).min(u16::MAX as usize) as u16;
  (slice_start, slice_end, adjusted_offset)
}

/// common for all resources
pub fn draw_yaml_block(f: &mut Frame<'_>, app: &mut App, area: Rect, title: Line<'_>) {
  let palette = app.palette;
  let block = layout_block_top_border(title, palette);
  if ensure_highlight_cache(app) {
    let total = app.data.describe_out.highlighted_lines.len();
    if total == 0 {
      loading(f, block, area, app.is_loading(), palette);
      return;
    }
    let offset = app.data.describe_out.offset;
    // Subtract 2 for the top-border of the block; clamp to >=1 so a tiny
    // terminal doesn't degenerate into an empty slice.
    let view_h = (area.height.saturating_sub(2) as usize).max(1);
    let (slice_start, slice_end, adjusted_offset) = highlight_window(offset, total, view_h);
    let visible_lines = app.data.describe_out.highlighted_lines[slice_start..slice_end].to_vec();
    let paragraph = Paragraph::new(visible_lines)
      .block(block)
      .wrap(Wrap { trim: false })
      .scroll((adjusted_offset, 0));
    f.render_widget(paragraph, area);
  } else {
    loading(f, block, area, app.is_loading(), palette);
  }
}

fn draw_resource_table<'a, T: Named, F>(
  f: &mut Frame<'_>,
  area: Rect,
  table_props: ResourceTableProps<'a, T>,
  row_cell_mapper: F,
  palette: Palette,
  is_loading: bool,
  block: Block<'a>,
) where
  F: Fn(&T) -> Row<'a>,
{
  if !table_props.resource.items.is_empty() {
    let filter = table_props.resource.filter.to_lowercase();
    let has_filter = !filter.is_empty();
    let mut filtered_indices: Vec<usize> = Vec::new();
    let mut filtered_items: Vec<&T> = Vec::new();
    for (idx, item) in table_props.resource.items.iter().enumerate() {
      if !has_filter || filter_by_name(&filter, item) {
        if has_filter {
          filtered_indices.push(idx);
        }
        filtered_items.push(item);
      }
    }

    if has_filter {
      let max = filtered_items.len().saturating_sub(1);
      if let Some(sel) = table_props.resource.state.selected() {
        if sel > max {
          table_props.resource.state.select(Some(max));
        }
      }
    }
    table_props.resource.filtered_indices = filtered_indices;

    // Skip row_cell_mapper for off-screen items: ratatui's Table only paints
    // rows intersecting the visible area, so we can hand it cheap empty Rows
    // outside the window. The window must bracket the legal range of
    // state.offset() (which ratatui keeps within `selected ± view_h`),
    // hence `selected.saturating_sub(view_h)` and `selected + view_h * 2`.
    // view_h is clamped to >=1 so a tiny terminal still renders one row.
    let selected = table_props.resource.state.selected().unwrap_or(0);
    let view_h = (area.height.saturating_sub(3) as usize).max(1);
    let visible_start = selected.saturating_sub(view_h);
    let visible_end = (selected + view_h * 2).min(filtered_items.len());

    let rows: Vec<Row<'a>> = filtered_items
      .iter()
      .enumerate()
      .map(|(fi, item)| {
        if fi >= visible_start && fi < visible_end {
          row_cell_mapper(item)
        } else {
          Row::default()
        }
      })
      .collect();

    let table = Table::new(rows, &table_props.column_widths)
      .header(table_header_style(table_props.table_headers, palette))
      .block(block)
      .row_highlight_style(style_highlight())
      .highlight_symbol(HIGHLIGHT);

    f.render_stateful_widget(table, area, &mut table_props.resource.state);
  } else {
    loading(f, block, area, is_loading, palette);
  }
}

/// Builds the help `Line` for a resource block title, weaving filter status
/// into any existing inline help (placing it after a "containers" prefix when present).
fn build_resource_help_line(
  inline_help: Line<'_>,
  filter: &str,
  filter_active: bool,
  palette: Palette,
) -> Line<'static> {
  let inline_help_text = inline_help
    .spans
    .iter()
    .map(|span| span.content.as_ref())
    .collect::<String>();
  let containers_prefix = format!(
    "{} · ",
    action_hint("containers", DEFAULT_KEYBINDING.submit.key)
  );
  let mut help_parts: Vec<LinePart<'static>> = Vec::new();
  if let Some(rest) = inline_help_text.strip_prefix(&containers_prefix) {
    help_parts.push(help_part(containers_prefix));
    help_parts.extend(owned_filter_status_parts(filter, filter_active));
    if !rest.is_empty() {
      help_parts.push(help_part(" · ".to_string()));
      help_parts.push(help_part(rest.to_string()));
    }
  } else {
    help_parts.extend(owned_filter_status_parts(filter, filter_active));
    if !inline_help_text.is_empty() {
      help_parts.push(help_part(" · ".to_string()));
      help_parts.push(help_part(inline_help_text));
    }
  }
  mixed_bold_line(help_parts, palette)
}

/// Draw a kubernetes resource overview tab
pub fn draw_resource_block<'a, T: Named, F>(
  f: &mut Frame<'_>,
  area: Rect,
  table_props: ResourceTableProps<'a, T>,
  row_cell_mapper: F,
  palette: Palette,
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
      mixed_bold_line(owned_filter_status_parts(&filter, true), palette),
      palette,
    );
    let block = layout_block_top_border(title, palette);
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
      palette,
      is_loading,
      block,
    );
    f.set_cursor_position(filter_cursor_position(area, title_width, &filter));
    return;
  }

  let help_line = build_resource_help_line(inline_help, &filter, filter_active, palette);
  let title = title_with_dual_style(title, help_line, palette);
  let block = layout_block_top_border(title, palette);
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
    palette,
    is_loading,
    block,
  );
}

pub fn draw_route_resource_block<'a, T: Named, F>(
  f: &mut Frame<'_>,
  area: Rect,
  table_props: ResourceTableProps<'a, T>,
  row_cell_mapper: F,
  palette: Palette,
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
      mixed_bold_line(owned_filter_status_parts(&filter, true), palette),
      palette,
    );
    let block = layout_block_active_span(title, palette);
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
      palette,
      is_loading,
      block,
    );
    f.set_cursor_position(filter_cursor_position(area, title_width, &filter));
    return;
  }

  let title = title_with_dual_style(title, inline_help, palette);
  let block = layout_block_active_span(title, palette);
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
    palette,
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
  use crate::ui::theme::{palette_for, ThemeName};

  #[test]
  fn test_gauge_fill_style_tiers() {
    let palette = palette_for(ThemeName::Macchiato);
    assert_eq!(gauge_fill_style(0.0, palette), style_success(palette));
    assert_eq!(gauge_fill_style(0.59, palette), style_success(palette));
    assert_eq!(gauge_fill_style(0.6, palette), style_caution(palette));
    assert_eq!(gauge_fill_style(0.84, palette), style_caution(palette));
    assert_eq!(gauge_fill_style(0.85, palette), style_failure(palette));
    assert_eq!(gauge_fill_style(1.0, palette), style_failure(palette));
  }

  #[test]
  fn test_gauge_line_layout() {
    let palette = palette_for(ThemeName::Macchiato);
    // 20 cells total - "CPU  " (5) - " 50%" (4) = 11 bar cells, 6 filled
    let line = gauge_line("CPU  ".into(), 50.0, "50%".into(), 20, palette, true);
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(text, "CPU  ██████░░░░░ 50%");

    // values over 100% keep their real number with a full bar
    let line = gauge_line("Lim  ".into(), 250.0, "250%".into(), 20, palette, true);
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(text, "Lim  ██████████ 250%");

    // ascii fallback when enhanced graphics are off
    let line = gauge_line("CPU  ".into(), 50.0, "50%".into(), 20, palette, false);
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(text, "CPU  ######..... 50%");
  }

  #[test]
  fn test_draw_resource_block() {
    let backend = TestBackend::new(100, 6);
    let mut terminal = Terminal::new(backend).unwrap();
    let p = palette_for(ThemeName::Macchiato);

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
            inline_help: help_bold_line("-> yaml <y>", p),
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
            .style(style_primary(p))
          },
          p,
          false,
        );
      })
      .unwrap();

    let mut expected = Buffer::with_lines(vec![
        "Test/:filter · -> yaml <y>──────────────────────────────────────────────────────────────────────────",
        "   Namespace                     Name                                 Data           Age            ",
        "=> Test ns                       Test 1                               5              65h3m          ",
        "   Test ns                       Test long name that should be trunca 3              65h3m          ",
        "   Test ns long value check that test_long_name_that_should_be_trunca 6              65h3m          ",
        "                                                                                                    ",
      ]);
    // set row styles
    // First row: title text (secondary), hints (muted), then accent top border
    for col in 0..=99 {
      match col {
        0..=3 => {
          expected.cell_mut(Position::new(col, 0)).unwrap().set_style(
            Style::default()
              .fg(p.secondary)
              .add_modifier(Modifier::BOLD),
          );
        }
        4..=25 => {
          expected
            .cell_mut(Position::new(col, 0))
            .unwrap()
            .set_style(Style::default().fg(p.muted).add_modifier(Modifier::BOLD));
        }
        _ => {
          expected
            .cell_mut(Position::new(col, 0))
            .unwrap()
            .set_style(Style::default().fg(p.accent));
        }
      }
    }

    // Second row table header style (labels → blue)
    for col in 0..=99 {
      expected
        .cell_mut(Position::new(col, 1))
        .unwrap()
        .set_style(Style::default().fg(p.label));
    }
    // first table data row style
    for col in 0..=99 {
      expected.cell_mut(Position::new(col, 2)).unwrap().set_style(
        Style::default()
          .fg(p.accent)
          .add_modifier(Modifier::REVERSED),
      );
    }
    // remaining table data row style
    for row in 3..=4 {
      for col in 0..=99 {
        expected
          .cell_mut(Position::new(col, row))
          .unwrap()
          .set_style(Style::default().fg(p.accent));
      }
    }

    terminal.backend().assert_buffer(&expected);
  }

  #[test]
  fn test_draw_resource_block_filter() {
    let backend = TestBackend::new(100, 6);
    let mut terminal = Terminal::new(backend).unwrap();
    let p = palette_for(ThemeName::Macchiato);

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
            inline_help: help_bold_line("-> yaml <y>", p),
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
            .style(style_primary(p))
          },
          p,
          false,
        );
      })
      .unwrap();

    let mut expected = Buffer::with_lines(vec![
        "Test[truncated] · /:edit  · -> yaml <y>─────────────────────────────────────────────────────────────",
        "   Namespace                     Name                                 Data           Age            ",
        "=> Test ns                       Test long name that should be trunca 3              65h3m          ",
        "   Test ns long value check that test_long_name_that_should_be_trunca 6              65h3m          ",
        "                                                                                                    ",
        "                                                                                                    ",
      ]);
    // set row styles
    // First row: title (secondary), filter value (fg), hints (muted), border (accent)
    for col in 0..=99 {
      match col {
        0..=3 => {
          expected.cell_mut(Position::new(col, 0)).unwrap().set_style(
            Style::default()
              .fg(p.secondary)
              .add_modifier(Modifier::BOLD),
          );
        }
        4..=14 => {
          expected
            .cell_mut(Position::new(col, 0))
            .unwrap()
            .set_style(Style::default().fg(p.fg).add_modifier(Modifier::BOLD));
        }
        15..=38 => {
          expected
            .cell_mut(Position::new(col, 0))
            .unwrap()
            .set_style(Style::default().fg(p.muted).add_modifier(Modifier::BOLD));
        }
        _ => {
          expected
            .cell_mut(Position::new(col, 0))
            .unwrap()
            .set_style(Style::default().fg(p.accent));
        }
      }
    }

    // Second row table header style (labels → blue)
    for col in 0..=99 {
      expected
        .cell_mut(Position::new(col, 1))
        .unwrap()
        .set_style(Style::default().fg(p.label));
    }
    // first table data row style
    for col in 0..=99 {
      expected.cell_mut(Position::new(col, 2)).unwrap().set_style(
        Style::default()
          .fg(p.accent)
          .add_modifier(Modifier::REVERSED),
      );
    }
    // remaining table data row style
    for row in 3..=3 {
      for col in 0..=99 {
        expected
          .cell_mut(Position::new(col, row))
          .unwrap()
          .set_style(Style::default().fg(p.accent));
      }
    }

    terminal.backend().assert_buffer(&expected);
  }

  #[test]
  fn test_draw_resource_block_filter_glob() {
    let backend = TestBackend::new(100, 6);
    let mut terminal = Terminal::new(backend).unwrap();
    let p = palette_for(ThemeName::Macchiato);

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
            inline_help: help_bold_line("-> yaml <y>", p),
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
            .style(style_primary(p))
          },
          p,
          false,
        );
      })
      .unwrap();

    let mut expected = Buffer::with_lines(vec![
        "Test[*long*truncated*] · /:edit  · -> yaml <y>──────────────────────────────────────────────────────",
        "   Namespace                     Name                                 Data           Age            ",
        "=> Test ns                       Test long name that should be trunca 3              65h3m          ",
        "   Test ns long value check that test_long_name_that_should_be_trunca 6              65h3m          ",
        "                                                                                                    ",
        "                                                                                                    ",
      ]);
    // set row styles
    // First row: title (secondary), filter value (fg), hints (muted), border (accent)
    for col in 0..=99 {
      match col {
        0..=3 => {
          expected.cell_mut(Position::new(col, 0)).unwrap().set_style(
            Style::default()
              .fg(p.secondary)
              .add_modifier(Modifier::BOLD),
          );
        }
        4..=21 => {
          expected
            .cell_mut(Position::new(col, 0))
            .unwrap()
            .set_style(Style::default().fg(p.fg).add_modifier(Modifier::BOLD));
        }
        22..=45 => {
          expected
            .cell_mut(Position::new(col, 0))
            .unwrap()
            .set_style(Style::default().fg(p.muted).add_modifier(Modifier::BOLD));
        }
        _ => {
          expected
            .cell_mut(Position::new(col, 0))
            .unwrap()
            .set_style(Style::default().fg(p.accent));
        }
      }
    }

    // Second row table header style (labels → blue)
    for col in 0..=99 {
      expected
        .cell_mut(Position::new(col, 1))
        .unwrap()
        .set_style(Style::default().fg(p.label));
    }
    // first table data row style
    for col in 0..=99 {
      expected.cell_mut(Position::new(col, 2)).unwrap().set_style(
        Style::default()
          .fg(p.accent)
          .add_modifier(Modifier::REVERSED),
      );
    }
    // remaining table data row style
    for row in 3..=3 {
      for col in 0..=99 {
        expected
          .cell_mut(Position::new(col, row))
          .unwrap()
          .set_style(Style::default().fg(p.accent));
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
    let p = palette_for(ThemeName::Macchiato);

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
            inline_help: help_bold_line("d:describe · Esc:back", p),
            resource: &mut resource,
            table_headers: vec!["Name"],
            column_widths: vec![Constraint::Percentage(100)],
          },
          |c| Row::new(vec![Cell::from(c.name.to_owned())]).style(style_primary(p)),
          p,
          false,
        );
      })
      .unwrap();

    let first_line = (0..terminal.backend().buffer().area.width)
      .map(|col| terminal.backend().buffer()[(col, 0)].symbol())
      .collect::<String>();
    assert!(first_line.contains("[pod]"));
    assert!(first_line.contains("Esc:clear"));
    assert!(!first_line.contains("d:describe"));
    assert!(!first_line.contains("Esc:back"));
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
    let p = palette_for(ThemeName::Macchiato);
    // Case 1: Empty inline_help, empty filter, filter_active=false
    // -> line text should contain the inactive "filter <key>" action hint
    let line = build_resource_help_line(Line::default(), "", false, p);
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    let expected_filter_hint = action_hint("filter", DEFAULT_KEYBINDING.filter.key);
    assert!(
      text.contains(&expected_filter_hint),
      "Case 1: expected '{text}' to contain '{expected_filter_hint}'"
    );

    // Case 2: Non-empty inline_help, empty filter, filter_active=false
    // -> line text should contain the inline help hint after " · "
    let line2 = build_resource_help_line(help_bold_line("-> yaml <y>", p), "", false, p);
    let text2: String = line2.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
      text2.contains("-> yaml <y>"),
      "Case 2: expected '{text2}' to contain '-> yaml <y>'"
    );

    // Case 3: inline_help starting with the containers prefix
    // -> line text should start with the containers hint
    let containers_prefix_str = format!(
      "{} · ",
      action_hint("containers", DEFAULT_KEYBINDING.submit.key)
    );
    let line3 = build_resource_help_line(
      help_bold_line(containers_prefix_str.as_str(), p),
      "",
      false,
      p,
    );
    let text3: String = line3.spans.iter().map(|s| s.content.as_ref()).collect();
    let containers_hint = action_hint("containers", DEFAULT_KEYBINDING.submit.key);
    assert!(
      text3.starts_with(&containers_hint),
      "Case 3: expected '{text3}' to start with '{containers_hint}'"
    );

    // Case 4: Empty inline_help, filter="foo", filter_active=false
    // -> line text should contain "[foo]"
    let line4 = build_resource_help_line(Line::default(), "foo", false, p);
    let text4: String = line4.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
      text4.contains("[foo]"),
      "Case 4: expected '{text4}' to contain '[foo]'"
    );
  }

  #[test]
  fn test_highlight_window_offset_within_bounds() {
    // total=100, view_h=10, offset=50 → window straddles the offset.
    let (start, end, scroll) = highlight_window(50, 100, 10);
    assert_eq!(start, 40);
    assert_eq!(end, 80);
    assert_eq!(scroll, 10);
    assert!(start <= end && end <= 100);
  }

  #[test]
  fn test_highlight_window_offset_at_zero() {
    let (start, end, scroll) = highlight_window(0, 100, 10);
    assert_eq!(start, 0);
    assert_eq!(end, 30);
    assert_eq!(scroll, 0);
  }

  #[test]
  fn test_highlight_window_offset_exceeds_total_does_not_panic() {
    // Regression: items.len() can exceed highlighted_lines.len() when
    // some lines fail to highlight, leaving offset stale relative to total.
    // The slice [start..end] must remain valid.
    let (start, end, _) = highlight_window(50, 5, 10);
    assert!(start <= end, "start {start} must not exceed end {end}");
    assert!(end <= 5, "end {end} must not exceed total");
  }

  #[test]
  fn test_highlight_window_view_h_zero_clamps_to_one() {
    // A view height of 0 should not collapse the window to empty.
    let (start, end, _) = highlight_window(2, 10, 0);
    assert!(start < end, "window must not be empty when content exists");
  }

  #[test]
  fn test_highlight_window_total_one() {
    // Single-line buffer must produce a non-empty window.
    let (start, end, scroll) = highlight_window(0, 1, 5);
    assert_eq!(start, 0);
    assert_eq!(end, 1);
    assert_eq!(scroll, 0);
  }
}
