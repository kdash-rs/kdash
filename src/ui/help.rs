use ratatui::{
  layout::{Constraint, Direction, Layout, Rect},
  style::Modifier,
  text::{Line, Span},
  widgets::{Block, Borders, Padding, Paragraph, Wrap},
  Frame,
};

use super::utils::{
  help_part, key_hints, mixed_bold_line, style_label, style_primary, style_secondary, style_text,
  title_with_dual_style,
};
use crate::app::{
  key_binding::{get_help_sections, HContext, HelpSection, DEFAULT_KEYBINDING},
  App,
};
use crate::ui::theme::Palette;

/// Full-page help: keybindings grouped by context into two columns,
/// scrollable with up/down. Layout is derived entirely from the keymap.
pub fn draw_help(f: &mut Frame<'_>, app: &mut App, area: Rect) {
  let palette = app.palette;
  let sections = get_help_sections();

  let hint = mixed_bold_line(
    [help_part(format!(
      "{}:scroll · {}:back ",
      key_hints(&[DEFAULT_KEYBINDING.up.key, DEFAULT_KEYBINDING.down.key]),
      DEFAULT_KEYBINDING.esc.key.symbol(),
    ))],
    palette,
  );
  let block = Block::default()
    .borders(Borders::ALL)
    .border_style(style_secondary(palette))
    .title(title_with_dual_style(" Help ".to_string(), hint, palette))
    .padding(Padding::new(2, 2, 1, 1));
  let inner = block.inner(area);
  f.render_widget(block, area);

  if inner.width == 0 || inner.height == 0 {
    return;
  }

  let n_cols = if inner.width >= 70 { 2 } else { 1 };
  let columns = columns_for(&sections, n_cols);

  // Clamp the scroll offset to the tallest column (estimated; wrapped
  // descriptions may add a little, which the common full-height page absorbs).
  let tallest = columns
    .iter()
    .map(|col| col.iter().copied().map(section_height).sum::<usize>())
    .max()
    .unwrap_or(0) as u16;
  app.help_scroll = app.help_scroll.min(tallest.saturating_sub(inner.height));
  let scroll_y = app.help_scroll;

  let constraints: Vec<Constraint> = (0..n_cols)
    .map(|_| Constraint::Ratio(1, n_cols as u32))
    .collect();
  let cols = Layout::default()
    .direction(Direction::Horizontal)
    .constraints(constraints)
    .split(inner);

  for (i, col_sections) in columns.iter().enumerate() {
    f.render_widget(
      Paragraph::new(render_column(col_sections, palette))
        .wrap(Wrap { trim: false })
        .scroll((scroll_y, 0)),
      cols[i],
    );
  }
}

/// Rows a section occupies: title + one per binding + a trailing blank.
fn section_height(section: &HelpSection) -> usize {
  section.rows.len() + 2
}

/// Split sections across columns. With two columns the Resource Views group
/// (by far the largest) stands alone on the right, while General and
/// Utilization stack together on the left in display order. A single column
/// keeps everything in order.
fn columns_for(sections: &[HelpSection], n_cols: usize) -> Vec<Vec<&HelpSection>> {
  if n_cols <= 1 {
    return vec![sections.iter().collect()];
  }
  let mut left = Vec::new();
  let mut right = Vec::new();
  for section in sections {
    if section.context == HContext::Overview {
      right.push(section);
    } else {
      left.push(section);
    }
  }
  vec![left, right]
}

/// Render a column's sections to styled lines: accent-bold heading, then
/// `keys` (blue) + description (text) per binding.
fn render_column(sections: &[&HelpSection], palette: Palette) -> Vec<Line<'static>> {
  let mut out: Vec<Line<'static>> = Vec::new();
  for section in sections {
    out.push(Line::from(Span::styled(
      section.title.to_string(),
      style_primary(palette).add_modifier(Modifier::BOLD),
    )));
    for (keys, desc) in &section.rows {
      out.push(Line::from(vec![
        Span::styled(format!("  {:<10} ", keys), style_label(palette)),
        Span::styled(desc.clone(), style_text(palette)),
      ]));
    }
    out.push(Line::default());
  }
  out
}

#[cfg(test)]
mod tests {
  use ratatui::{backend::TestBackend, Terminal};

  use super::*;
  use crate::ui::theme::{palette_for, ThemeName};

  fn render(width: u16, height: u16) -> (Vec<String>, ratatui::buffer::Buffer) {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
      .draw(|f| {
        let mut app = App::default();
        draw_help(f, &mut app, f.area());
      })
      .unwrap();
    let buffer = terminal.backend().buffer().clone();
    let lines: Vec<String> = (0..buffer.area.height)
      .map(|row| {
        (0..buffer.area.width)
          .map(|col| buffer[(col, row)].symbol())
          .collect::<String>()
      })
      .collect();
    (lines, buffer)
  }

  #[test]
  fn test_draw_help_renders_grouped_sections() {
    let (lines, _) = render(160, 40);
    let joined = lines.join("\n");

    // Panel title + the three context group headings.
    assert!(joined.contains("Help"));
    assert!(joined.contains("General"));
    assert!(joined.contains("Resource Views"));
    assert!(joined.contains("Utilization"));

    // A representative binding with its glyph keys and description.
    assert!(joined.contains("Cycle through main views"));
    assert!(joined.contains("Ctrl+c,q"));
    // Scroll/back hint in the title.
    assert!(joined.contains("Esc:back"));
  }

  /// First column at which `title` starts, scanning rows top-to-bottom.
  fn title_column(lines: &[String], title: &str) -> Option<usize> {
    lines.iter().find_map(|line| line.find(title))
  }

  #[test]
  fn test_help_two_columns_stack_utilization_under_general() {
    let (lines, _) = render(160, 40);

    let general_x = title_column(&lines, "General").expect("General heading");
    let util_x = title_column(&lines, "Utilization").expect("Utilization heading");
    let resources_x = title_column(&lines, "Resource Views").expect("Resource Views heading");

    // General and Utilization share the left column; Resource Views is the
    // sole occupant of the right column.
    assert_eq!(general_x, util_x);
    assert!(util_x < resources_x);
  }

  #[test]
  fn test_draw_help_colours() {
    let (_, buffer) = render(160, 40);
    let p = palette_for(ThemeName::Macchiato);

    // Border + " Help " title → secondary.
    assert_eq!(buffer[(0, 0)].fg, p.secondary);
    assert_eq!(buffer[(2, 0)].fg, p.secondary);
    assert!(buffer[(2, 0)].modifier.contains(Modifier::BOLD));
  }
}
