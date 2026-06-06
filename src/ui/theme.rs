use std::{collections::BTreeMap, sync::OnceLock};

use log::warn;
use ratatui::style::Color;

use crate::config::KdashConfig;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ThemeOverrides {
  dark: BTreeMap<String, Color>,
  light: BTreeMap<String, Color>,
}

static ACTIVE_THEME_OVERRIDES: OnceLock<ThemeOverrides> = OnceLock::new();

const THEME_KEYS: &[&str] = &[
  "text",
  "failure",
  "warning",
  "success",
  "primary",
  "secondary",
  "help",
  "background",
];

pub fn initialize_theme(config: &KdashConfig) -> Vec<String> {
  let (overrides, warnings) = build_theme_overrides(config);
  let _ = ACTIVE_THEME_OVERRIDES.set(overrides);

  for warning in &warnings {
    warn!("{}", warning);
  }

  warnings
}

pub fn override_color(name: &str, light: bool) -> Option<Color> {
  ACTIVE_THEME_OVERRIDES.get().and_then(|overrides| {
    let colors = if light {
      &overrides.light
    } else {
      &overrides.dark
    };
    colors.get(name).copied()
  })
}

fn build_theme_overrides(config: &KdashConfig) -> (ThemeOverrides, Vec<String>) {
  let mut overrides = ThemeOverrides::default();
  let mut warnings = vec![];

  if let Some(theme) = &config.theme {
    load_theme_section(
      &mut overrides.dark,
      theme.dark.as_ref(),
      "dark",
      &mut warnings,
    );
    load_theme_section(
      &mut overrides.light,
      theme.light.as_ref(),
      "light",
      &mut warnings,
    );
  }

  (overrides, warnings)
}

fn load_theme_section(
  target: &mut BTreeMap<String, Color>,
  values: Option<&BTreeMap<String, String>>,
  section: &str,
  warnings: &mut Vec<String>,
) {
  let Some(values) = values else {
    return;
  };

  for (key, value) in values {
    if !THEME_KEYS.contains(&key.as_str()) {
      warnings.push(format!("Unknown {} theme override: {}", section, key));
      continue;
    }

    match parse_color(value) {
      Ok(color) => {
        target.insert(key.clone(), color);
      }
      Err(error) => warnings.push(format!(
        "Invalid {} color override for {}: {} ({})",
        section, key, value, error
      )),
    }
  }
}

fn parse_color(value: &str) -> Result<Color, String> {
  let normalized = value.trim().to_lowercase();
  match normalized.as_str() {
    "black" => Ok(Color::Black),
    "red" => Ok(Color::Red),
    "green" => Ok(Color::Green),
    "yellow" => Ok(Color::Yellow),
    "blue" => Ok(Color::Blue),
    "magenta" => Ok(Color::Magenta),
    "cyan" => Ok(Color::Cyan),
    "gray" | "grey" => Ok(Color::Gray),
    "darkgray" | "darkgrey" | "dark_gray" | "dark_grey" => Ok(Color::DarkGray),
    "lightred" | "light_red" => Ok(Color::LightRed),
    "lightgreen" | "light_green" => Ok(Color::LightGreen),
    "lightyellow" | "light_yellow" => Ok(Color::LightYellow),
    "lightblue" | "light_blue" => Ok(Color::LightBlue),
    "lightmagenta" | "light_magenta" => Ok(Color::LightMagenta),
    "lightcyan" | "light_cyan" => Ok(Color::LightCyan),
    "white" => Ok(Color::White),
    "reset" | "default" => Ok(Color::Reset),
    _ => parse_hex_color(&normalized),
  }
}

fn parse_hex_color(value: &str) -> Result<Color, String> {
  let hex = value
    .strip_prefix('#')
    .ok_or_else(|| format!("unsupported color '{}'", value))?;

  if hex.len() != 6 {
    return Err(format!("hex color '{}' must be 6 characters", value));
  }

  let red = u8::from_str_radix(&hex[0..2], 16)
    .map_err(|_| format!("invalid red channel in '{}'", value))?;
  let green = u8::from_str_radix(&hex[2..4], 16)
    .map_err(|_| format!("invalid green channel in '{}'", value))?;
  let blue = u8::from_str_radix(&hex[4..6], 16)
    .map_err(|_| format!("invalid blue channel in '{}'", value))?;

  Ok(Color::Rgb(red, green, blue))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::{KdashConfig, ThemeConfig};

  #[test]
  fn test_build_theme_overrides_reads_valid_colors() {
    let config = KdashConfig {
      theme: Some(ThemeConfig {
        dark: Some(BTreeMap::from([
          ("primary".into(), "green".into()),
          ("background".into(), "#112233".into()),
        ])),
        light: Some(BTreeMap::from([("primary".into(), "blue".into())])),
      }),
      ..Default::default()
    };

    let (overrides, warnings) = build_theme_overrides(&config);

    assert_eq!(overrides.dark.get("primary"), Some(&Color::Green));
    assert_eq!(
      overrides.dark.get("background"),
      Some(&Color::Rgb(0x11, 0x22, 0x33))
    );
    assert_eq!(overrides.light.get("primary"), Some(&Color::Blue));
    assert!(warnings.is_empty());
  }

  #[test]
  fn test_build_theme_overrides_warns_on_invalid_or_unknown_values() {
    let config = KdashConfig {
      theme: Some(ThemeConfig {
        dark: Some(BTreeMap::from([
          ("primary".into(), "not-a-color".into()),
          ("made_up".into(), "green".into()),
        ])),
        light: Some(BTreeMap::from([("also_bad".into(), "green".into())])),
      }),
      ..Default::default()
    };

    let (_, warnings) = build_theme_overrides(&config);

    assert_eq!(warnings.len(), 3);
    assert!(warnings.iter().any(|warning| warning.contains("primary")));
    assert!(warnings.iter().any(|warning| warning.contains("made_up")));
    assert!(warnings.iter().any(|warning| warning.contains("also_bad")));
  }
}
