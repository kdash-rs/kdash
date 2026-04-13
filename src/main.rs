#![warn(rust_2018_idioms)]
#[deny(clippy::shadow_unrelated)]
mod app;
mod banner;
mod cmd;
mod config;
mod event;
mod handlers;
mod network;
mod ui;

use std::{
  fs::File,
  io::{self, stdout, Stdout},
  panic::{self, PanicHookInfo},
  sync::Arc,
};

use anyhow::{anyhow, Result};
use app::{key_binding::initialize_keybindings, App, DEFAULT_LOG_TAIL_LINES};
use banner::BANNER;
use chrono::{self};
use clap::{builder::PossibleValuesParser, Parser};
use cmd::{
  shell::{prepare_shell_exec, run_shell_exec, ShellExecTarget},
  CmdRunner, IoCmdEvent,
};
use config::load_config;
use crossterm::{
  event::{KeyEvent, MouseEvent},
  execute,
  terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use event::Key;
use log::{info, warn, LevelFilter, SetLoggerError};
use network::{
  get_client,
  stream::{IoStreamEvent, NetworkStream},
  IoEvent, Network,
};
use ratatui::{
  backend::{Backend, CrosstermBackend},
  Terminal,
};
use simplelog::{Config, WriteLogger};
use tokio::sync::{mpsc, Mutex};
use ui::theme::initialize_theme;

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
  /// Enables debug mode and writes logs to 'kdash-debug-<timestamp>.log' file in the current directory.
  /// Default behavior is to write INFO logs. Pass a log level to overwrite the default.
  #[arg(
    name = "debug",
    short,
    long,
    default_missing_value = "Info",
    require_equals = true,
    num_args = 0..=1,
    ignore_case = true,
    value_parser = PossibleValuesParser::new(&["info", "debug", "trace", "warn", "error"])
  )]
  pub debug: Option<String>,
  /// Set how many historical log lines to fetch before live streaming starts.
  #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
  pub log_tail_lines: Option<u32>,
}

#[tokio::main]
async fn main() -> Result<()> {
  // SAFETY: safe as this is called once at startup before spawning threads
  unsafe { openssl_probe::try_init_openssl_env_vars() };
  panic::set_hook(Box::new(|info| {
    panic_hook(info);
  }));

  // parse CLI arguments
  let cli = Cli::parse();

  // Setup logging if debug flag is set
  if cli.debug.is_some() {
    setup_logging(cli.debug.clone())?;
    info!(
      "Debug mode is enabled. Level: {}, KDash version: {}",
      cli.debug.clone().unwrap(),
      env!("CARGO_PKG_VERSION")
    );
  }

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
  let loaded_config = load_config();
  let log_tail_lines = resolve_log_tail_lines(cli.log_tail_lines, &loaded_config.config);
  let mut config_warnings = vec![];
  if let Some(warning) = loaded_config.warning.clone() {
    config_warnings.push(warning);
  }
  config_warnings.extend(initialize_keybindings(&loaded_config.config));
  config_warnings.extend(initialize_theme(&loaded_config.config));

  // Initialize app state
  let app = Arc::new(Mutex::new(App::new(
    sync_io_tx,
    sync_io_stream_tx,
    sync_io_cmd_tx,
    cli.enhanced_graphics,
    cli.poll_rate / cli.tick_rate,
    log_tail_lines,
    loaded_config.config,
  )));

  {
    let app = app.lock().await;
    if app.config.keybindings.is_some() || app.config.theme.is_some() {
      info!("Loaded config overrides from file");
    }
  }

  if !config_warnings.is_empty() {
    let mut app = app.lock().await;
    app.handle_error(anyhow!(config_warnings.join(" | ")));
  }

  // Launch network, stream, and cmd tasks on a dedicated tokio runtime running
  // on its own OS thread.  This keeps all network I/O off the main runtime so
  // the UI loop is never starved of CPU time by long-running API calls.
  let app_nw = Arc::clone(&app);
  let app_stream = Arc::clone(&app);
  let app_cli = Arc::clone(&app);

  std::thread::spawn(move || {
    let rt = tokio::runtime::Builder::new_multi_thread()
      .enable_all()
      .thread_name("kdash-network")
      .build()
      .expect("Failed to create network runtime");

    rt.block_on(async move {
      tokio::spawn(async move {
        info!("Starting network task");
        start_network(sync_io_rx, &app_nw).await;
      });

      tokio::spawn(async move {
        info!("Starting network stream task");
        start_stream_network(sync_io_stream_rx, &app_stream).await;
      });

      tokio::spawn(async move {
        info!("Starting cmd runner task");
        start_cmd_runner(sync_io_cmd_rx, &app_cli).await;
      });

      // Keep this runtime alive until all tasks complete.
      // When the UI exits and drops the channel senders, recv() returns None
      // and the tasks finish naturally.
      tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl_c");
    });
  });

  // Launch the UI on the main runtime — it owns the terminal and must run here
  start_ui(cli, &app).await?;

  Ok(())
}

async fn start_network(mut io_rx: mpsc::Receiver<IoEvent>, app: &Arc<Mutex<App>>) {
  match get_client(None).await {
    Ok(client) => {
      let mut network = Network::new(client, app);

      while let Some(io_event) = io_rx.recv().await {
        info!("Network event received: {:?}", io_event);
        network.handle_network_event(io_event).await;
      }
    }
    Err(e) => {
      let mut app = app.lock().await;
      app.handle_error(anyhow!("Unable to obtain Kubernetes client. {}", e));
    }
  }
}

fn resolve_log_tail_lines(cli_value: Option<u32>, config: &config::KdashConfig) -> u32 {
  cli_value
    .or(config.log_tail_lines)
    .unwrap_or(DEFAULT_LOG_TAIL_LINES)
}

async fn start_stream_network(mut io_rx: mpsc::Receiver<IoStreamEvent>, app: &Arc<Mutex<App>>) {
  match get_client(None).await {
    Ok(client) => {
      let mut network = NetworkStream::new(client, app);

      while let Some(io_event) = io_rx.recv().await {
        info!("Network stream event received: {:?}", io_event);
        network.handle_network_stream_event(io_event).await;
      }
    }
    Err(e) => {
      let mut app = app.lock().await;
      app.handle_error(anyhow!("Unable to obtain Kubernetes client. {}", e));
    }
  }
}

async fn start_cmd_runner(mut io_rx: mpsc::Receiver<IoCmdEvent>, app: &Arc<Mutex<App>>) {
  let mut cmd = CmdRunner::new(app);

  while let Some(io_event) = io_rx.recv().await {
    info!("Cmd event received: {:?}", io_event);
    cmd.handle_cmd_event(io_event).await;
  }
}

/// Process a single UI event.  Returns `true` when the app should exit (Ctrl+C).
async fn process_event(
  app: &mut App,
  ev: event::Event<KeyEvent, MouseEvent>,
  is_first_render: &mut bool,
) -> bool {
  match ev {
    event::Event::Input(key_event) => {
      let key = Key::from(key_event);
      if key == Key::Ctrl('c') {
        true
      } else {
        handlers::handle_key_events(key, key_event, app).await;
        false
      }
    }
    event::Event::MouseInput(mouse) => {
      handlers::handle_mouse_events(mouse, app).await;
      false
    }
    event::Event::Tick => {
      app.on_tick(*is_first_render).await;
      *is_first_render = false;
      false
    }
    event::Event::KubeConfigChange => {
      info!("Kubeconfig change detected, reloading");
      app.dispatch(IoEvent::GetKubeConfig).await;
      false
    }
  }
}

async fn start_ui(cli: Cli, app: &Arc<Mutex<App>>) -> Result<()> {
  info!("Starting UI");
  // see https://docs.rs/crossterm/0.17.7/crossterm/terminal/#raw-mode
  enable_raw_mode()?;
  // Terminal initialization
  let mut stdout = stdout();
  // not capturing mouse to make text select/copy possible
  execute!(stdout, EnterAlternateScreen)?;
  // terminal backend for cross platform support
  let backend = CrosstermBackend::new(stdout);
  let mut terminal = Terminal::new(backend)?;
  terminal.clear()?;
  terminal.hide_cursor()?;
  // custom events
  let mut events = event::Events::new(cli.tick_rate);
  let mut is_first_render = true;
  // Perform initial draw so the user sees the UI immediately
  {
    let mut app = app.lock().await;
    if let Ok(size) = terminal.backend().size() {
      app.size.width = size.width;
      app.size.height = size.height;
    }
    terminal.draw(|f| ui::draw(f, &mut app))?;
  }
  // main UI loop
  loop {
    // Wait for the next event BEFORE acquiring the lock.
    // This is the blocking call — no reason to hold the mutex while waiting.
    let event = events.next()?;

    let (pending_shell_exec, should_quit) = {
      let mut app = app.lock().await;

      // Handle events BEFORE drawing so the frame always reflects
      // the latest state, eliminating the 1-event visual lag.

      // Process the blocking event
      let mut should_break = process_event(&mut app, event, &mut is_first_render).await;

      // Drain any pending events so rapid key-presses are batched into a
      // single render pass instead of each triggering a stale redraw.
      if !should_break {
        for _ in 0..20 {
          match events.try_next() {
            Some(ev) => {
              should_break = process_event(&mut app, ev, &mut is_first_render).await;
              if should_break {
                break;
              }
            }
            None => break,
          }
        }
      }

      if should_break {
        break;
      }

      // Get the size of the screen on each loop to account for resize events
      if let Ok(size) = terminal.backend().size() {
        if app.refresh || app.size.as_size() != size {
          app.size.width = size.width;
          app.size.height = size.height;
        }
      }

      // Draw the UI layout AFTER processing events so the frame is up-to-date
      terminal.draw(|f| ui::draw(f, &mut app))?;

      is_first_render = false;
      let pending_shell_exec = app.take_pending_shell_exec();
      let should_quit = app.should_quit;
      (pending_shell_exec, should_quit)
    };

    if let Some(request) = pending_shell_exec {
      drop(events);
      execute_pending_shell_exec(app, &mut terminal, request).await?;
      events = event::Events::new(cli.tick_rate);
    }

    if should_quit {
      break;
    }
  }

  terminal.show_cursor()?;
  shutdown(terminal)?;
  Ok(())
}

// shutdown the CLI and show terminal
fn shutdown(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
  info!("Shutting down");
  disable_raw_mode()?;
  execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
  terminal.show_cursor()?;
  Ok(())
}

fn suspend_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
  disable_raw_mode()?;
  execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
  terminal.show_cursor()?;
  Ok(())
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
  enable_raw_mode()?;
  execute!(terminal.backend_mut(), EnterAlternateScreen)?;
  terminal.hide_cursor()?;
  terminal.clear()?;
  Ok(())
}

trait ShellTerminal {
  fn suspend(&mut self) -> Result<()>;
  fn restore(&mut self) -> Result<()>;
}

impl ShellTerminal for Terminal<CrosstermBackend<Stdout>> {
  fn suspend(&mut self) -> Result<()> {
    suspend_terminal(self)
  }

  fn restore(&mut self) -> Result<()> {
    restore_terminal(self)
  }
}

async fn execute_pending_shell_exec(
  app: &Arc<Mutex<App>>,
  terminal: &mut Terminal<CrosstermBackend<Stdout>>,
  request: app::PendingShellExec,
) -> Result<()> {
  execute_pending_shell_exec_with(app, terminal, request, |request| {
    let target = ShellExecTarget {
      namespace: request.namespace,
      pod: request.pod,
      container: request.container,
    };
    let command = prepare_shell_exec(&target).map_err(|error| anyhow!(error.to_string()))?;
    let shell = command.shell.clone();
    run_shell_exec(&command).map_err(|error| anyhow!(error.to_string()))?;
    Ok(shell)
  })
  .await
}

async fn execute_pending_shell_exec_with<F, T>(
  app: &Arc<Mutex<App>>,
  terminal: &mut T,
  request: app::PendingShellExec,
  run_shell: F,
) -> Result<()>
where
  F: FnOnce(app::PendingShellExec) -> Result<String>,
  T: ShellTerminal,
{
  terminal.suspend()?;
  let shell_result = run_shell(request.clone());
  let restore_result = terminal.restore();

  let mut app = app.lock().await;

  if let Err(error) = restore_result {
    app.handle_error(anyhow!(
      "Unable to restore terminal after shell exec: {}",
      error
    ));
    return Err(error);
  }

  match shell_result {
    Ok(shell) => {
      app.set_status_message(format!(
        "Closed {} shell for {}/{}",
        shell, request.pod, request.container
      ));
      Ok(())
    }
    Err(error) => {
      app.handle_error(anyhow!(
        "Unable to open shell for {}/{}: {}",
        request.pod,
        request.container,
        error
      ));
      Ok(())
    }
  }
}

fn setup_logging(debug: Option<String>) -> Result<(), SetLoggerError> {
  let log_file = format!(
    "./kdash-debug-{}.log",
    chrono::Local::now().format("%Y%m%d%H%M%S")
  );
  let log_level = debug
    .map(|level| match level.to_lowercase().as_str() {
      "debug" => LevelFilter::Debug,
      "trace" => LevelFilter::Trace,
      "warn" => LevelFilter::Warn,
      "error" => LevelFilter::Error,
      _ => LevelFilter::Info,
    })
    .unwrap_or_else(|| LevelFilter::Info);

  WriteLogger::init(
    log_level,
    Config::default(),
    File::create(log_file).unwrap(),
  )
}

#[cfg(debug_assertions)]
fn panic_hook(info: &PanicHookInfo<'_>) {
  use backtrace::Backtrace;
  use crossterm::style::Print;

  let (msg, location) = get_panic_info(info);

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
fn panic_hook(info: &PanicHookInfo<'_>) {
  use backtrace::Backtrace;
  use crossterm::style::Print;
  use human_panic::{handle_dump, print_msg, Metadata};
  use log::error;

  let meta = Metadata::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
    .authors(env!("CARGO_PKG_AUTHORS").replace(':', ", "))
    .homepage(env!("CARGO_PKG_HOMEPAGE"));

  let file_path = handle_dump(&meta, info);
  let (msg, location) = get_panic_info(info);
  let stacktrace: String = format!("{:?}", Backtrace::new()).replace('\n', "\n\r");

  error!(
    "thread '<unnamed>' panicked at '{}', {}\n\r{}",
    msg, location, stacktrace
  );

  disable_raw_mode().unwrap();
  execute!(
    io::stdout(),
    LeaveAlternateScreen,
    Print(format!("Error: '{}' at {}\n", msg, location)),
  )
  .unwrap();
  print_msg(file_path, &meta).expect("human-panic: printing error message to console failed");
}

fn get_panic_info(info: &PanicHookInfo<'_>) -> (String, String) {
  let location = info.location().unwrap();

  let msg = match info.payload().downcast_ref::<&'static str>() {
    Some(s) => *s,
    None => match info.payload().downcast_ref::<String>() {
      Some(s) => &s[..],
      None => "Box<Any>",
    },
  };

  (msg.to_string(), format!("{}", location))
}

#[cfg(test)]
mod tests {
  use super::{execute_pending_shell_exec_with, resolve_log_tail_lines};
  use crate::{app::App, config::KdashConfig};
  use anyhow::anyhow;
  use std::sync::Arc;
  use tokio::sync::Mutex;

  struct StubTerminal;

  impl super::ShellTerminal for StubTerminal {
    fn suspend(&mut self) -> anyhow::Result<()> {
      Ok(())
    }

    fn restore(&mut self) -> anyhow::Result<()> {
      Ok(())
    }
  }

  #[test]
  fn test_resolve_log_tail_lines_uses_default() {
    assert_eq!(resolve_log_tail_lines(None, &KdashConfig::default()), 100);
  }

  #[test]
  fn test_resolve_log_tail_lines_uses_config_when_cli_missing() {
    let config = KdashConfig {
      log_tail_lines: Some(250),
      ..KdashConfig::default()
    };

    assert_eq!(resolve_log_tail_lines(None, &config), 250);
  }

  #[test]
  fn test_resolve_log_tail_lines_prefers_cli() {
    let config = KdashConfig {
      log_tail_lines: Some(250),
      ..KdashConfig::default()
    };

    assert_eq!(resolve_log_tail_lines(Some(500), &config), 500);
  }

  #[tokio::test]
  async fn test_execute_pending_shell_exec_with_sets_success_status_and_clears_request() {
    let app = Arc::new(Mutex::new(App::default()));
    let mut terminal = StubTerminal;

    let result = execute_pending_shell_exec_with(
      &app,
      &mut terminal,
      crate::app::PendingShellExec {
        namespace: "default".into(),
        pod: "api-123".into(),
        container: "web".into(),
      },
      |_| Ok("/bin/sh".into()),
    )
    .await;

    assert!(result.is_ok());

    let app = app.lock().await;
    assert!(app.api_error.is_empty());
    assert_eq!(
      app.status_message.text(),
      "Closed /bin/sh shell for api-123/web"
    );
    assert!(app.pending_shell_exec().is_none());
  }

  #[tokio::test]
  async fn test_execute_pending_shell_exec_with_reports_shell_errors_after_restoring_terminal() {
    let app = Arc::new(Mutex::new(App::default()));
    let mut terminal = StubTerminal;

    let result = execute_pending_shell_exec_with(
      &app,
      &mut terminal,
      crate::app::PendingShellExec {
        namespace: "default".into(),
        pod: "api-123".into(),
        container: "web".into(),
      },
      |_| Err(anyhow!("probe failed")),
    )
    .await;

    assert!(result.is_ok());

    let app = app.lock().await;
    assert_eq!(
      app.api_error,
      "Unable to open shell for api-123/web: probe failed"
    );
    assert!(app.status_message.is_empty());
  }
}
