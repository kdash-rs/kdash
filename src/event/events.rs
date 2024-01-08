//  adapted from tui-rs/examples/crossterm_demo.rs
use std::{
  sync::mpsc,
  thread,
  time::{Duration, Instant},
};

use crossterm::event::{self, Event as CEvent, KeyEvent, MouseEvent};

use super::Key;

#[derive(Debug, Clone, Copy)]
/// Configuration for event handling.
pub struct EventConfig {
  pub exit_key: Key,
  /// The tick rate at which the application will sent an tick event.
  pub tick_rate: Duration,
}

impl Default for EventConfig {
  fn default() -> EventConfig {
    EventConfig {
      exit_key: Key::Ctrl('c'),
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
      ..EventConfig::default()
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
        if event::poll(timeout).unwrap() {
          match event::read().unwrap() {
            CEvent::Key(key_event) => handle_key_event(&event_tx, key_event),
            CEvent::Mouse(mouse_event) => {
              event_tx.send(Event::MouseInput(mouse_event)).unwrap();
            }
            _ => {}
          }
        }
        if last_tick.elapsed() >= tick_rate {
          event_tx.send(Event::Tick).unwrap();
          last_tick = Instant::now();
        }
      }
    });

    Events { rx, _tx: tx }
  }

  /// Attempts to read an event.
  /// This function will block the current thread.
  pub fn next(&self) -> Result<Event<KeyEvent, MouseEvent>, mpsc::RecvError> {
    self.rx.recv()
  }
}

#[cfg(target_os = "windows")]
fn handle_key_event(event_tx: &mpsc::Sender<Event<KeyEvent, MouseEvent>>, key_event: KeyEvent) {
  if key_event.kind == event::KeyEventKind::Press {
    event_tx.send(Event::Input(key_event)).unwrap();
  }
}

#[cfg(not(target_os = "windows"))]
fn handle_key_event(event_tx: &mpsc::Sender<Event<KeyEvent, MouseEvent>>, key_event: KeyEvent) {
  event_tx.send(Event::Input(key_event)).unwrap();
}
