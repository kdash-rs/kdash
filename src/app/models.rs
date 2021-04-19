use std::collections::VecDeque;

use super::super::event::Key;
use super::ActiveBlock;
use tui::{
  layout::Rect,
  style::Style,
  text::Span,
  widgets::{List, ListItem, TableState},
};

#[derive(Clone)]
pub struct KeyBindings {
  pub esc: Key,
  pub quit: Key,
  pub help: Key,
  pub submit: Key,
  pub refresh: Key,
  pub toggle_theme: Key,
  pub jump_to_all_context: Key,
  pub jump_to_current_context: Key,
  pub up: Key,
  pub down: Key,
  pub left: Key,
  pub right: Key,
  pub toggle_info: Key,
  pub select_all_namespace: Key,
  pub jump_to_namespace: Key,
  pub jump_to_pods: Key,
  pub jump_to_services: Key,
  pub jump_to_nodes: Key,
  pub jump_to_deployments: Key,
  pub jump_to_configmaps: Key,
  pub jump_to_statefulsets: Key,
  pub jump_to_replicasets: Key,
}

pub const DEFAULT_KEYBINDING: KeyBindings = KeyBindings {
  esc: Key::Esc,
  quit: Key::Char('q'),
  help: Key::Char('?'),
  submit: Key::Enter,
  refresh: Key::Ctrl('r'),
  toggle_theme: Key::Char('t'),
  jump_to_all_context: Key::Char('C'),
  jump_to_current_context: Key::Char('A'),
  up: Key::Up,
  down: Key::Down,
  left: Key::Left,
  right: Key::Right,
  toggle_info: Key::Char('i'),
  select_all_namespace: Key::Char('a'),
  jump_to_namespace: Key::Char('n'),
  jump_to_pods: Key::Char('p'),
  jump_to_services: Key::Char('s'),
  jump_to_nodes: Key::Char('N'),
  jump_to_deployments: Key::Char('d'),
  jump_to_configmaps: Key::Char('c'),
  jump_to_statefulsets: Key::Char('S'),
  jump_to_replicasets: Key::Char('r'),
};

#[derive(Clone)]
pub struct StatefulTable<T> {
  pub state: TableState,
  pub items: Vec<T>,
}

impl<T> StatefulTable<T> {
  pub fn new() -> StatefulTable<T> {
    StatefulTable {
      state: TableState::default(),
      items: Vec::new(),
    }
  }

  pub fn with_items(items: Vec<T>) -> StatefulTable<T> {
    let mut sft = StatefulTable::new();
    sft.set_items(items);
    sft
  }

  pub fn set_items(&mut self, items: Vec<T>) {
    self.items = items;
    if !self.items.is_empty() {
      let i = self
        .state
        .selected()
        .map_or(0, |i| if i > 0 { i } else { 0 });
      self.state.select(Some(i));
    }
  }

  pub fn next(&mut self) {
    let i = self.state.selected().map_or(0, |i| {
      if i >= self.items.len().wrapping_sub(1) {
        0
      } else {
        i + 1
      }
    });
    self.state.select(Some(i));
  }

  pub fn previous(&mut self) {
    let i = self.state.selected().map_or(0, |i| {
      if i == 0 {
        self.items.len().wrapping_sub(1)
      } else {
        i - 1
      }
    });
    self.state.select(Some(i));
  }

  pub fn _unselect(&mut self) {
    self.state.select(None);
  }
}

impl<T: Clone> StatefulTable<T> {
  pub fn get_selected_item(&mut self) -> Option<T> {
    self.state.selected().map(|i| self.items[i].clone())
  }
}

pub struct TabsState {
  pub titles: Vec<String>,
  pub index: usize,
  pub active_block_ids: Option<Vec<ActiveBlock>>,
  pub active_block: Option<ActiveBlock>,
}

impl TabsState {
  pub fn new(titles: Vec<String>) -> TabsState {
    TabsState {
      titles,
      index: 0,
      active_block_ids: None,
      active_block: None,
    }
  }
  pub fn with_active_blocks(titles: Vec<String>, blocks: Vec<ActiveBlock>) -> TabsState {
    TabsState {
      titles,
      index: 0,
      active_block: Some(blocks[0]),
      active_block_ids: Some(blocks),
    }
  }
  pub fn set_index(&mut self, index: usize) {
    self.index = index;
    self.set_active();
  }
  pub fn set_active(&mut self) {
    self.active_block = self.active_block_ids.as_ref().map(|ids| ids[self.index]);
  }
  pub fn next(&mut self) {
    self.index = (self.index + 1) % self.titles.len();
    self.set_active();
  }
  pub fn previous(&mut self) {
    if self.index > 0 {
      self.index -= 1;
    } else {
      self.index = self.titles.len() - 1;
    }
    self.set_active();
  }
}

// TODO implement line buffer to avoid gathering too much data in memory
#[derive(Debug, Clone)]
#[allow(clippy::type_complexity)]
pub struct LogsState {
  /// Stores the log messages to be displayed
  ///
  /// (original_message, (wrapped_message, wrapped_at_width))
  records: VecDeque<(String, Option<(Vec<ListItem<'static>>, u16)>)>,
}

impl LogsState {
  pub fn new() -> LogsState {
    LogsState {
      records: VecDeque::with_capacity(512),
    }
  }
  /// Get the current state as a list widget
  pub fn get_list(&mut self, logs_area: Rect, style: Style) -> List {
    let available_lines = logs_area.height as usize;
    let logs_area_width = logs_area.width as usize;

    let num_records = self.records.len();
    // Keep track of the number of lines after wrapping so we can skip lines as
    // needed below
    let mut wrapped_lines_len = 0;

    let mut items = Vec::with_capacity(logs_area.height as usize);

    items.extend(
      self
        .records
        .iter_mut()
        // Only wrap the records we could potentially be displaying
        .skip(num_records.saturating_sub(available_lines))
        .map(|r| {
          // See if we can use a cached wrapped line
          if let Some(wrapped) = &r.1 {
            if wrapped.1 as usize == logs_area_width {
              wrapped_lines_len += wrapped.0.len();
              return wrapped.0.clone();
            }
          }

          // If not, wrap the line and cache it
          r.1 = Some((
            textwrap::wrap(r.0.as_ref(), logs_area_width)
              .into_iter()
              .map(|s| s.to_string())
              .map(|c| Span::styled(c, style))
              .map(ListItem::new)
              .collect::<Vec<ListItem>>(),
            logs_area.width,
          ));

          wrapped_lines_len += r.1.as_ref().unwrap().0.len();
          r.1.as_ref().unwrap().0.clone()
        })
        .flatten(),
    );

    // TODO: we should be wrapping text with paragraph, but it currently
    // doesn't support wrapping and staying scrolled to the bottom
    //
    // see https://github.com/fdehau/tui-rs/issues/89
    List::new(
      items
        .into_iter()
        // Wrapping could have created more lines than what we can display;
        // skip them
        .skip(wrapped_lines_len.saturating_sub(available_lines))
        .collect::<Vec<_>>(),
    )
  }
  /// Add a record to be displayed
  pub fn add_record(&mut self, record: String) {
    self.records.push_back((record, None));
  }
}
