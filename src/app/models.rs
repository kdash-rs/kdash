use std::collections::VecDeque;

use super::ActiveBlock;
use serde::Serialize;
use tui::{
  backend::Backend,
  layout::Rect,
  style::Style,
  text::Span,
  widgets::{Block, List, ListItem, ListState, TableState},
  Frame,
};

/// generate YAML from the original kubernetes resource
pub trait ResourceToYaml<T: Serialize> {
  fn get_k8s_obj(&self) -> &T;

  fn resource_to_yaml(&self) -> String {
    match serde_yaml::to_string(&self.get_k8s_obj()) {
      Ok(yaml) => yaml,
      Err(_) => "".into(),
    }
  }
}

pub trait Scrollable {
  fn scroll_down(&mut self);
  fn scroll_up(&mut self);
}

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

  pub fn _unselect(&mut self) {
    self.state.select(None);
  }
}

impl<T> Scrollable for StatefulTable<T> {
  fn scroll_down(&mut self) {
    let i = self.state.selected().map_or(0, |i| {
      if i >= self.items.len().wrapping_sub(1) {
        0
      } else {
        i + 1
      }
    });
    self.state.select(Some(i));
  }

  fn scroll_up(&mut self) {
    let i = self.state.selected().map_or(0, |i| {
      if i == 0 {
        self.items.len().wrapping_sub(1)
      } else {
        i - 1
      }
    });
    self.state.select(Some(i));
  }
}

impl<T: Clone> StatefulTable<T> {
  /// a clone of the currently selected item.
  /// for mutable ref use state.selected() and fetch from items when needed
  pub fn get_selected_item_copy(&self) -> Option<T> {
    if !self.items.is_empty() {
      self.state.selected().map(|i| self.items[i].clone())
    } else {
      None
    }
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

pub struct ScrollableTxt {
  items: Vec<String>,
  pub offset: u16,
}

impl ScrollableTxt {
  pub fn new() -> ScrollableTxt {
    ScrollableTxt {
      items: vec![],
      offset: 0,
    }
  }

  pub fn with_string(item: String) -> ScrollableTxt {
    let items: Vec<&str> = item.split('\n').collect();
    let items: Vec<String> = items.iter().map(|it| it.to_string()).collect();
    ScrollableTxt { items, offset: 0 }
  }

  pub fn get_txt(&self) -> String {
    self.items.join("\n")
  }
}

impl Scrollable for ScrollableTxt {
  fn scroll_down(&mut self) {
    // scroll only if offset is less than total lines in text
    // we subtract 8 to keep the text in view. Its just an arbitrary number that works
    if self.offset < (self.items.len() - 8) as u16 {
      self.offset += 1;
    }
  }
  fn scroll_up(&mut self) {
    // scroll up and avoid going negative
    if self.offset > 0 {
      self.offset -= 1;
    }
  }
}

// TODO implement line buffer to avoid gathering too much data in memory
#[derive(Debug, Clone)]
pub struct LogsState {
  /// Stores the log messages to be displayed
  ///
  /// (original_message, (wrapped_message, wrapped_at_width))
  #[allow(clippy::type_complexity)]
  records: VecDeque<(String, Option<(Vec<ListItem<'static>>, u16)>)>,
  wrapped_length: usize,
  pub state: ListState,
  pub id: String,
}

impl LogsState {
  pub fn new(id: String) -> LogsState {
    LogsState {
      records: VecDeque::with_capacity(512),
      state: ListState::default(),
      wrapped_length: 0,
      id,
    }
  }

  /// get a plain text version of the logs
  pub fn get_plain_text(&self) -> String {
    self.records.iter().fold(String::new(), |mut acc, v| {
      acc.push('\n');
      acc.push_str(v.0.as_str());
      acc
    })
  }

  /// Render the current state as a list widget
  pub fn render_list<B: Backend>(
    &mut self,
    f: &mut Frame<B>,
    logs_area: Rect,
    block: Block,
    style: Style,
    follow: bool,
  ) {
    let available_lines = logs_area.height as usize;
    let logs_area_width = logs_area.width as usize;

    let num_records = self.records.len();
    // Keep track of the number of lines after wrapping so we can skip lines as
    // needed below
    let mut wrapped_lines_len = 0;

    let mut items = Vec::with_capacity(logs_area.height as usize);

    let lines_to_skip = if follow {
      self.unselect();
      num_records.saturating_sub(available_lines)
    } else {
      0
    };

    items.extend(
      self
        .records
        .iter_mut()
        // Only wrap the records we could potentially be displaying
        .skip(lines_to_skip)
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

    let lines_to_skip = if follow {
      wrapped_lines_len.saturating_sub(available_lines)
    } else {
      0
    };

    let items = items
      .into_iter()
      // Wrapping could have created more lines than what we can display;
      // skip them
      .skip(lines_to_skip)
      .collect::<Vec<_>>();

    self.wrapped_length = items.len();

    // TODO: All this is a workaround. we should be wrapping text with paragraph, but it currently
    // doesn't support wrapping and staying scrolled to the bottom
    //
    // see https://github.com/fdehau/tui-rs/issues/89
    let list = List::new(items).block(block);

    f.render_stateful_widget(list, logs_area, &mut self.state);
  }
  /// Add a record to be displayed
  pub fn add_record(&mut self, record: String) {
    self.records.push_back((record, None));
  }

  fn unselect(&mut self) {
    self.state.select(None);
  }
}

impl Scrollable for LogsState {
  fn scroll_down(&mut self) {
    let i = self.state.selected().map_or(0, |i| {
      if i >= self.wrapped_length.wrapping_sub(1) {
        i
      } else {
        i + 1
      }
    });
    self.state.select(Some(i));
  }

  fn scroll_up(&mut self) {
    let i = self
      .state
      .selected()
      .map_or(0, |i| if i != 0 { i - 1 } else { 0 });
    self.state.select(Some(i));
  }
}

#[cfg(test)]
mod tests {
  // TODO
}
