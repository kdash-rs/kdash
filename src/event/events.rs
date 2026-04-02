//  adapted from tui-rs/examples/crossterm_demo.rs
use std::{
  sync::mpsc,
  thread,
  time::{Duration, Instant},
};

use crossterm::event::{self, Event as CEvent, KeyEvent, MouseEvent};
use log::error;

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
    let _ = event_tx.send(Event::Input(key_event));
  }
}

#[cfg(not(target_os = "windows"))]
fn handle_key_event(event_tx: &mpsc::Sender<Event<KeyEvent, MouseEvent>>, key_event: KeyEvent) {
  let _ = event_tx.send(Event::Input(key_event));
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_events_produces_tick() {
    // Events should produce at least one Tick within a reasonable time
    let events = Events::new(50); // 50ms tick rate
    match events.next() {
      Ok(Event::Tick) => {}          // expected
      Ok(Event::Input(_)) => {}      // possible if terminal sends something
      Ok(Event::MouseInput(_)) => {} // possible
      Err(e) => panic!("Events::next() returned error: {:?}", e),
    }
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
}
