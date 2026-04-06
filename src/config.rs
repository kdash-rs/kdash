use std::{
  collections::BTreeMap,
  env,
  ffi::OsString,
  fs,
  io::ErrorKind,
  path::{Path, PathBuf},
};

use log::warn;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct KdashConfig {
  pub keybindings: Option<KeybindingOverrides>,
  pub theme: Option<ThemeConfig>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct KeybindingOverrides {
  #[serde(flatten)]
  pub values: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct ThemeConfig {
  pub dark: Option<BTreeMap<String, String>>,
  pub light: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadedConfig {
  pub config: KdashConfig,
  pub warning: Option<String>,
}

fn config_path_from(
  env_override: Option<OsString>,
  config_dir: Option<PathBuf>,
) -> Option<PathBuf> {
  env_override
    .filter(|path| !path.is_empty())
    .map(PathBuf::from)
    .or_else(|| config_dir.map(|dir| dir.join("kdash").join("config.yaml")))
}

pub fn config_path() -> Option<PathBuf> {
  config_path_from(env::var_os("KDASH_CONFIG"), dirs::config_dir())
}

fn parse_config(contents: &str, path: &Path) -> LoadedConfig {
  match serde_yaml::from_str::<KdashConfig>(contents) {
    Ok(config) => LoadedConfig {
      config,
      warning: None,
    },
    Err(error) => LoadedConfig {
      config: KdashConfig::default(),
      warning: Some(format!(
        "Failed to parse config file {}: {}. Using defaults.",
        path.display(),
        error
      )),
    },
  }
}

pub fn load_config_from_path(path: &Path) -> LoadedConfig {
  match fs::read_to_string(path) {
    Ok(contents) => parse_config(&contents, path),
    Err(error) if error.kind() == ErrorKind::NotFound => LoadedConfig::default(),
    Err(error) => LoadedConfig {
      config: KdashConfig::default(),
      warning: Some(format!(
        "Failed to read config file {}: {}. Using defaults.",
        path.display(),
        error
      )),
    },
  }
}

pub fn load_config() -> LoadedConfig {
  let loaded = config_path()
    .map(|path| load_config_from_path(&path))
    .unwrap_or_default();

  if let Some(warning) = &loaded.warning {
    warn!("{}", warning);
  }

  loaded
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
  };

  fn temp_test_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("system time should be after epoch")
      .as_nanos();
    let path = env::temp_dir().join(format!(
      "kdash-config-tests-{}-{}-{}",
      name,
      std::process::id(),
      suffix
    ));
    fs::create_dir_all(&path).expect("temp test dir should be created");
    path
  }

  #[test]
  fn test_config_path_from_prefers_env_override() {
    let path = config_path_from(
      Some(OsString::from("/tmp/custom-kdash.yaml")),
      Some(PathBuf::from("/tmp/ignored")),
    );

    assert_eq!(path, Some(PathBuf::from("/tmp/custom-kdash.yaml")));
  }

  #[test]
  fn test_config_path_from_uses_xdg_path() {
    let path = config_path_from(None, Some(PathBuf::from("/tmp/config-home")));

    assert_eq!(
      path,
      Some(PathBuf::from("/tmp/config-home/kdash/config.yaml"))
    );
  }

  #[test]
  fn test_load_config_from_path_reads_valid_config() {
    let dir = temp_test_dir("valid");
    let path = dir.join("config.yaml");
    fs::write(
      &path,
      "keybindings:\n  quit: ctrl+q\ntheme:\n  dark:\n    primary: green\n  light:\n    primary: blue\n",
    )
    .expect("config fixture should be written");

    let loaded = load_config_from_path(&path);

    assert_eq!(
      loaded
        .config
        .keybindings
        .as_ref()
        .map(|overrides| &overrides.values),
      Some(&BTreeMap::from([(
        "quit".to_string(),
        "ctrl+q".to_string()
      )]))
    );
    assert_eq!(
      loaded
        .config
        .theme
        .as_ref()
        .and_then(|theme| theme.dark.as_ref()),
      Some(&BTreeMap::from([(
        "primary".to_string(),
        "green".to_string()
      )]))
    );
    assert_eq!(
      loaded
        .config
        .theme
        .as_ref()
        .and_then(|theme| theme.light.as_ref()),
      Some(&BTreeMap::from([(
        "primary".to_string(),
        "blue".to_string()
      )]))
    );
    assert!(loaded.warning.is_none());

    fs::remove_dir_all(dir).expect("temp test dir should be removed");
  }

  #[test]
  fn test_load_config_from_path_missing_file_uses_defaults() {
    let dir = temp_test_dir("missing");
    let path = dir.join("missing.yaml");

    let loaded = load_config_from_path(&path);

    assert_eq!(loaded.config, KdashConfig::default());
    assert!(loaded.warning.is_none());

    fs::remove_dir_all(dir).expect("temp test dir should be removed");
  }

  #[test]
  fn test_load_config_from_path_malformed_yaml_uses_defaults_with_warning() {
    let dir = temp_test_dir("malformed");
    let path = dir.join("config.yaml");
    fs::write(&path, "keybindings: [").expect("config fixture should be written");

    let loaded = load_config_from_path(&path);

    assert_eq!(loaded.config, KdashConfig::default());
    assert!(loaded
      .warning
      .as_deref()
      .is_some_and(|warning| warning.contains("Failed to parse config file")));

    fs::remove_dir_all(dir).expect("temp test dir should be removed");
  }

  #[test]
  fn test_load_config_from_path_partial_config_uses_missing_defaults() {
    let dir = temp_test_dir("partial");
    let path = dir.join("config.yaml");
    fs::write(&path, "keybindings:\n  quit: ctrl+q\n").expect("config fixture should be written");

    let loaded = load_config_from_path(&path);

    assert!(loaded.config.keybindings.is_some());
    assert!(loaded.config.theme.is_none());
    assert!(loaded.warning.is_none());

    fs::remove_dir_all(dir).expect("temp test dir should be removed");
  }
}
