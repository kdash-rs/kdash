use clap::{App as ClapApp, Arg};

use super::banner::BANNER;

/// kdash CLI
pub struct Cli {
  /// time in ms between two ticks.
  pub tick_rate: u64,
  /// time in ms between two network calls.
  pub poll_rate: u64,
  /// whether unicode symbols are used to improve the overall look of the app
  pub enhanced_graphics: bool,
}

impl Cli {
  pub fn new() -> Cli {
    Cli {
      tick_rate: 250,  // 250 ms
      poll_rate: 5000, // 5 seconds
      enhanced_graphics: true,
    }
  }

  /// create a new clapapp instance
  pub fn get_clap_app<'a>(&mut self) -> ClapApp<'a> {
    ClapApp::new(env!("CARGO_PKG_NAME"))
      .version(env!("CARGO_PKG_VERSION"))
      .author(env!("CARGO_PKG_AUTHORS"))
      .about(env!("CARGO_PKG_DESCRIPTION"))
      .override_usage("Press `?` while running the app to see keybindings")
      .before_help(BANNER)
      .arg(
        Arg::new("tick-rate")
          .short('t')
          .long("tick-rate")
          .help("Set the tick rate (milliseconds): the lower the number the higher the FPS.")
          .takes_value(true),
      )
      .arg(
        Arg::new("poll-rate")
          .short('p')
          .long("poll-rate")
          .help("Set the network call polling rate (milliseconds, should be multiples of tick-rate): the lower the number the higher the network calls.")
          .takes_value(true),
      )
  }
}
