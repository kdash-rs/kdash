mod app;
mod banner;
mod config;
mod event;
mod network;
mod ui;
mod util;

use crate::app::RouteId;
use crate::event::Key;
use app::{ActiveBlock, App};
use banner::BANNER;
use config::ClientConfig;
use network::{IoEvent, Network};

use anyhow::{anyhow, Result};
use backtrace::Backtrace;
use clap::{App as ClapApp, Arg, Shell};
use crossterm::{
  event::{DisableMouseCapture, EnableMouseCapture},
  execute,
  style::Print,
  terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use kube::Client;
use std::{
  cmp::{max, min},
  io::{self, stdout},
  panic::{self, PanicInfo},
  path::PathBuf,
  sync::Arc,
  time::SystemTime,
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
  /// whether unicode symbols are used to improve the overall look of the app
  enhanced_graphics: bool,
}

// shutdown the CLI and show terminal
fn close_application() -> Result<()> {
  disable_raw_mode()?;
  let mut stdout = io::stdout();
  execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
  Ok(())
}
// fn shutdown(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> Result<(), Box<dyn Error>> {
//   disable_raw_mode()?;
//   execute!(
//     terminal.backend_mut(),
//     LeaveAlternateScreen,
//     DisableMouseCapture
//   )?;
//   terminal.show_cursor()?;
//   Ok(())
// }

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

  let mut clap_app = ClapApp::new(env!("CARGO_PKG_NAME"))
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
        .long_help(
          "Specify the tick rate in milliseconds: the lower the number the \
higher the FPS. It can be nicer to have a lower value when you want to use the audio analysis view \
of the app. Beware that this comes at a CPU cost!",
        )
        .takes_value(true),
    );

  let mut cli: Cli = Cli {
    tick_rate: 250,
    enhanced_graphics: true,
  };
  let matches = clap_app.clone().get_matches();

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

  let mut client_config = ClientConfig::new();

  let (sync_io_tx, sync_io_rx) = std::sync::mpsc::channel::<IoEvent>();

  // Initialise app state
  let app = Arc::new(Mutex::new(App::new(sync_io_tx, cli.enhanced_graphics)));

  // Launch the UI (async)
  let cloned_app = Arc::clone(&app);
  let client = Client::try_default().await?;

  std::thread::spawn(move || {
    let mut network = Network::new(client.clone(), client_config, &app);
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

  // play music on, if not send them to the device selection view

  let mut is_first_render = true;

  loop {
    let mut app = app.lock().await;
    // Get the size of the screen on each loop to account for resize event
    if let Ok(size) = terminal.backend().size() {
      // Reset the help menu is the terminal was resized
      if is_first_render || app.size != size {
        app.help_menu_max_lines = 0;
        app.help_menu_offset = 0;
        app.help_menu_page = 0;

        app.size = size;

        // Based on the size of the terminal, adjust the search limit.
        let potential_limit = max((app.size.height as i32) - 13, 0) as u32;
        let max_limit = min(potential_limit, 50);
        // let large_search_limit = min((f32::from(size.height) / 1.4) as u32, max_limit);
        // let small_search_limit = min((f32::from(size.height) / 2.85) as u32, max_limit / 2);

        // app.dispatch(IoEvent::UpdateSearchLimits(
        //   large_search_limit,
        //   small_search_limit,
        // ));

        // Based on the size of the terminal, adjust how many lines are
        // dislayed in the help menu
        if app.size.height > 8 {
          app.help_menu_max_lines = (app.size.height as u32) - 8;
        } else {
          app.help_menu_max_lines = 0;
        }
      }
    };

    // let current_route = app.get_current_route();
    terminal.draw(|f| ui::draw(f, &mut app))?;

    // terminal.draw(|mut f| match current_route.active_block {
    //   ActiveBlock::HelpMenu => {
    //     ui::draw_help_menu(&mut f, &app);
    //   }
    //   ActiveBlock::Error => {
    //     ui::draw_error_screen(&mut f, &app);
    //   }
    //   ActiveBlock::SelectDevice => {
    //     ui::draw_device_list(&mut f, &app);
    //   }
    //   ActiveBlock::Analysis => {
    //     ui::audio_analysis::draw(&mut f, &app);
    //   }
    //   ActiveBlock::BasicView => {
    //     ui::draw_basic_view(&mut f, &app);
    //   }
    //   _ => {
    //     ui::draw_main_layout(&mut f, &app);
    //   }
    // })?;

    // if current_route.active_block == ActiveBlock::Input {
    //   terminal.show_cursor()?;
    // } else {
    //   terminal.hide_cursor()?;
    // }

    // let cursor_offset = if app.size.height > ui::util::SMALL_TERMINAL_HEIGHT {
    //   2
    // } else {
    //   1
    // };

    // // Put the cursor back inside the input box
    // terminal.backend_mut().execute(MoveTo(
    //   cursor_offset + app.input_cursor_position,
    //   cursor_offset,
    // ))?;

    match events.next()? {
      event::Event::Input(key) => {
        // handle CTRL + C
        if key == Key::Ctrl('c') {
          break;
        }
        // quit when q is pressed
        if key == Key::Char('q') {
          break;
        }

        match key {
          Key::Left => app.on_left(),
          Key::Right => app.on_right(),
          Key::Up => app.on_up(),
          Key::Down => app.on_down(),
          _ => (),
        }

        let current_active_block = app.get_current_route().active_block;

        // To avoid swallowing the global key presses `q` and `-` make a special
        // case for the input handler
        // if current_active_block == ActiveBlock::Input {
        //   handlers::input_handler(key, &mut app);
        // } else if key == app.user_config.keys.back {
        //   if app.get_current_route().active_block != ActiveBlock::Input {
        //     // Go back through navigation stack when not in search input mode and exit the app if there are no more places to back to

        //     let pop_result = match app.pop_navigation_stack() {
        //       Some(ref x) if x.id == RouteId::Search => app.pop_navigation_stack(),
        //       Some(x) => Some(x),
        //       None => None,
        //     };
        //     if pop_result.is_none() {
        //       break; // Exit application
        //     }
        //   }
        // } else {
        //   handlers::handle_app(key, &mut app);
        // }
      }
      event::Event::Tick => {
        app.on_tick();
      }
    }

    // Delay requests until first render, will have the effect of improving
    // startup speed
    if is_first_render {
      //   app.dispatch(IoEvent::GetPods);
      //   app.help_docs_size = ui::help::get_help_docs(&app.user_config.keys).len() as u32;

      is_first_render = false;
    }

    if app.should_quit {
      break;
    }
  }

  terminal.show_cursor()?;
  close_application()?;

  Ok(())
}
