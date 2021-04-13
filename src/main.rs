mod app;
mod banner;
mod event;
mod handlers;
mod network;
mod ui;

use crate::event::Key;
use app::App;
use banner::BANNER;
use network::{get_client, IoEvent, Network};

use anyhow::Result;
use backtrace::Backtrace;
use clap::{App as ClapApp, Arg};
use crossterm::{
  event::{DisableMouseCapture, EnableMouseCapture},
  execute,
  style::Print,
  terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
  io::{self, stdout, Stdout},
  panic::{self, PanicInfo},
  sync::Arc,
};
use tokio::sync::Mutex;
use tui::{
  backend::{Backend, CrosstermBackend},
  Terminal,
};

/// kdash CLI
#[derive(Debug)]
struct Cli {
  /// time in ms between two ticks.
  tick_rate: u64,
  /// time in ms between two network calls.
  poll_rate: u64,
  /// whether unicode symbols are used to improve the overall look of the app
  enhanced_graphics: bool,
}

// shutdown the CLI and show terminal
fn shutdown(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
  disable_raw_mode()?;
  execute!(
    terminal.backend_mut(),
    LeaveAlternateScreen,
    DisableMouseCapture
  )?;
  terminal.show_cursor()?;
  Ok(())
}

fn panic_hook(info: &PanicInfo<'_>) {
  if cfg!(debug_assertions) {
    let location = info.location().unwrap();

    let msg = match info.payload().downcast_ref::<&'static str>() {
      Some(s) => *s,
      None => match info.payload().downcast_ref::<String>() {
        Some(s) => &s[..],
        None => "Box<Any>",
      },
    };

    let stacktrace: String = format!("{:?}", Backtrace::new()).replace('\n', "\n\r");

    disable_raw_mode().unwrap();
    execute!(
      io::stdout(),
      LeaveAlternateScreen,
      Print(format!(
        "thread '<unnamed>' panicked at '{}', {}\n\r{}",
        msg, location, stacktrace
      )),
      DisableMouseCapture
    )
    .unwrap();
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  panic::set_hook(Box::new(|info| {
    panic_hook(info);
  }));

  let clap_app = ClapApp::new(env!("CARGO_PKG_NAME"))
    .version(env!("CARGO_PKG_VERSION"))
    .author(env!("CARGO_PKG_AUTHORS"))
    .about(env!("CARGO_PKG_DESCRIPTION"))
    .usage("Press `?` while running the app to see keybindings")
    .before_help(BANNER)
    .arg(
      Arg::with_name("tick-rate")
        .short("t")
        .long("tick-rate")
        .help("Set the tick rate (milliseconds): the lower the number the higher the FPS.")
        .takes_value(true),
    )
    .arg(
      Arg::with_name("poll-rate")
        .short("p")
        .long("poll-rate")
        .help("Set the network call polling rate (milliseconds, should be multiples of tick-rate): the lower the number the higher the network calls.")
        .takes_value(true),
    );

  let mut cli: Cli = Cli {
    tick_rate: 250,
    poll_rate: 2000,
    enhanced_graphics: true,
  };
  let matches = clap_app.get_matches();

  if let Some(tick_rate) = matches
    .value_of("tick-rate")
    .and_then(|tick_rate| tick_rate.parse().ok())
  {
    if tick_rate >= 1000 {
      panic!("Tick rate must be below 1000");
    } else {
      cli.tick_rate = tick_rate;
    }
  }

  if let Some(poll_rate) = matches
    .value_of("poll-rate")
    .and_then(|poll_rate| poll_rate.parse().ok())
  {
    if (poll_rate % cli.tick_rate) > 0u64 {
      panic!("Poll rate must be multiple of tick-rate");
    } else {
      cli.poll_rate = poll_rate;
    }
  }

  let (sync_io_tx, sync_io_rx) = std::sync::mpsc::channel::<IoEvent>();

  // Initialize app state
  let app = Arc::new(Mutex::new(App::new(
    sync_io_tx,
    cli.enhanced_graphics,
    cli.poll_rate / cli.tick_rate,
  )));

  // Launch the UI (async)
  let cloned_app = Arc::clone(&app);
  let client = get_client().await?;

  // Launch network thread
  std::thread::spawn(move || {
    let mut network = Network::new(client, &app);
    start_tokio(sync_io_rx, &mut network);
  });
  // The UI must run in the "main" thread
  start_ui(cli, &cloned_app).await?;

  Ok(())
}

#[tokio::main]
async fn start_tokio<'a>(io_rx: std::sync::mpsc::Receiver<IoEvent>, network: &mut Network) {
  while let Ok(io_event) = io_rx.recv() {
    network.handle_network_event(io_event).await;
  }
}

async fn start_ui(cli: Cli, app: &Arc<Mutex<App>>) -> Result<()> {
  // Terminal initialization
  let mut stdout = stdout();
  execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
  // see https://docs.rs/crossterm/0.17.7/crossterm/terminal/#raw-mode
  enable_raw_mode()?;

  let backend = CrosstermBackend::new(stdout);
  let mut terminal = Terminal::new(backend)?;
  terminal.hide_cursor()?;
  terminal.clear()?;

  let events = event::Events::new(cli.tick_rate);

  let mut is_first_render = true;

  loop {
    let mut app = app.lock().await;
    // Get the size of the screen on each loop to account for resize event
    if let Ok(size) = terminal.backend().size() {
      // Reset the help menu if the terminal was resized
      if is_first_render || app.size != size {
        app.help_menu_max_lines = 0;
        app.help_menu_offset = 0;
        app.help_menu_page = 0;

        app.size = size;

        // Based on the size of the terminal, adjust how many lines are
        // displayed in the help menu
        if app.size.height > 8 {
          app.help_menu_max_lines = (app.size.height as u32) - 8;
        } else {
          app.help_menu_max_lines = 0;
        }
      }
    };

    // draw the UI layout
    terminal.draw(|f| ui::draw(f, &mut app))?;

    // handle key vents
    match events.next()? {
      event::Event::Input(key) => {
        // handle CTRL + C
        if key == Key::Ctrl('c') {
          break;
        }
        // handle all other keys
        handlers::handle_app(key, &mut app)
      }
      event::Event::Tick => {
        app.on_tick();
      }
    }

    // Delay one time requests until first render, will have the effect of improving
    // startup speed
    if is_first_render {
      app.dispatch(IoEvent::GetCLIInfo);
      app.dispatch(IoEvent::GetKubeConfig);

      is_first_render = false;
    }

    if app.should_quit {
      break;
    }
  }

  terminal.show_cursor()?;
  shutdown(terminal)?;

  Ok(())
}
