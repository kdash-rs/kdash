use std::collections::VecDeque;

use async_trait::async_trait;
use ratatui::{
  layout::Rect,
  style::{Modifier, Style},
  text::Span,
  widgets::{Block, List, ListItem, ListState, TableState},
  Frame,
};
use serde::Serialize;

use super::{ActiveBlock, App, Route};
use crate::network::Network;

#[async_trait]
pub trait AppResource {
  fn render(block: ActiveBlock, f: &mut Frame<'_>, app: &mut App, area: Rect);

  async fn get_resource(network: &Network<'_>);
}
pub trait KubeResource<T: Serialize> {
  fn get_name(&self) -> &String;

  fn get_k8s_obj(&self) -> &T;

  /// generate YAML from the original kubernetes resource
  fn resource_to_yaml(&self) -> String {
    match serde_yaml::to_string(&self.get_k8s_obj()) {
      Ok(yaml) => yaml,
      Err(_) => "".into(),
    }
  }
}

pub trait Scrollable {
  fn handle_scroll(&mut self, up: bool, page: bool) {
    // support page up/down
    let inc_or_dec = if page { 10 } else { 1 };
    if up {
      self.scroll_up(inc_or_dec);
    } else {
      self.scroll_down(inc_or_dec);
    }
  }
  fn scroll_down(&mut self, inc_or_dec: usize);
  fn scroll_up(&mut self, inc_or_dec: usize);
}

pub struct StatefulList<T> {
  pub state: ListState,
  pub items: Vec<T>,
}

impl<T> StatefulList<T> {
  pub fn new() -> StatefulList<T> {
    StatefulList {
      state: ListState::default(),
      items: Vec::new(),
    }
  }
  pub fn with_items(items: Vec<T>) -> StatefulList<T> {
    let mut state = ListState::default();
    if !items.is_empty() {
      state.select(Some(0));
    }
    StatefulList { state, items }
  }
}

impl<T> Scrollable for StatefulList<T> {
  // for lists we cycle back to the beginning when we reach the end
  fn scroll_down(&mut self, increment: usize) {
    let i = match self.state.selected() {
      Some(i) => {
        if i >= self.items.len().saturating_sub(increment) {
          0
        } else {
          i + increment
        }
      }
      None => 0,
    };
    self.state.select(Some(i));
  }
  // for lists we cycle back to the end when we reach the beginning
  fn scroll_up(&mut self, decrement: usize) {
    let i = match self.state.selected() {
      Some(i) => {
        if i == 0 {
          self.items.len().saturating_sub(decrement)
        } else {
          i.saturating_sub(decrement)
        }
      }
      None => 0,
    };
    self.state.select(Some(i));
  }
}

#[derive(Clone, Debug)]
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
    let mut table = StatefulTable::new();
    if !items.is_empty() {
      table.state.select(Some(0));
    }
    table.set_items(items);
    table
  }

  pub fn set_items(&mut self, items: Vec<T>) {
    let item_len = items.len();
    self.items = items;
    if !self.items.is_empty() {
      let i = self.state.selected().map_or(0, |i| {
        if i > 0 && i < item_len {
          i
        } else if i >= item_len {
          item_len - 1
        } else {
          0
        }
      });
      self.state.select(Some(i));
    }
  }
}

impl<T> Scrollable for StatefulTable<T> {
  fn scroll_down(&mut self, increment: usize) {
    if let Some(i) = self.state.selected() {
      if (i + increment) < self.items.len() {
        self.state.select(Some(i + increment));
      } else {
        self.state.select(Some(self.items.len().saturating_sub(1)));
      }
    }
  }

  fn scroll_up(&mut self, decrement: usize) {
    if let Some(i) = self.state.selected() {
      if i != 0 {
        self.state.select(Some(i.saturating_sub(decrement)));
      }
    }
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

#[derive(Clone)]
pub struct TabRoute {
  pub title: String,
  pub route: Route,
}

pub struct TabsState {
  pub items: Vec<TabRoute>,
  pub index: usize,
}

impl TabsState {
  pub fn new(items: Vec<TabRoute>) -> TabsState {
    TabsState { items, index: 0 }
  }
  pub fn set_index(&mut self, index: usize) -> &TabRoute {
    self.index = index;
    &self.items[self.index]
  }
  pub fn get_active_route(&self) -> &Route {
    &self.items[self.index].route
  }

  pub fn next(&mut self) {
    self.index = (self.index + 1) % self.items.len();
  }
  pub fn previous(&mut self) {
    if self.index > 0 {
      self.index -= 1;
    } else {
      self.index = self.items.len() - 1;
    }
  }
}

#[derive(Debug, Eq, PartialEq)]
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
  fn scroll_down(&mut self, increment: usize) {
    // scroll only if offset is less than total lines in text
    // we subtract increment + 2 to keep the text in view. Its just an arbitrary number that works
    if self.offset < self.items.len().saturating_sub(increment + 2) as u16 {
      self.offset += increment as u16;
    }
  }
  fn scroll_up(&mut self, decrement: usize) {
    // scroll up and avoid going negative
    if self.offset > 0 {
      self.offset = self.offset.saturating_sub(decrement as u16);
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
  pub fn render_list(
    &mut self,
    f: &mut Frame<'_>,
    logs_area: Rect,
    block: Block<'_>,
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
        .flat_map(|r| {
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
              .collect::<Vec<ListItem<'_>>>(),
            logs_area.width,
          ));

          wrapped_lines_len += r.1.as_ref().unwrap().0.len();
          r.1.as_ref().unwrap().0.clone()
        }),
    );

    let wrapped_lines_to_skip = if follow {
      wrapped_lines_len.saturating_sub(available_lines)
    } else {
      0
    };

    let items = items
      .into_iter()
      // Wrapping could have created more lines than what we can display;
      // skip them
      .skip(wrapped_lines_to_skip)
      .collect::<Vec<_>>();

    self.wrapped_length = items.len();

    // TODO: All this is a workaround. we should be wrapping text with paragraph, but it currently
    // doesn't support wrapping and staying scrolled to the bottom
    //
    // see https://github.com/fdehau/tui-rs/issues/89
    let list = List::new(items)
      .block(block)
      .highlight_style(Style::default().add_modifier(Modifier::BOLD));

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
  fn scroll_down(&mut self, increment: usize) {
    let i = self.state.selected().map_or(0, |i| {
      if i >= self.wrapped_length.saturating_sub(increment) {
        i
      } else {
        i + increment
      }
    });
    self.state.select(Some(i));
  }

  fn scroll_up(&mut self, decrement: usize) {
    let i = self.state.selected().map_or(0, |i| {
      if i != 0 {
        i.saturating_sub(decrement)
      } else {
        0
      }
    });
    self.state.select(Some(i));
  }
}

#[cfg(test)]
mod tests {
  use k8s_openapi::api::core::v1::Namespace;
  use kube::api::ObjectMeta;
  use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

  use super::*;
  use crate::app::{ns::KubeNs, ActiveBlock, RouteId};

  #[test]
  fn test_kube_resource() {
    struct TestStruct {
      name: String,
      k8s_obj: Namespace,
    }
    impl KubeResource<Namespace> for TestStruct {
      fn get_name(&self) -> &String {
        &self.name
      }
      fn get_k8s_obj(&self) -> &Namespace {
        &self.k8s_obj
      }
    }
    let ts = TestStruct {
      name: "test".into(),
      k8s_obj: Namespace {
        metadata: ObjectMeta {
          name: Some("test".into()),
          namespace: Some("test".into()),
          ..ObjectMeta::default()
        },
        ..Namespace::default()
      },
    };
    assert_eq!(
      ts.resource_to_yaml(),
      "apiVersion: v1\nkind: Namespace\nmetadata:\n  name: test\n  namespace: test\n"
    )
  }

  #[test]
  fn test_stateful_table() {
    let mut sft: StatefulTable<KubeNs> = StatefulTable::new();

    assert_eq!(sft.items.len(), 0);
    assert_eq!(sft.state.selected(), None);
    // check default selection on set
    sft.set_items(vec![KubeNs::default(), KubeNs::default()]);
    assert_eq!(sft.items.len(), 2);
    assert_eq!(sft.state.selected(), Some(0));
    // check selection retain on set
    sft.state.select(Some(1));
    sft.set_items(vec![
      KubeNs::default(),
      KubeNs::default(),
      KubeNs::default(),
    ]);
    assert_eq!(sft.items.len(), 3);
    assert_eq!(sft.state.selected(), Some(1));
    // check selection overflow prevention
    sft.state.select(Some(2));
    sft.set_items(vec![KubeNs::default(), KubeNs::default()]);
    assert_eq!(sft.items.len(), 2);
    assert_eq!(sft.state.selected(), Some(1));
    // check scroll down
    sft.state.select(Some(0));
    assert_eq!(sft.state.selected(), Some(0));
    sft.scroll_down(1);
    assert_eq!(sft.state.selected(), Some(1));
    // check scroll overflow
    sft.scroll_down(1);
    assert_eq!(sft.state.selected(), Some(1));
    sft.scroll_up(1);
    assert_eq!(sft.state.selected(), Some(0));
    // check scroll overflow
    sft.scroll_up(1);
    assert_eq!(sft.state.selected(), Some(0));
    // check increment
    sft.scroll_down(10);
    assert_eq!(sft.state.selected(), Some(1));

    let sft2 = StatefulTable::with_items(vec![KubeNs::default(), KubeNs::default()]);
    assert_eq!(sft2.state.selected(), Some(0));
  }

  #[test]
  fn test_handle_table_scroll() {
    let mut item: StatefulTable<&str> = StatefulTable::new();
    item.set_items(vec!["A", "B", "C"]);

    assert_eq!(item.state.selected(), Some(0));

    item.handle_scroll(false, false);
    assert_eq!(item.state.selected(), Some(1));

    item.handle_scroll(false, false);
    assert_eq!(item.state.selected(), Some(2));

    item.handle_scroll(false, false);
    assert_eq!(item.state.selected(), Some(2));
    // previous
    item.handle_scroll(true, false);
    assert_eq!(item.state.selected(), Some(1));
    // page down
    item.handle_scroll(false, true);
    assert_eq!(item.state.selected(), Some(2));
    // page up
    item.handle_scroll(true, true);
    assert_eq!(item.state.selected(), Some(0));
  }

  #[test]
  fn test_stateful_tab() {
    let mut tab = TabsState::new(vec![
      TabRoute {
        title: "Hello".into(),
        route: Route {
          active_block: ActiveBlock::Pods,
          id: RouteId::Home,
        },
      },
      TabRoute {
        title: "Test".into(),
        route: Route {
          active_block: ActiveBlock::Nodes,
          id: RouteId::Home,
        },
      },
    ]);

    assert_eq!(tab.index, 0);
    assert_eq!(tab.get_active_route().active_block, ActiveBlock::Pods);
    tab.next();
    assert_eq!(tab.index, 1);
    assert_eq!(tab.get_active_route().active_block, ActiveBlock::Nodes);
    tab.next();
    assert_eq!(tab.index, 0);
    assert_eq!(tab.get_active_route().active_block, ActiveBlock::Pods);
    tab.previous();
    assert_eq!(tab.index, 1);
    assert_eq!(tab.get_active_route().active_block, ActiveBlock::Nodes);
    tab.previous();
    assert_eq!(tab.index, 0);
    assert_eq!(tab.get_active_route().active_block, ActiveBlock::Pods);
  }

  #[test]
  fn test_scrollable_txt() {
    let mut stxt = ScrollableTxt::with_string("test\n multiline\n string".into());

    assert_eq!(stxt.offset, 0);
    assert_eq!(stxt.items.len(), 3);

    assert_eq!(stxt.get_txt(), "test\n multiline\n string");

    stxt.scroll_down(1);
    assert_eq!(stxt.offset, 0);

    let mut stxt2 = ScrollableTxt::with_string("te\nst\nmul\ntil\ni\nne\nstr\ni\nn\ng".into());
    assert_eq!(stxt2.items.len(), 10);
    stxt2.scroll_down(1);
    assert_eq!(stxt2.offset, 1);
    stxt2.scroll_down(1);
    assert_eq!(stxt2.offset, 2);
    stxt2.scroll_down(5);
    assert_eq!(stxt2.offset, 7);
    stxt2.scroll_down(1);
    // no overflow past (len - 2)
    assert_eq!(stxt2.offset, 7);
    stxt2.scroll_up(1);
    assert_eq!(stxt2.offset, 6);
    stxt2.scroll_up(6);
    assert_eq!(stxt2.offset, 0);
    stxt2.scroll_up(1);
    // no overflow past (0)
    assert_eq!(stxt2.offset, 0);
  }

  #[test]
  fn test_logs_state() {
    let mut log = LogsState::new("1".into());
    log.add_record("record 1".into());
    log.add_record("record 2".into());

    assert_eq!(log.get_plain_text(), "\nrecord 1\nrecord 2");

    let backend = TestBackend::new(20, 7);
    let mut terminal = Terminal::new(backend).unwrap();

    log.add_record("record 4 should be long enough to be wrapped".into());
    log.add_record("record 5".into());
    log.add_record("record 6".into());
    log.add_record("record 7".into());
    log.add_record("record 8".into());

    terminal
      .draw(|f| log.render_list(f, f.size(), Block::default(), Style::default(), true))
      .unwrap();

    let expected = Buffer::with_lines(vec![
      "record 4 should be  ",
      "long enough to be   ",
      "wrapped             ",
      "record 5            ",
      "record 6            ",
      "record 7            ",
      "record 8            ",
    ]);

    terminal.backend().assert_buffer(&expected);

    terminal
      .draw(|f| log.render_list(f, f.size(), Block::default(), Style::default(), false))
      .unwrap();

    let expected2 = Buffer::with_lines(vec![
      "record 1            ",
      "record 2            ",
      "record 4 should be  ",
      "long enough to be   ",
      "wrapped             ",
      "record 5            ",
      "record 6            ",
    ]);

    terminal.backend().assert_buffer(&expected2);

    log.add_record("record 9".into());
    log.add_record("record 10 which is again looooooooooooooooooooooooooooooonnnng".into());
    log.add_record("record 11".into());
    // enabling follow should scroll back to bottom
    terminal
      .draw(|f| log.render_list(f, f.size(), Block::default(), Style::default(), true))
      .unwrap();

    let expected3 = Buffer::with_lines(vec![
      "record 8            ",
      "record 9            ",
      "record 10           ",
      "which is again      ",
      "looooooooooooooooooo",
      "oooooooooooonnnng   ",
      "record 11           ",
    ]);

    terminal.backend().assert_buffer(&expected3);

    terminal
      .draw(|f| log.render_list(f, f.size(), Block::default(), Style::default(), false))
      .unwrap();

    let expected4 = Buffer::with_lines(vec![
      "record 1            ",
      "record 2            ",
      "record 4 should be  ",
      "long enough to be   ",
      "wrapped             ",
      "record 5            ",
      "record 6            ",
    ]);

    terminal.backend().assert_buffer(&expected4);

    log.scroll_up(1); // to reset select state
    log.scroll_down(11);

    terminal
      .draw(|f| log.render_list(f, f.size(), Block::default(), Style::default(), false))
      .unwrap();

    let mut expected5 = Buffer::with_lines(vec![
      "record 5            ",
      "record 6            ",
      "record 7            ",
      "record 8            ",
      "record 9            ",
      "record 10           ",
      "which is again      ",
    ]);

    // Second row table header style
    for col in 0..=19 {
      expected5
        .get_mut(col, 6)
        .set_style(Style::default().add_modifier(Modifier::BOLD));
    }

    terminal.backend().assert_buffer(&expected5);
  }
}
