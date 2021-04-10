use serde::{Deserialize, Serialize};

const DEFAULT_PORT: u16 = 8888;

#[derive(Default, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClientConfig {
  pub client_url: String,
  pub port: Option<u16>,
}

impl ClientConfig {
  pub fn new() -> ClientConfig {
    ClientConfig {
      client_url: "".to_string(),
      port: None,
    }
  }

  pub fn get_port(&self) -> u16 {
    self.port.unwrap_or(DEFAULT_PORT)
  }
}
