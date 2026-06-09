use ratatui::{
  layout::{Constraint, Rect},
  widgets::{Row, Table},
  Frame,
};

use super::{
  utils::{
    default_part, filter_cursor_position, filter_status_parts, help_part, layout_block_active_line,
    mixed_bold_line, style_highlight, style_label, style_text, text_matches_filter,
    vertical_chunks,
  },
  HIGHLIGHT,
};
use crate::app::{key_binding::DEFAULT_KEYBINDING, models::FilterableTable, App};

pub fn draw_help(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let chunks = vertical_chunks(vec![Constraint::Percentage(100)], area);

  // Create a one-column table to avoid flickering due to non-determinism when
  // resolving constraints on widths of table columns.
  let format_row =
    |r: &Vec<String>| -> Vec<String> { vec![format!("{:50}{:40}{:20}", r[0], r[1], r[2])] };

  let header = ["Key", "Action", "Context"];
  let header = format_row(&header.iter().map(|s| s.to_string()).collect());

  let title = format!(" Help [{}] ", app.help_docs.count_label());
  let mut title_parts = vec![default_part(&title)];
  title_parts.extend(filter_status_parts(
    &app.help_docs.filter,
    app.help_docs.filter_active,
  ));
  if !app.help_docs.filter_active {
    title_parts.push(help_part(format!(
      " · back {} ",
      DEFAULT_KEYBINDING.esc.key
    )));
  }

  let filter = app.help_docs.filter.to_lowercase();
  let has_filter = !filter.is_empty();
  let mut filtered_indices = Vec::new();
  let rows: Vec<_> = app
    .help_docs
    .items
    .iter()
    .enumerate()
    .filter_map(|(idx, item)| {
      if !help_doc_matches_filter(&filter, item) {
        return None;
      }
      if has_filter {
        filtered_indices.push(idx);
      }

      Some(Row::new(format_row(item)).style(style_text(app.palette)))
    })
    .collect();

  if has_filter {
    let max = filtered_indices.len().saturating_sub(1);
    if let Some(sel) = app.help_docs.state.selected() {
      if sel > max {
        app.help_docs.state.select(Some(max));
      }
    }
  }
  app.help_docs.filtered_indices = filtered_indices;

  let help_menu = Table::new(rows, [Constraint::Percentage(100)])
    .header(
      Row::new(header)
        .style(style_label(app.palette))
        .bottom_margin(0),
    )
    .block(layout_block_active_line(
      mixed_bold_line(title_parts, app.palette),
      app.palette,
    ))
    .row_highlight_style(style_highlight())
    .highlight_symbol(HIGHLIGHT);
  f.render_stateful_widget(help_menu, chunks[0], &mut app.help_docs.state);

  if app.help_docs.filter_active {
    f.set_cursor_position(filter_cursor_position(
      area,
      title.chars().count() + 1,
      &app.help_docs.filter,
    ));
  }
}

fn help_doc_matches_filter(filter: &str, item: &[String]) -> bool {
  item.iter().any(|value| text_matches_filter(filter, value))
}

#[cfg(test)]
mod tests {
  use ratatui::{backend::TestBackend, style::Modifier, Terminal};

  use super::*;
  use crate::ui::theme::{palette_for, ThemeName};

  #[test]
  fn test_draw_help() {
    let backend = TestBackend::new(100, 7);
    let mut terminal = Terminal::new(backend).unwrap();
    let p = palette_for(ThemeName::Macchiato);

    terminal
      .draw(|f| {
        let size = f.area();
        let mut app = App::default();
        draw_help(f, &mut app, size);
      })
      .unwrap();

    let buffer = terminal.backend().buffer();
    let lines: Vec<String> = (0..buffer.area.height)
      .map(|row| {
        (0..buffer.area.width)
          .map(|col| buffer[(col, row)].symbol())
          .collect::<String>()
      })
      .collect();

    assert_eq!(
      lines,
      vec![
        "┌ Help [48] filter </> · back <Esc> ───────────────────────────────────────────────────────────────┐",
        "│   Key                                               Action                                  Conte│",
        "│=> <Ctrl+c> | <q>                                    Quit                                    Gener│",
        "│   <Esc>                                             Close child page/Go back                Gener│",
        "│   <?>                                               Help page                               Gener│",
        "│   <Enter>                                           Select table row                        Gener│",
        "└──────────────────────────────────────────────────────────────────────────────────────────────────┘",
      ]
    );

    // Border corners use the focused/highlight tone.
    assert_eq!(buffer[(0, 0)].fg, p.highlight);
    // Title text (default part) → fg, bold.
    assert_eq!(buffer[(1, 0)].fg, p.fg);
    assert!(buffer[(1, 0)].modifier.contains(Modifier::BOLD));
    // Hints → muted, bold.
    assert_eq!(buffer[(12, 0)].fg, p.muted);
    assert!(buffer[(12, 0)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(23, 0)].fg, p.muted);
    assert!(buffer[(23, 0)].modifier.contains(Modifier::BOLD));
    // Selected data row → text + reversed.
    assert_eq!(buffer[(1, 2)].fg, p.fg);
    assert!(buffer[(1, 2)].modifier.contains(Modifier::REVERSED));
    assert_eq!(buffer[(1, 3)].fg, p.fg);
    assert_eq!(buffer[(99, 6)].fg, p.highlight);
  }

  #[test]
  fn test_draw_help_hides_close_hint_while_filtering() {
    let backend = TestBackend::new(100, 7);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
      .draw(|f| {
        let size = f.area();
        let mut app = App::default();
        app.help_docs.filter_active = true;
        app.help_docs.filter = "pod".into();
        app.help_docs.filtered_indices = vec![2];
        draw_help(f, &mut app, size);
      })
      .unwrap();

    let buffer = terminal.backend().buffer();
    let first_line: String = (0..buffer.area.width)
      .map(|col| buffer[(col, 0)].symbol())
      .collect();

    assert!(first_line.contains("Help [1/48] [pod] · clear <Esc>"));
    assert!(!first_line.contains("back <Esc>"));
  }
}
