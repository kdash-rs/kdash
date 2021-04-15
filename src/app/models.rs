use tui::widgets::TableState;

use crate::event::Key;

use super::ActiveBlock;

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
  jump_to_all_context: Key::Char('c'),
  jump_to_current_context: Key::Char('a'),
  up: Key::Up,
  down: Key::Down,
  left: Key::Left,
  right: Key::Right,
  toggle_info: Key::Char('i'),
  jump_to_namespace: Key::Char('n'),
  jump_to_pods: Key::Char('p'),
  jump_to_services: Key::Char('s'),
  jump_to_nodes: Key::Char('N'),
  jump_to_deployments: Key::Char('D'),
  jump_to_configmaps: Key::Char('C'),
  jump_to_statefulsets: Key::Char('S'),
  jump_to_replicasets: Key::Char('R'),
};

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
    if self.items.len() > 0 {
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

  pub fn unselect(&mut self) {
    self.state.select(None);
  }
}

impl<T: Clone> StatefulTable<T> {
  pub fn get_selected_item(&mut self) -> Option<T> {
    self
      .state
      .selected()
      .and_then(|i| Some(self.items[i].clone()))
  }
}

pub struct TabsState {
  pub titles: Vec<&'static str>,
  pub index: usize,
  pub active_block_ids: Option<Vec<ActiveBlock>>,
  pub active_block: Option<ActiveBlock>,
}

impl TabsState {
  pub fn new(titles: Vec<&'static str>) -> TabsState {
    TabsState {
      titles,
      index: 0,
      active_block_ids: None,
      active_block: None,
    }
  }
  pub fn with_active_blocks(titles: Vec<&'static str>, blocks: Vec<ActiveBlock>) -> TabsState {
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
    self.active_block = self
      .active_block_ids
      .as_ref()
      .and_then(|ids| Some(ids[self.index]));
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
