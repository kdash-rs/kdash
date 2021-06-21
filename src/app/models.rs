use std::collections::VecDeque;

use serde::Serialize;
use tui::{
  backend::Backend,
  layout::Rect,
  style::Style,
  text::Span,
  widgets::{Block, List, ListItem, ListState, TableState},
  Frame,
};

use super::Route;

pub trait KubeResource<T: Serialize> {
  /// convert a kube API object
  fn from_api(item: &T) -> Self;

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

  pub fn _unselect(&mut self) {
    self.state.select(None);
  }
}

impl<T> Scrollable for StatefulTable<T> {
  fn scroll_down(&mut self) {
    if let Some(i) = self.state.selected() {
      if i < self.items.len().wrapping_sub(1) {
        self.state.select(Some(i + 1));
      }
    }
  }

  fn scroll_up(&mut self) {
    if let Some(i) = self.state.selected() {
      if i != 0 {
        self.state.select(Some(i - 1));
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

#[derive(Debug, PartialEq)]
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
    if self.offset < self.items.len().saturating_sub(8) as u16 {
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
  use k8s_openapi::api::core::v1::Namespace;
  use kube::api::ObjectMeta;
  use tui::{backend::TestBackend, buffer::Buffer, Terminal};

  use super::*;
  use crate::app::{ns::KubeNs, ActiveBlock, RouteId};

  #[test]
  fn test_kube_resource() {
    struct TestStruct {
      k8s_obj: Namespace,
    }
    impl KubeResource<Namespace> for TestStruct {
      fn from_api(_: &Namespace) -> Self {
        unimplemented!()
      }

      fn get_k8s_obj(&self) -> &Namespace {
        &self.k8s_obj
      }
    }
    let ts = TestStruct {
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
      "---\napiVersion: v1\nkind: Namespace\nmetadata:\n  name: test\n  namespace: test\n"
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
    sft.scroll_down();
    assert_eq!(sft.state.selected(), Some(1));
    // check scroll overflow
    sft.scroll_down();
    assert_eq!(sft.state.selected(), Some(1));
    sft.scroll_up();
    assert_eq!(sft.state.selected(), Some(0));
    // check scroll overflow
    sft.scroll_up();
    assert_eq!(sft.state.selected(), Some(0));

    let sft = StatefulTable::with_items(vec![KubeNs::default(), KubeNs::default()]);
    assert_eq!(sft.state.selected(), Some(0));
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

    stxt.scroll_down();
    assert_eq!(stxt.offset, 0);

    let mut stxt = ScrollableTxt::with_string("te\nst\nmul\ntil\ni\nne\nstr\ni\nn\ng".into());
    assert_eq!(stxt.items.len(), 10);
    stxt.scroll_down();
    assert_eq!(stxt.offset, 1);
    stxt.scroll_down();
    assert_eq!(stxt.offset, 2);
    stxt.scroll_down();
    // no overflow past (len - 8)
    assert_eq!(stxt.offset, 2);
    stxt.scroll_up();
    assert_eq!(stxt.offset, 1);
    stxt.scroll_up();
    assert_eq!(stxt.offset, 0);
    stxt.scroll_up();
    // no overflow past (0)
    assert_eq!(stxt.offset, 0);
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

    let expected = Buffer::with_lines(vec![
      "record 1            ",
      "record 2            ",
      "record 4 should be  ",
      "long enough to be   ",
      "wrapped             ",
      "record 5            ",
      "record 6            ",
    ]);

    terminal.backend().assert_buffer(&expected);

    log.add_record("record 9".into());
    log.add_record("record 10 which is again looooooooooooooooooooooooooooooonnnng".into());
    log.add_record("record 11".into());
    // enabling follow should scroll back to bottom
    terminal
      .draw(|f| log.render_list(f, f.size(), Block::default(), Style::default(), true))
      .unwrap();

    let expected = Buffer::with_lines(vec![
      "record 8            ",
      "record 9            ",
      "record 10           ",
      "which is again      ",
      "looooooooooooooooooo",
      "oooooooooooonnnng   ",
      "record 11           ",
    ]);

    terminal.backend().assert_buffer(&expected);

    terminal
      .draw(|f| log.render_list(f, f.size(), Block::default(), Style::default(), false))
      .unwrap();

    let expected = Buffer::with_lines(vec![
      "record 1            ",
      "record 2            ",
      "record 4 should be  ",
      "long enough to be   ",
      "wrapped             ",
      "record 5            ",
      "record 6            ",
    ]);

    terminal.backend().assert_buffer(&expected);

    log.scroll_up(); // to reset select state
    log.scroll_down();
    log.scroll_down();
    log.scroll_down();
    log.scroll_down();
    log.scroll_down();
    log.scroll_down();
    log.scroll_down();
    log.scroll_down();
    log.scroll_down();
    log.scroll_down();
    log.scroll_down();

    terminal
      .draw(|f| log.render_list(f, f.size(), Block::default(), Style::default(), false))
      .unwrap();

    let expected = Buffer::with_lines(vec![
      "record 5            ",
      "record 6            ",
      "record 7            ",
      "record 8            ",
      "record 9            ",
      "record 10           ",
      "which is again      ",
    ]);

    terminal.backend().assert_buffer(&expected);
  }
}
