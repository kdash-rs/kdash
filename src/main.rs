#![warn(rust_2018_idioms)]
#[deny(clippy::shadow_unrelated)]
mod app;
mod banner;
mod cmd;
mod event;
mod handlers;
mod network;
mod ui;

use std::{
  io::{self, stdout, Stdout},
  panic::{self, PanicInfo},
  sync::Arc,
};

use anyhow::Result;
use app::App;
use banner::BANNER;
use clap::Parser;
use cmd::{CmdRunner, IoCmdEvent};
use crossterm::{
  execute,
  terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use event::Key;
use network::{
  get_client,
  stream::{IoStreamEvent, NetworkStream},
  IoEvent, Network,
};
use tokio::sync::{mpsc, Mutex};
use tui::{
  backend::{Backend, CrosstermBackend},
  Terminal,
};

/// kdash CLI
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, override_usage = "Press `?` while running the app to see keybindings", before_help = BANNER)]
pub struct Cli {
  /// Set the tick rate (milliseconds): the lower the number the higher the FPS.
  #[arg(short, long, value_parser, default_value_t = 250)]
  pub tick_rate: u64,
  /// Set the network call polling rate (milliseconds, should be multiples of tick-rate):
  /// the lower the number the higher the network calls.
  #[arg(short, long, value_parser, default_value_t = 5000)]
  pub poll_rate: u64,
  /// whether unicode symbols are used to improve the overall look of the app
  #[arg(short, long, value_parser, default_value_t = true)]
  pub enhanced_graphics: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
  panic::set_hook(Box::new(|info| {
    panic_hook(info);
  }));

  // parse CLI arguments
  let cli = Cli::parse();

  if cli.tick_rate >= 1000 {
    panic!("Tick rate must be below 1000");
  }
  if (cli.poll_rate % cli.tick_rate) > 0u64 {
    panic!("Poll rate must be multiple of tick-rate");
  }

  // channels for communication between network/cmd threads & UI thread
  let (sync_io_tx, sync_io_rx) = mpsc::channel::<IoEvent>(500);
  let (sync_io_stream_tx, sync_io_stream_rx) = mpsc::channel::<IoStreamEvent>(500);
  let (sync_io_cmd_tx, sync_io_cmd_rx) = mpsc::channel::<IoCmdEvent>(500);

  // Initialize app state
  let app = Arc::new(Mutex::new(App::new(
    sync_io_tx,
    sync_io_stream_tx,
    sync_io_cmd_tx,
    cli.enhanced_graphics,
    cli.poll_rate / cli.tick_rate,
  )));

  // make copies for the network/cli threads
  let app_nw = Arc::clone(&app);
  let app_stream = Arc::clone(&app);
  let app_cli = Arc::clone(&app);

  // Launch network thread
  std::thread::spawn(move || {
    start_network(sync_io_rx, &app_nw);
  });
  // Launch network thread for streams
  std::thread::spawn(move || {
    start_stream_network(sync_io_stream_rx, &app_stream);
  });
  // Launch thread for cmd runner
  std::thread::spawn(move || {
    start_cmd_runner(sync_io_cmd_rx, &app_cli);
  });
  // Launch the UI asynchronously
  // The UI must run in the "main" thread
  start_ui(cli, &app).await?;

  Ok(())
}

#[tokio::main]
async fn start_network(mut io_rx: mpsc::Receiver<IoEvent>, app: &Arc<Mutex<App>>) {
  match get_client(None).await {
    Ok(client) => {
      let mut network = Network::new(client, app);

      while let Some(io_event) = io_rx.recv().await {
        network.handle_network_event(io_event).await;
      }
    }
    Err(e) => panic!("Unable to obtain Kubernetes client {}", e),
  }
}

#[tokio::main]
async fn start_stream_network(mut io_rx: mpsc::Receiver<IoStreamEvent>, app: &Arc<Mutex<App>>) {
  match get_client(None).await {
    Ok(client) => {
      let mut network = NetworkStream::new(client, app);

      while let Some(io_event) = io_rx.recv().await {
        network.handle_network_stream_event(io_event).await;
      }
    }
    Err(e) => panic!("Unable to obtain Kubernetes client {}", e),
  }
}

#[tokio::main]
async fn start_cmd_runner(mut io_rx: mpsc::Receiver<IoCmdEvent>, app: &Arc<Mutex<App>>) {
  let mut cmd = CmdRunner::new(app);

  while let Some(io_event) = io_rx.recv().await {
    cmd.handle_cmd_event(io_event).await;
  }
}

async fn start_ui(cli: Cli, app: &Arc<Mutex<App>>) -> Result<()> {
  // Terminal initialization
  let mut stdout = stdout();
  // not capturing mouse to make text select/copy possible
  execute!(stdout, EnterAlternateScreen)?;
  // see https://docs.rs/crossterm/0.17.7/crossterm/terminal/#raw-mode
  enable_raw_mode()?;
  // terminal backend for cross platform support
  let backend = CrosstermBackend::new(stdout);
  let mut terminal = Terminal::new(backend)?;
  terminal.clear()?;
  terminal.hide_cursor()?;
  // custom events
  let events = event::Events::new(cli.tick_rate);
  let mut is_first_render = true;
  // main UI loop
  loop {
    let mut app = app.lock().await;
    // Get the size of the screen on each loop to account for resize event
    if let Ok(size) = terminal.backend().size() {
      // Reset the help menu if the terminal was resized
      if app.refresh || app.size != size {
        app.size = size;

        // Based on the size of the terminal, adjust how many cols are
        // displayed in the tables
        if app.size.width > 8 {
          app.table_cols = app.size.width - 1;
        } else {
          app.table_cols = 2;
        }
      }
    };

    // draw the UI layout
    terminal.draw(|f| ui::draw(f, &mut app))?;

    // handle key events
    match events.next()? {
      event::Event::Input(key) => {
        // quit on CTRL + C
        if key == Key::Ctrl('c') {
          break;
        }
        // handle all other keys
        handlers::handle_key_events(key, &mut app).await
      }
      // handle mouse events
      event::Event::MouseInput(mouse) => handlers::handle_mouse_events(mouse, &mut app).await,
      // handle tick events
      event::Event::Tick => {
        app.on_tick(is_first_render).await;
      }
    }

    is_first_render = false;

    if app.should_quit {
      break;
    }
  }

  terminal.show_cursor()?;
  shutdown(terminal)?;

  Ok(())
}

// shutdown the CLI and show terminal
fn shutdown(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
  disable_raw_mode()?;
  execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
  terminal.show_cursor()?;
  Ok(())
}

#[cfg(debug_assertions)]
fn panic_hook(info: &PanicInfo<'_>) {
  use backtrace::Backtrace;
  use crossterm::style::Print;

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
  )
  .unwrap();
}

#[cfg(not(debug_assertions))]
fn panic_hook(info: &PanicInfo<'_>) {
  use human_panic::{handle_dump, print_msg, Metadata};

  let meta = Metadata {
    version: env!("CARGO_PKG_VERSION").into(),
    name: env!("CARGO_PKG_NAME").into(),
    authors: env!("CARGO_PKG_AUTHORS").replace(":", ", ").into(),
    homepage: env!("CARGO_PKG_HOMEPAGE").into(),
  };
  let file_path = handle_dump(&meta, info);
  disable_raw_mode().unwrap();
  execute!(io::stdout(), LeaveAlternateScreen).unwrap();
  print_msg(file_path, &meta).expect("human-panic: printing error message to console failed");
}
