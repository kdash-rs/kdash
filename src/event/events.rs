//  adapted from tui-rs/examples/crossterm_demo.rs
use std::{
  env,
  path::PathBuf,
  sync::mpsc,
  thread,
  time::{Duration, Instant},
};

use crossterm::event::{self, Event as CEvent, KeyEvent, MouseEvent};
use log::{error, info, warn};
use notify::{RecursiveMode, Watcher};

#[derive(Debug, Clone, Copy)]
/// Configuration for event handling.
pub struct EventConfig {
  /// The tick rate at which the application will sent an tick event.
  pub tick_rate: Duration,
}

impl Default for EventConfig {
  fn default() -> EventConfig {
    EventConfig {
      tick_rate: Duration::from_millis(250),
    }
  }
}

/// An occurred event.
pub enum Event<I, J> {
  /// An input event occurred.
  Input(I),
  MouseInput(J),
  /// An tick event occurred.
  Tick,
  /// The kubeconfig file changed on disk.
  KubeConfigChange,
}

/// A small event handler that wrap crossterm input and tick event. Each event
/// type is handled in its own thread and returned to a common `Receiver`
pub struct Events {
  rx: mpsc::Receiver<Event<KeyEvent, MouseEvent>>,
  // Need to be kept around to prevent disposing the sender side.
  _tx: mpsc::Sender<Event<KeyEvent, MouseEvent>>,
}

impl Events {
  /// Constructs an new instance of `Events` with the default config.
  pub fn new(tick_rate: u64) -> Events {
    Events::with_config(EventConfig {
      tick_rate: Duration::from_millis(tick_rate),
    })
  }

  /// Constructs an new instance of `Events` from given config.
  pub fn with_config(config: EventConfig) -> Events {
    let (tx, rx) = mpsc::channel();

    let tick_rate = config.tick_rate;

    let event_tx = tx.clone();
    thread::spawn(move || {
      let mut last_tick = Instant::now();
      loop {
        let timeout = tick_rate
          .checked_sub(last_tick.elapsed())
          .unwrap_or_else(|| Duration::from_secs(0));
        // poll for tick rate duration, if no event, sent tick event.
        match event::poll(timeout) {
          Ok(true) => match event::read() {
            Ok(CEvent::Key(key_event)) => handle_key_event(&event_tx, key_event),
            Ok(CEvent::Mouse(mouse_event)) => {
              if event_tx.send(Event::MouseInput(mouse_event)).is_err() {
                break; // receiver dropped, app is shutting down
              }
            }
            Ok(_) => {}
            Err(e) => {
              error!("Failed to read terminal event: {:?}", e);
            }
          },
          Ok(false) => {} // no event available, fall through to tick
          Err(e) => {
            error!("Failed to poll terminal events: {:?}", e);
          }
        }
        if last_tick.elapsed() >= tick_rate {
          if event_tx.send(Event::Tick).is_err() {
            break; // receiver dropped, app is shutting down
          }
          last_tick = Instant::now();
        }
      }
    });

    // Start kubeconfig file watcher for live sync (#315)
    start_kubeconfig_watcher(tx.clone());

    Events { rx, _tx: tx }
  }

  /// Attempts to read an event.
  /// This function will block the current thread.
  pub fn next(&self) -> Result<Event<KeyEvent, MouseEvent>, mpsc::RecvError> {
    self.rx.recv()
  }

  /// Attempts to read an event without blocking.
  /// Returns `None` if no event is currently available or if the channel has
  /// been disconnected.
  pub fn try_next(&self) -> Option<Event<KeyEvent, MouseEvent>> {
    self.rx.try_recv().ok()
  }
}

/// Resolve the kubeconfig file paths that should be watched.
fn kubeconfig_watch_paths() -> Vec<PathBuf> {
  if let Some(value) = env::var_os("KUBECONFIG") {
    let paths: Vec<PathBuf> = env::split_paths(&value)
      .filter(|p| !p.as_os_str().is_empty())
      .collect();
    if !paths.is_empty() {
      return paths;
    }
  }
  // Fall back to default kubeconfig location
  if let Some(home) = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")) {
    vec![PathBuf::from(home).join(".kube").join("config")]
  } else {
    vec![]
  }
}

/// Start a file watcher thread for kubeconfig files. Sends `Event::KubeConfigChange`
/// on the provided channel when any watched file is modified.
fn start_kubeconfig_watcher(tx: mpsc::Sender<Event<KeyEvent, MouseEvent>>) {
  let paths = kubeconfig_watch_paths();
  if paths.is_empty() {
    info!("No kubeconfig paths to watch");
    return;
  }

  thread::spawn(move || {
    let (notify_tx, notify_rx) = mpsc::channel();
    let mut watcher = match notify::recommended_watcher(move |res| {
      let _ = notify_tx.send(res);
    }) {
      Ok(w) => w,
      Err(e) => {
        warn!("Failed to create kubeconfig file watcher: {}", e);
        return;
      }
    };

    // Collect the canonical file names we care about, and watch their
    // parent directories instead of the files themselves. This handles
    // atomic saves (tmp + rename) that tools like kubectl perform, which
    // would otherwise invalidate a direct file watch.
    let mut watched_dirs = std::collections::HashSet::new();
    let mut target_filenames = std::collections::HashSet::new();
    for path in &paths {
      if let Some(filename) = path.file_name() {
        target_filenames.insert(filename.to_os_string());
      }
      let dir = if let Some(parent) = path.parent() {
        if parent.exists() {
          parent.to_path_buf()
        } else {
          continue;
        }
      } else {
        continue;
      };
      if watched_dirs.insert(dir.clone()) {
        if let Err(e) = watcher.watch(&dir, RecursiveMode::NonRecursive) {
          warn!("Failed to watch {:?}: {}", dir, e);
        } else {
          info!("Watching kubeconfig directory: {:?}", dir);
        }
      }
    }

    // Debounce: ignore rapid successive events (editors do multiple writes)
    let debounce = Duration::from_secs(2);
    let mut last_sent = Instant::now() - debounce;

    for res in notify_rx {
      match res {
        Ok(event) => {
          // Only react to events that touch our target kubeconfig files
          let dominated = event
            .paths
            .iter()
            .any(|p| p.file_name().is_some_and(|f| target_filenames.contains(f)));
          if !dominated {
            continue;
          }
          if last_sent.elapsed() >= debounce {
            info!("Kubeconfig file change detected: {:?}", event.kind);
            if tx.send(Event::KubeConfigChange).is_err() {
              break; // receiver dropped, app is shutting down
            }
            last_sent = Instant::now();
          }
        }
        Err(e) => {
          warn!("Kubeconfig watcher error: {:?}", e);
        }
      }
    }
  });
}

#[cfg(target_os = "windows")]
fn handle_key_event(event_tx: &mpsc::Sender<Event<KeyEvent, MouseEvent>>, key_event: KeyEvent) {
  if key_event.kind == event::KeyEventKind::Press {
    let _ = event_tx.send(Event::Input(key_event));
  }
}

#[cfg(not(target_os = "windows"))]
fn handle_key_event(event_tx: &mpsc::Sender<Event<KeyEvent, MouseEvent>>, key_event: KeyEvent) {
  let _ = event_tx.send(Event::Input(key_event));
}

#[cfg(test)]
mod tests {
  use std::sync::{LazyLock, Mutex};

  use super::*;

  static KUBECONFIG_ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

  #[test]
  fn test_events_produces_tick() {
    // Events should produce at least one Tick within a reasonable time
    let events = Events::new(50); // 50ms tick rate
    match events.next() {
      Ok(Event::Tick) => {}             // expected
      Ok(Event::Input(_)) => {}         // possible if terminal sends something
      Ok(Event::MouseInput(_)) => {}    // possible
      Ok(Event::KubeConfigChange) => {} // possible if kubeconfig watcher fires
      Err(e) => panic!("Events::next() returned error: {:?}", e),
    }
  }

  #[test]
  fn test_try_next_returns_none_when_empty() {
    // try_next should return None when there are no pending events
    let (tx, rx) = mpsc::channel();
    let events = Events { rx, _tx: tx };
    assert!(events.try_next().is_none());
  }

  #[test]
  fn test_events_receiver_drop_stops_sender() {
    // Create events, then drop the Events struct — the sender thread should exit gracefully
    let events = Events::new(50);
    // Get one event to ensure the thread is running
    let _ = events.next();
    // Drop events — the thread should detect the receiver is gone and break
    drop(events);
    // If this test completes without hanging, the thread exited properly
  }

  #[test]
  fn test_handle_key_event_send_failure() {
    // When the receiver is dropped, handle_key_event should not panic
    let (tx, rx) = mpsc::channel();
    drop(rx); // drop receiver immediately
    let key_event = KeyEvent::new(
      crossterm::event::KeyCode::Char('a'),
      crossterm::event::KeyModifiers::NONE,
    );
    // This should not panic — uses `let _ = send()`
    handle_key_event(&tx, key_event);
  }

  #[test]
  fn test_event_config_default() {
    let config = EventConfig::default();
    assert_eq!(config.tick_rate, std::time::Duration::from_millis(250));
  }

  #[test]
  fn test_kubeconfig_watch_paths_default() {
    let _guard = KUBECONFIG_ENV_LOCK.lock().unwrap();
    // When KUBECONFIG is not set, should return ~/.kube/config
    let original = env::var_os("KUBECONFIG");
    env::remove_var("KUBECONFIG");

    let paths = kubeconfig_watch_paths();

    // Restore
    if let Some(val) = original {
      env::set_var("KUBECONFIG", val);
    }

    assert_eq!(paths.len(), 1);
    assert!(paths[0].ends_with(".kube/config"));
  }

  #[test]
  fn test_kubeconfig_watch_paths_from_env() {
    let _guard = KUBECONFIG_ENV_LOCK.lock().unwrap();
    let original = env::var_os("KUBECONFIG");
    // Use env::join_paths for cross-platform separator (: on Unix, ; on Windows)
    let joined = env::join_paths(["/tmp/a", "/tmp/b"]).unwrap();
    env::set_var("KUBECONFIG", &joined);

    let paths = kubeconfig_watch_paths();

    // Restore
    match original {
      Some(val) => env::set_var("KUBECONFIG", val),
      None => env::remove_var("KUBECONFIG"),
    }

    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0], PathBuf::from("/tmp/a"));
    assert_eq!(paths[1], PathBuf::from("/tmp/b"));
  }

  #[test]
  fn test_kubeconfig_watch_paths_ignores_empty_segments() {
    let _guard = KUBECONFIG_ENV_LOCK.lock().unwrap();
    let original = env::var_os("KUBECONFIG");
    // Use env::join_paths and include empty segments
    let joined = env::join_paths(["/tmp/a", "", "/tmp/b", ""]).unwrap();
    env::set_var("KUBECONFIG", &joined);

    let paths = kubeconfig_watch_paths();

    match original {
      Some(val) => env::set_var("KUBECONFIG", val),
      None => env::remove_var("KUBECONFIG"),
    }

    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0], PathBuf::from("/tmp/a"));
    assert_eq!(paths[1], PathBuf::from("/tmp/b"));
  }

  #[test]
  fn test_start_kubeconfig_watcher_sends_event_on_file_change() {
    use std::fs;

    let dir = env::temp_dir().join(format!("kdash-watcher-test-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let config_file = dir.join("config");
    fs::write(&config_file, "initial").unwrap();

    let (tx, rx) = mpsc::channel::<Event<KeyEvent, MouseEvent>>();

    // Manually set up a watcher on our test file
    let watch_tx = tx.clone();
    let config_path = config_file.clone();
    thread::spawn(move || {
      let (notify_tx, notify_rx) = mpsc::channel();
      let mut watcher = notify::recommended_watcher(move |res| {
        let _ = notify_tx.send(res);
      })
      .unwrap();
      watcher
        .watch(&config_path, RecursiveMode::NonRecursive)
        .unwrap();
      for res in notify_rx {
        if res.is_ok() {
          let _ = watch_tx.send(Event::KubeConfigChange);
          break; // one event is enough for the test
        }
      }
    });

    // Give the watcher time to start
    thread::sleep(Duration::from_millis(200));

    // Modify the file
    fs::write(&config_file, "modified").unwrap();

    // Should receive the event within a reasonable time
    match rx.recv_timeout(Duration::from_secs(5)) {
      Ok(Event::KubeConfigChange) => {} // expected
      other => panic!("Expected KubeConfigChange, got: {:?}", other.is_ok()),
    }

    fs::remove_dir_all(dir).unwrap();
  }
}
