use serde::{Deserialize, Serialize};

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
}
