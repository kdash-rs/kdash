use std::collections::VecDeque;

use async_trait::async_trait;
use ratatui::{
  layout::Rect,
  style::{Modifier, Style},
  text::{Line as RatatuiLine, Span},
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
/// Trait for workload resources that own pods via a label selector.
pub trait HasPodSelector {
  /// Returns a comma-separated label selector string (e.g., "app=web,version=v2")
  /// suitable for use with `ListParams::labels()`. Returns `None` if the resource
  /// has no selector (e.g., missing spec).
  fn pod_label_selector(&self) -> Option<String>;
}

/// Helper to convert a BTreeMap of labels to a comma-separated selector string.
pub fn labels_to_selector(labels: &std::collections::BTreeMap<String, String>) -> String {
  labels
    .iter()
    .map(|(k, v)| format!("{}={}", k, v))
    .collect::<Vec<_>>()
    .join(",")
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
  pub filter: String,
  pub filter_active: bool,
  /// When a filter is active, maps visible row index → `items` index.
  /// Empty when no filter is applied.
  pub filtered_indices: Vec<usize>,
}

pub trait FilterableTable {
  fn filter_text(&self) -> &str;
  fn is_filter_active(&self) -> bool;
  fn count_label(&self) -> String;
  fn filter_parts_mut(&mut self) -> (&mut String, &mut bool, &mut TableState);
}

impl<T> StatefulTable<T> {
  pub fn new() -> StatefulTable<T> {
    StatefulTable {
      state: TableState::default(),
      items: Vec::new(),
      filter: String::new(),
      filter_active: false,
      filtered_indices: Vec::new(),
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

impl<T> FilterableTable for StatefulTable<T> {
  fn filter_text(&self) -> &str {
    &self.filter
  }

  fn is_filter_active(&self) -> bool {
    self.filter_active
  }

  fn count_label(&self) -> String {
    if self.filter.is_empty() {
      self.items.len().to_string()
    } else {
      format!("{}/{}", self.filtered_indices.len(), self.items.len())
    }
  }

  fn filter_parts_mut(&mut self) -> (&mut String, &mut bool, &mut TableState) {
    (&mut self.filter, &mut self.filter_active, &mut self.state)
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
  /// A clone of the currently selected item.
  /// When a filter is active, maps the visual index through `filtered_indices`
  /// so the correct item is returned regardless of filtering.
  pub fn get_selected_item_copy(&self) -> Option<T> {
    if !self.items.is_empty() {
      self.state.selected().and_then(|i| {
        if self.filtered_indices.is_empty() {
          self.items.get(i).cloned()
        } else {
          self
            .filtered_indices
            .get(i)
            .and_then(|&real| self.items.get(real).cloned())
        }
      })
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

#[derive(Debug)]
pub struct ScrollableTxt {
  items: Vec<String>,
  pub offset: usize,
  /// Pre-joined text, computed once when items change.
  txt_cache: String,
  /// Cached syntax-highlighted lines, reused across render frames.
  /// Invalidated when content or theme changes.
  pub highlighted_lines: Vec<RatatuiLine<'static>>,
  /// The theme used to produce `highlighted_lines` (true = light).
  pub highlight_light_theme: bool,
}

impl PartialEq for ScrollableTxt {
  fn eq(&self, other: &Self) -> bool {
    self.items == other.items && self.offset == other.offset
  }
}

impl Eq for ScrollableTxt {}

impl ScrollableTxt {
  pub fn new() -> ScrollableTxt {
    ScrollableTxt {
      items: vec![],
      offset: 0,
      txt_cache: String::new(),
      highlighted_lines: Vec::new(),
      highlight_light_theme: false,
    }
  }

  pub fn with_string(item: String) -> ScrollableTxt {
    let items: Vec<&str> = item.split('\n').collect();
    let items: Vec<String> = items.iter().map(|it| it.to_string()).collect();
    ScrollableTxt {
      txt_cache: item,
      items,
      offset: 0,
      highlighted_lines: Vec::new(),
      highlight_light_theme: false,
    }
  }

  pub fn get_txt(&self) -> &str {
    &self.txt_cache
  }
}

impl Scrollable for ScrollableTxt {
  fn scroll_down(&mut self, increment: usize) {
    // Ratatui's Paragraph with Wrap counts scroll offset in *visual* rows
    // (post-wrap), but we only know the number of source lines.  We cap at
    // items.len() - 1 so at least the last source line remains visible,
    // while still allowing enough scroll range for wrapped content.
    let max_offset = self.items.len().saturating_sub(1);
    if self.offset < max_offset {
      self.offset = (self.offset + increment).min(max_offset);
    }
  }
  fn scroll_up(&mut self, decrement: usize) {
    if self.offset > 0 {
      self.offset = self.offset.saturating_sub(decrement);
    }
  }
}

// TODO implement line buffer to avoid gathering too much data in memory
const MAX_LOG_RECORDS: usize = 10_000;

#[derive(Debug, Clone)]
pub struct LogsState {
  /// Stores the log messages to be displayed
  ///
  /// (original_message, (wrapped_message, wrapped_at_width))
  #[allow(clippy::type_complexity)]
  records: VecDeque<(String, Option<(Vec<ListItem<'static>>, u16)>)>,
  wrapped_length: usize,
  viewport_height: usize,
  pub state: ListState,
  pub id: String,
}

impl LogsState {
  pub fn new(id: String) -> LogsState {
    LogsState {
      records: VecDeque::with_capacity(512),
      state: ListState::default(),
      wrapped_length: 0,
      viewport_height: 0,
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
    self.viewport_height = available_lines;
    let wrap_width = logs_area.width.max(1);
    let mut items = self.wrapped_items(wrap_width, style);
    self.wrapped_length = items.len();

    if follow {
      self.unselect();
      let wrapped_lines_to_skip = items.len().saturating_sub(available_lines);
      items = items.into_iter().skip(wrapped_lines_to_skip).collect();
    }

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
  #[cfg(test)]
  pub fn add_record(&mut self, record: String) {
    self.records.push_back((record, None));
    while self.records.len() > MAX_LOG_RECORDS {
      self.records.pop_front();
    }
  }

  /// Add multiple records in a batch
  pub fn add_records(&mut self, records: Vec<String>) {
    for record in records {
      self.records.push_back((record, None));
    }
    while self.records.len() > MAX_LOG_RECORDS {
      self.records.pop_front();
    }
  }

  /// Get the last n raw log lines (for dedup on reconnect)
  pub fn last_n_records(&self, n: usize) -> Vec<&str> {
    self
      .records
      .iter()
      .rev()
      .take(n)
      .map(|(s, _)| s.as_str())
      .collect()
  }

  fn unselect(&mut self) {
    self.state.select(None);
  }

  pub fn freeze_follow_position(&mut self) {
    if self.state.selected().is_none() {
      let offset = self.wrapped_length.saturating_sub(self.viewport_height);
      self.state.select(Some(offset));
    }
  }

  fn wrapped_items(&mut self, width: u16, style: Style) -> Vec<ListItem<'static>> {
    let logs_area_width = width as usize;

    self
      .records
      .iter_mut()
      .flat_map(|record| {
        if let Some(wrapped) = &record.1 {
          if wrapped.1 == width {
            return wrapped.0.clone();
          }
        }

        record.1 = Some((
          textwrap::wrap(record.0.as_ref(), logs_area_width)
            .into_iter()
            .map(|line| line.to_string())
            .map(|line| Span::styled(line, style))
            .map(ListItem::new)
            .collect::<Vec<ListItem<'_>>>(),
          width,
        ));

        record
          .1
          .as_ref()
          .map(|wrapped| wrapped.0.clone())
          .unwrap_or_default()
      })
      .collect()
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
  use ratatui::{backend::TestBackend, buffer::Buffer, layout::Position, Terminal};

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
    assert!(sft.filter.is_empty());
    assert!(!sft.filter_active);
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
  fn test_filtered_selection_returns_correct_item() {
    let mut sft: StatefulTable<&str> = StatefulTable::new();
    sft.set_items(vec!["alpha", "beta", "gamma", "delta", "epsilon"]);

    // Simulate a filter that shows items at indices 1, 3 (beta, delta)
    sft.filtered_indices = vec![1, 3];

    // Visual row 0 → items[1] = "beta"
    sft.state.select(Some(0));
    assert_eq!(sft.get_selected_item_copy(), Some("beta"));

    // Visual row 1 → items[3] = "delta"
    sft.state.select(Some(1));
    assert_eq!(sft.get_selected_item_copy(), Some("delta"));
  }

  #[test]
  fn test_no_filter_returns_direct_index() {
    let mut sft: StatefulTable<&str> = StatefulTable::new();
    sft.set_items(vec!["alpha", "beta", "gamma"]);

    // No filter — filtered_indices is empty
    sft.state.select(Some(2));
    assert_eq!(sft.get_selected_item_copy(), Some("gamma"));
  }

  #[test]
  fn test_filtered_selection_out_of_range_returns_none() {
    let mut sft: StatefulTable<&str> = StatefulTable::new();
    sft.set_items(vec!["alpha", "beta", "gamma"]);

    // Filter shows 1 item but selection points past it
    sft.filtered_indices = vec![2];
    sft.state.select(Some(5));
    assert_eq!(sft.get_selected_item_copy(), None);
  }

  #[test]
  fn test_filtered_empty_matches_returns_none() {
    let mut sft: StatefulTable<&str> = StatefulTable::new();
    sft.set_items(vec!["alpha", "beta"]);

    // Filter matches nothing
    sft.filtered_indices = vec![];
    sft.state.select(Some(0));
    // filtered_indices is empty → direct indexing (no filter active)
    assert_eq!(sft.get_selected_item_copy(), Some("alpha"));
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

    // 3 items → max offset = 2 (last line visible)
    stxt.scroll_down(1);
    assert_eq!(stxt.offset, 1);
    stxt.scroll_down(5);
    assert_eq!(stxt.offset, 2);

    // 10 lines → max offset = 9
    let mut stxt2 = ScrollableTxt::with_string("te\nst\nmul\ntil\ni\nne\nstr\ni\nn\ng".into());
    assert_eq!(stxt2.items.len(), 10);
    stxt2.scroll_down(1);
    assert_eq!(stxt2.offset, 1);
    stxt2.scroll_down(1);
    assert_eq!(stxt2.offset, 2);
    stxt2.scroll_down(5);
    assert_eq!(stxt2.offset, 7);
    for _ in 0..5 {
      stxt2.scroll_down(1);
    }
    // capped at len - 1 = 9
    assert_eq!(stxt2.offset, 9);
    stxt2.scroll_up(1);
    assert_eq!(stxt2.offset, 8);
    stxt2.scroll_up(8);
    assert_eq!(stxt2.offset, 0);
    stxt2.scroll_up(1);
    // no overflow past (0)
    assert_eq!(stxt2.offset, 0);
  }

  #[test]
  fn test_scrollable_txt_viewport_reaches_end() {
    // 100 lines → max offset = 99
    let lines: Vec<String> = (0..100).map(|i| format!("line {}", i)).collect();
    let mut stxt = ScrollableTxt::with_string(lines.join("\n"));

    assert_eq!(stxt.items.len(), 100);
    for _ in 0..110 {
      stxt.scroll_down(1);
    }
    assert_eq!(stxt.offset, 99);
  }

  #[test]
  fn test_scrollable_txt_single_line_no_scroll() {
    // 1 line → max offset = 0
    let mut stxt = ScrollableTxt::with_string("hello".into());

    stxt.scroll_down(1);
    assert_eq!(stxt.offset, 0);
  }

  #[test]
  fn test_scrollable_txt_scroll_cap_is_len_minus_one() {
    // 20 lines → max offset = 19 regardless of viewport
    let lines: Vec<String> = (0..20).map(|i| format!("line {}", i)).collect();
    let mut stxt = ScrollableTxt::with_string(lines.join("\n"));

    for _ in 0..30 {
      stxt.scroll_down(1);
    }
    assert_eq!(stxt.offset, 19);
  }

  #[test]
  fn test_scrollable_txt_beyond_u16_max() {
    let line_count = u16::MAX as usize + 100; // 65635 lines
    let lines: Vec<String> = (0..line_count).map(|i| format!("line {}", i)).collect();
    let mut stxt = ScrollableTxt::with_string(lines.join("\n"));
    assert_eq!(stxt.items.len(), line_count);
    assert_eq!(stxt.offset, 0);

    // Scroll down past u16::MAX in large steps — should not wrap or panic
    let target = line_count.saturating_sub(1); // max reachable offset (len - 1)
    for _ in 0..(target / 1000) {
      stxt.scroll_down(1000);
    }
    // Finish off with single increments to reach the cap
    for _ in 0..1000 {
      stxt.scroll_down(1);
    }

    // Offset must be beyond what u16 could hold and must not have wrapped
    assert!(
      stxt.offset > u16::MAX as usize,
      "offset {} should exceed u16::MAX (65535)",
      stxt.offset
    );
    // Must be capped at items.len() - 1
    assert!(
      stxt.offset <= target,
      "offset {} should be at most {}",
      stxt.offset,
      target
    );

    // Scroll back up past u16::MAX boundary — should not wrap or panic
    for _ in 0..(stxt.offset / 1000) {
      stxt.scroll_up(1000);
    }
    for _ in 0..1000 {
      stxt.scroll_up(1);
    }
    assert_eq!(stxt.offset, 0);
  }

  #[test]
  fn test_scrollable_txt_last_line_always_reachable() {
    // 10 source lines → max offset = 9 (last line at top of viewport)
    let lines: Vec<String> = (0..10).map(|i| format!("line {}", i)).collect();
    let mut stxt = ScrollableTxt::with_string(lines.join("\n"));

    for _ in 0..20 {
      stxt.scroll_down(1);
    }
    assert_eq!(stxt.offset, 9);
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
      .draw(|f| log.render_list(f, f.area(), Block::default(), Style::default(), true))
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
      .draw(|f| log.render_list(f, f.area(), Block::default(), Style::default(), false))
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
      .draw(|f| log.render_list(f, f.area(), Block::default(), Style::default(), true))
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
      .draw(|f| log.render_list(f, f.area(), Block::default(), Style::default(), false))
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
      .draw(|f| log.render_list(f, f.area(), Block::default(), Style::default(), false))
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
        .cell_mut(Position::new(col, 6))
        .unwrap()
        .set_style(Style::default().add_modifier(Modifier::BOLD));
    }

    terminal.backend().assert_buffer(&expected5);
  }

  #[test]
  fn test_logs_state_bounded() {
    let mut log = LogsState::new("bounded".into());

    // Add more than MAX_LOG_RECORDS entries
    for i in 0..MAX_LOG_RECORDS + 100 {
      log.add_record(format!("record {}", i));
    }

    // Should be capped at MAX_LOG_RECORDS
    assert_eq!(log.records.len(), MAX_LOG_RECORDS);

    // Oldest records should have been evicted — first record should be 100
    assert_eq!(log.records.front().unwrap().0, "record 100");
    assert_eq!(
      log.records.back().unwrap().0,
      format!("record {}", MAX_LOG_RECORDS + 99)
    );
  }

  #[test]
  fn test_logs_state_bounded_exactly_at_limit() {
    let mut log = LogsState::new("exact".into());

    // Add exactly MAX_LOG_RECORDS entries — no eviction should occur
    for i in 0..MAX_LOG_RECORDS {
      log.add_record(format!("record {}", i));
    }

    assert_eq!(log.records.len(), MAX_LOG_RECORDS);
    assert_eq!(log.records.front().unwrap().0, "record 0");
    assert_eq!(
      log.records.back().unwrap().0,
      format!("record {}", MAX_LOG_RECORDS - 1)
    );
  }

  #[test]
  fn test_logs_state_bounded_one_over() {
    let mut log = LogsState::new("one_over".into());

    for i in 0..MAX_LOG_RECORDS + 1 {
      log.add_record(format!("record {}", i));
    }

    assert_eq!(log.records.len(), MAX_LOG_RECORDS);
    // First record should be evicted
    assert_eq!(log.records.front().unwrap().0, "record 1");
    assert_eq!(
      log.records.back().unwrap().0,
      format!("record {}", MAX_LOG_RECORDS)
    );
  }

  #[test]
  fn test_logs_state_empty() {
    let log = LogsState::new("empty".into());
    assert_eq!(log.records.len(), 0);
    assert_eq!(log.get_plain_text(), "");
  }

  #[test]
  fn test_logs_state_plain_text_preserves_order() {
    let mut log = LogsState::new("text".into());
    log.add_record("first".into());
    log.add_record("second".into());
    log.add_record("third".into());

    let text = log.get_plain_text();
    assert_eq!(text, "\nfirst\nsecond\nthird");
  }

  #[test]
  fn test_logs_state_follow_tracks_last_wrapped_lines() {
    let mut log = LogsState::new("follow".into());
    let backend = TestBackend::new(12, 4);
    let mut terminal = Terminal::new(backend).unwrap();

    log.add_record("alpha".into());
    log.add_record("beta".into());
    log.add_record("gamma delta epsilon".into());

    terminal
      .draw(|f| log.render_list(f, f.area(), Block::default(), Style::default(), true))
      .unwrap();

    let expected_initial = Buffer::with_lines(vec![
      "alpha       ",
      "beta        ",
      "gamma delta ",
      "epsilon     ",
    ]);
    terminal.backend().assert_buffer(&expected_initial);

    log.add_record("zeta eta theta".into());

    terminal
      .draw(|f| log.render_list(f, f.area(), Block::default(), Style::default(), true))
      .unwrap();

    let expected_after_append = Buffer::with_lines(vec![
      "gamma delta ",
      "epsilon     ",
      "zeta eta    ",
      "theta       ",
    ]);
    terminal.backend().assert_buffer(&expected_after_append);
  }

  #[test]
  fn test_logs_state_freeze_follow_position_keeps_current_bottom_offset() {
    let mut log = LogsState::new("freeze".into());
    let backend = TestBackend::new(12, 4);
    let mut terminal = Terminal::new(backend).unwrap();

    log.add_record("alpha".into());
    log.add_record("beta".into());
    log.add_record("gamma delta epsilon".into());
    log.add_record("zeta eta theta".into());

    terminal
      .draw(|f| log.render_list(f, f.area(), Block::default(), Style::default(), true))
      .unwrap();

    log.freeze_follow_position();

    assert_eq!(log.state.selected(), Some(2));
  }
}
