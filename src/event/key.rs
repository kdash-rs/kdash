// from https://github.com/Rigellute/spotify-tui
use std::fmt;
use std::str::FromStr;

use crossterm::event;

/// Represents an key.
#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub enum Key {
  /// Both Enter (or Return) and numpad Enter
  Enter,
  Tab,
  Backspace,
  Esc,
  /// Left arrow
  Left,
  /// Right arrow
  Right,
  /// Up arrow
  Up,
  /// Down arrow
  Down,
  /// Insert key
  Ins,
  /// Delete key
  Delete,
  /// Home key
  Home,
  /// End key
  End,
  /// Page Up key
  PageUp,
  /// Page Down key
  PageDown,
  /// F0 key
  F0,
  /// F1 key
  F1,
  /// F2 key
  F2,
  /// F3 key
  F3,
  /// F4 key
  F4,
  /// F5 key
  F5,
  /// F6 key
  F6,
  /// F7 key
  F7,
  /// F8 key
  F8,
  /// F9 key
  F9,
  /// F10 key
  F10,
  /// F11 key
  F11,
  /// F12 key
  F12,
  Char(char),
  Ctrl(char),
  Alt(char),
  Shift(char),
  Unknown,
}

impl Key {
  /// Returns the function key corresponding to the given number
  ///
  /// 1 -> F1, etc...
  ///
  /// # Panics
  ///
  /// If `n == 0 || n > 12`
  pub fn from_f(n: u8) -> Key {
    match n {
      0 => Key::F0,
      1 => Key::F1,
      2 => Key::F2,
      3 => Key::F3,
      4 => Key::F4,
      5 => Key::F5,
      6 => Key::F6,
      7 => Key::F7,
      8 => Key::F8,
      9 => Key::F9,
      10 => Key::F10,
      11 => Key::F11,
      12 => Key::F12,
      _ => panic!("unknown function key: F{}", n),
    }
  }
}

impl fmt::Display for Key {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match *self {
      Key::Alt(' ') => write!(f, "<Alt+Space>"),
      Key::Ctrl(' ') => write!(f, "<Ctrl+Space>"),
      Key::Shift(' ') => write!(f, "<Shift+Space>"),
      Key::Char(' ') => write!(f, "<Space>"),
      Key::Alt(c) => write!(f, "<Alt+{}>", c),
      Key::Ctrl(c) => write!(f, "<Ctrl+{}>", c),
      Key::Shift(c) if c.is_ascii_alphabetic() => write!(f, "<{}>", c.to_ascii_uppercase()),
      Key::Shift(c) => write!(f, "<Shift+{}>", c),
      Key::Char(c) => write!(f, "<{}>", c),
      Key::Left => write!(f, "<←>"),
      Key::Right => write!(f, "<→>"),
      Key::Up => write!(f, "<↑>"),
      Key::Down => write!(f, "<↓>"),
      _ => write!(f, "<{:?}>", self),
    }
  }
}

impl FromStr for Key {
  type Err = String;

  fn from_str(input: &str) -> Result<Self, Self::Err> {
    let normalized = input.trim().trim_matches(['<', '>']);
    if normalized.is_empty() {
      return Err("key cannot be empty".into());
    }

    if normalized.chars().count() == 1 {
      let c = normalized
        .chars()
        .next()
        .expect("single-character string must have one char");
      return if c.is_ascii_uppercase() {
        Ok(Key::Shift(c.to_ascii_lowercase()))
      } else {
        Ok(Key::Char(c))
      };
    }

    let lower = normalized.to_lowercase();
    let key = match lower.as_str() {
      "enter" | "return" => Key::Enter,
      "tab" => Key::Tab,
      "backspace" => Key::Backspace,
      "esc" | "escape" => Key::Esc,
      "left" | "leftarrow" | "left-arrow" => Key::Left,
      "right" | "rightarrow" | "right-arrow" => Key::Right,
      "up" | "uparrow" | "up-arrow" => Key::Up,
      "down" | "downarrow" | "down-arrow" => Key::Down,
      "insert" | "ins" => Key::Ins,
      "delete" | "del" => Key::Delete,
      "home" => Key::Home,
      "end" => Key::End,
      "pageup" | "page-up" => Key::PageUp,
      "pagedown" | "page-down" => Key::PageDown,
      "space" => Key::Char(' '),
      _ => {
        if let Some(rest) = lower.strip_prefix("ctrl+") {
          return parse_modified_char(rest, Key::Ctrl);
        }
        if let Some(rest) = lower.strip_prefix("alt+") {
          return parse_modified_char(rest, Key::Alt);
        }
        if let Some(rest) = lower.strip_prefix("shift+") {
          return parse_modified_char(rest, Key::Shift);
        }
        if let Some(rest) = lower.strip_prefix('f') {
          let value = rest
            .parse::<u8>()
            .map_err(|_| format!("unsupported key '{}'", input))?;
          if value <= 12 {
            return Ok(Key::from_f(value));
          }
        }
        return Err(format!("unsupported key '{}'", input));
      }
    };

    Ok(key)
  }
}

fn parse_modified_char(input: &str, wrap: fn(char) -> Key) -> Result<Key, String> {
  let value = if input == "space" { " " } else { input };
  if value.chars().count() != 1 {
    return Err(format!(
      "modified key '{}' must target a single character",
      input
    ));
  }

  Ok(wrap(
    value
      .chars()
      .next()
      .expect("single-character string must have one char"),
  ))
}

impl From<event::KeyEvent> for Key {
  fn from(key_event: event::KeyEvent) -> Self {
    match key_event {
      event::KeyEvent {
        code: event::KeyCode::Esc,
        ..
      } => Key::Esc,
      event::KeyEvent {
        code: event::KeyCode::Backspace,
        ..
      } => Key::Backspace,
      event::KeyEvent {
        code: event::KeyCode::Left,
        ..
      } => Key::Left,
      event::KeyEvent {
        code: event::KeyCode::Right,
        ..
      } => Key::Right,
      event::KeyEvent {
        code: event::KeyCode::Up,
        ..
      } => Key::Up,
      event::KeyEvent {
        code: event::KeyCode::Down,
        ..
      } => Key::Down,
      event::KeyEvent {
        code: event::KeyCode::Home,
        ..
      } => Key::Home,
      event::KeyEvent {
        code: event::KeyCode::End,
        ..
      } => Key::End,
      event::KeyEvent {
        code: event::KeyCode::PageUp,
        ..
      } => Key::PageUp,
      event::KeyEvent {
        code: event::KeyCode::PageDown,
        ..
      } => Key::PageDown,
      event::KeyEvent {
        code: event::KeyCode::Delete,
        ..
      } => Key::Delete,
      event::KeyEvent {
        code: event::KeyCode::Insert,
        ..
      } => Key::Ins,
      event::KeyEvent {
        code: event::KeyCode::F(n),
        ..
      } => Key::from_f(n),
      event::KeyEvent {
        code: event::KeyCode::Enter,
        ..
      } => Key::Enter,
      event::KeyEvent {
        code: event::KeyCode::Tab,
        ..
      } => Key::Tab,
      event::KeyEvent {
        code: event::KeyCode::Char(c),
        modifiers,
        ..
      } => {
        let normalized = if c.is_ascii_uppercase() {
          c.to_ascii_lowercase()
        } else {
          c
        };

        if modifiers.contains(event::KeyModifiers::CONTROL) {
          Key::Ctrl(normalized)
        } else if modifiers.contains(event::KeyModifiers::ALT) {
          Key::Alt(normalized)
        } else if modifiers.contains(event::KeyModifiers::SHIFT) {
          Key::Shift(normalized)
        } else {
          Key::Char(c)
        }
      }

      _ => Key::Unknown,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_key_fmt() {
    assert_eq!(format!("{}", Key::Left), "<←>");
    assert_eq!(format!("{}", Key::Right), "<→>");
    assert_eq!(format!("{}", Key::Up), "<↑>");
    assert_eq!(format!("{}", Key::Down), "<↓>");
    assert_eq!(format!("{}", Key::Alt(' ')), "<Alt+Space>");
    assert_eq!(format!("{}", Key::Shift(' ')), "<Shift+Space>");
    assert_eq!(format!("{}", Key::Alt('c')), "<Alt+c>");
    assert_eq!(format!("{}", Key::Shift('d')), "<D>");
    assert_eq!(format!("{}", Key::Char('c')), "<c>");
    assert_eq!(format!("{}", Key::Enter), "<Enter>");
    assert_eq!(format!("{}", Key::F10), "<F10>");
  }
  #[test]
  fn test_key_from_event() {
    assert_eq!(
      Key::from(event::KeyEvent::from(event::KeyCode::Esc)),
      Key::Esc
    );
    assert_eq!(
      Key::from(event::KeyEvent::from(event::KeyCode::F(2))),
      Key::F2
    );
    assert_eq!(
      Key::from(event::KeyEvent::from(event::KeyCode::Char('J'))),
      Key::Char('J')
    );
    assert_eq!(
      Key::from(event::KeyEvent {
        code: event::KeyCode::Char('D'),
        modifiers: event::KeyModifiers::SHIFT,
        kind: event::KeyEventKind::Press,
        state: event::KeyEventState::NONE,
      }),
      Key::Shift('d')
    );
    assert_eq!(
      Key::from(event::KeyEvent {
        code: event::KeyCode::Char('c'),
        modifiers: event::KeyModifiers::ALT,
        kind: event::KeyEventKind::Press,
        state: event::KeyEventState::NONE,
      }),
      Key::Alt('c')
    );
    assert_eq!(
      Key::from(event::KeyEvent {
        code: event::KeyCode::Char('c'),
        modifiers: event::KeyModifiers::CONTROL,
        kind: event::KeyEventKind::Press,
        state: event::KeyEventState::NONE
      }),
      Key::Ctrl('c')
    );
  }

  #[test]
  fn test_key_from_str() {
    assert_eq!("q".parse::<Key>(), Ok(Key::Char('q')));
    assert_eq!("ctrl+q".parse::<Key>(), Ok(Key::Ctrl('q')));
    assert_eq!("alt+x".parse::<Key>(), Ok(Key::Alt('x')));
    assert_eq!("D".parse::<Key>(), Ok(Key::Shift('d')));
    assert_eq!("shift+d".parse::<Key>(), Ok(Key::Shift('d')));
    assert_eq!("space".parse::<Key>(), Ok(Key::Char(' ')));
    assert_eq!("page-down".parse::<Key>(), Ok(Key::PageDown));
    assert_eq!("F10".parse::<Key>(), Ok(Key::F10));
    assert!("shift+tab".parse::<Key>().is_err());
  }

  #[test]
  fn test_uppercase_and_shift_string_parse_equally() {
    assert_eq!("D".parse::<Key>(), "shift+d".parse::<Key>());
  }
}
