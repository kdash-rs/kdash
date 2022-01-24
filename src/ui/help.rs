use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  widgets::{Row, Table},
  Frame,
};

use super::{
  utils::{
    layout_block_active_span, style_highlight, style_primary, style_secondary,
    title_with_dual_style, vertical_chunks,
  },
  HIGHLIGHT,
};
use crate::app::App;

pub fn draw_help<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let chunks = vertical_chunks(vec![Constraint::Percentage(100)], area);

  // Create a one-column table to avoid flickering due to non-determinism when
  // resolving constraints on widths of table columns.
  let format_row =
    |r: &Vec<String>| -> Vec<String> { vec![format!("{:50}{:40}{:20}", r[0], r[1], r[2])] };

  let header = ["Key", "Action", "Context"];
  let header = format_row(&header.iter().map(|s| s.to_string()).collect());

  let help_docs = app
    .help_docs
    .items
    .iter()
    .map(format_row)
    .collect::<Vec<Vec<String>>>();
  let help_docs = &help_docs[0_usize..];

  let rows = help_docs
    .iter()
    .map(|item| Row::new(item.clone()).style(style_primary()));

  let title = title_with_dual_style("Help ".into(), "| close <esc>".into(), app.light_theme);

  let help_menu = Table::new(rows)
    .header(Row::new(header).style(style_secondary()).bottom_margin(0))
    .block(layout_block_active_span(title))
    .highlight_style(style_highlight())
    .highlight_symbol(HIGHLIGHT)
    .widths(&[Constraint::Percentage(100)]);
  f.render_stateful_widget(help_menu, chunks[0], &mut app.help_docs.state);
}

#[cfg(test)]
mod tests {
  use tui::{
    backend::TestBackend,
    buffer::Buffer,
    style::{Color, Modifier, Style},
    Terminal,
  };

  use super::*;

  #[test]
  fn test_draw_help() {
    let backend = TestBackend::new(100, 7);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
      .draw(|f| {
        let size = f.size();
        let mut app = App::default();
        draw_help(f, &mut app, size);
      })
      .unwrap();

    let mut expected = Buffer::with_lines(vec![
        "┌Help | close <esc>────────────────────────────────────────────────────────────────────────────────┐",
        "│   Key                                               Action                                  Conte│",
        "│=> <Ctrl+c> | <q>                                    Quit                                    Gener│",
        "│   <Esc>                                             Close child page/Go back                Gener│",
        "│   <?>                                               Help page                               Gener│",
        "│   <Enter>                                           Select table row                        Gener│",
        "└──────────────────────────────────────────────────────────────────────────────────────────────────┘",
      ]);
    // set row styles
    // First row heading style
    for col in 0..=99 {
      match col {
        0 | 19..=99 => {
          expected
            .get_mut(col, 0)
            .set_style(Style::default().fg(Color::Yellow));
        }
        1..=5 => {
          expected.get_mut(col, 0).set_style(
            Style::default()
              .fg(Color::Yellow)
              .add_modifier(Modifier::BOLD),
          );
        }
        _ => {
          expected.get_mut(col, 0).set_style(
            Style::default()
              .fg(Color::White)
              .add_modifier(Modifier::BOLD),
          );
        }
      }
    }

    // second row table headings
    for col in 0..=99 {
      expected
        .get_mut(col, 1)
        .set_style(Style::default().fg(Color::Yellow));
    }

    // first table data row style
    for col in 0..=99 {
      match col {
        1..=98 => {
          expected.get_mut(col, 2).set_style(
            Style::default()
              .fg(Color::Cyan)
              .add_modifier(Modifier::REVERSED),
          );
        }
        _ => {
          expected
            .get_mut(col, 2)
            .set_style(Style::default().fg(Color::Yellow));
        }
      }
    }

    // rows
    for row in 3..=5 {
      for col in 0..=99 {
        match col {
          1..=98 => {
            expected
              .get_mut(col, row)
              .set_style(Style::default().fg(Color::Cyan));
          }
          _ => {
            expected
              .get_mut(col, row)
              .set_style(Style::default().fg(Color::Yellow));
          }
        }
      }
    }

    // last row
    for col in 0..=99 {
      expected
        .get_mut(col, 6)
        .set_style(Style::default().fg(Color::Yellow));
    }

    terminal.backend().assert_buffer(&expected);
  }
}
