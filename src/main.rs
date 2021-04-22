mod app;
mod banner;
mod cli;
mod cmd;
mod event;
mod handlers;
mod network;
mod ui;

use app::App;
use cli::Cli;
use cmd::{CmdRunner, IoCmdEvent};
use event::Key;
use network::{
  get_client,
  stream::{IoStreamEvent, NetworkStream},
  IoEvent, Network,
};

use anyhow::Result;
use backtrace::Backtrace;
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
use tokio::sync::{mpsc, Mutex};
use tui::{
  backend::{Backend, CrosstermBackend},
  Terminal,
};

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

  let mut cli = Cli::new();
  let clap_app = cli.get_clap_app();
  let matches = clap_app.get_matches();

  // parse CLI arguments
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
async fn start_network(mut io_rx: tokio::sync::mpsc::Receiver<IoEvent>, app: &Arc<Mutex<App>>) {
  match get_client().await {
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
async fn start_stream_network(
  mut io_rx: tokio::sync::mpsc::Receiver<IoStreamEvent>,
  app: &Arc<Mutex<App>>,
) {
  match get_client().await {
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
async fn start_cmd_runner(
  mut io_rx: tokio::sync::mpsc::Receiver<IoCmdEvent>,
  app: &Arc<Mutex<App>>,
) {
  let mut cmd = CmdRunner::new(app);

  while let Some(io_event) = io_rx.recv().await {
    cmd.handle_cmd_event(io_event).await;
  }
}

async fn start_ui(cli: Cli, app: &Arc<Mutex<App>>) -> Result<()> {
  // Terminal initialization
  let mut stdout = stdout();
  execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
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
