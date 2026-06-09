use std::{collections::BTreeMap, fmt, str::FromStr, sync::OnceLock};

use log::warn;
use ratatui::style::Color;
use serde::{Deserialize, Deserializer};

use crate::config::KdashConfig;

/// Named themes shipped with KDash.
///
/// String forms accept both the short name and the conventional long name
/// where one exists (e.g. `macchiato` and `catppuccin-macchiato`). Mirrors
/// the theme set in the sibling LlamaStash project so users moving between
/// the two see the same palettes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ThemeName {
  #[default]
  Macchiato,
  Latte,
  GruvboxDark,
  SolarizedDark,
  Mono,
  /// User-defined theme loaded from `config.yaml`'s `custom_theme:` block.
  /// The concrete palette is built at startup and carried on the `App`;
  /// `palette_for(Custom)` returns the macchiato palette as a benign
  /// fallback for code paths that don't have an `App` in scope.
  Custom,
}

impl ThemeName {
  /// Every built-in theme in cycle order. `Custom` is appended only when a
  /// custom palette is actually loaded (handled by the caller).
  pub const ALL: [ThemeName; 6] = [
    ThemeName::Macchiato,
    ThemeName::Latte,
    ThemeName::GruvboxDark,
    ThemeName::SolarizedDark,
    ThemeName::Mono,
    ThemeName::Custom,
  ];

  /// Canonical kebab-case identifier (used in config files / CLI args).
  pub fn canonical(self) -> &'static str {
    match self {
      ThemeName::Macchiato => "macchiato",
      ThemeName::Latte => "latte",
      ThemeName::GruvboxDark => "gruvbox-dark",
      ThemeName::SolarizedDark => "solarized-dark",
      ThemeName::Mono => "mono",
      ThemeName::Custom => "custom",
    }
  }
}

impl fmt::Display for ThemeName {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.canonical())
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownThemeError {
  pub value: String,
}

impl fmt::Display for UnknownThemeError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let valid: Vec<&str> = ThemeName::ALL.iter().map(|t| t.canonical()).collect();
    write!(
      f,
      "unknown theme '{}' (valid: {})",
      self.value,
      valid.join(", ")
    )
  }
}

impl std::error::Error for UnknownThemeError {}

impl FromStr for ThemeName {
  type Err = UnknownThemeError;

  fn from_str(input: &str) -> Result<Self, Self::Err> {
    match input.trim().to_lowercase().as_str() {
      "macchiato" | "catppuccin-macchiato" => Ok(ThemeName::Macchiato),
      "latte" | "catppuccin-latte" => Ok(ThemeName::Latte),
      "gruvbox-dark" | "gruvbox" => Ok(ThemeName::GruvboxDark),
      "solarized-dark" | "solarized" => Ok(ThemeName::SolarizedDark),
      "mono" | "monochrome" => Ok(ThemeName::Mono),
      "custom" => Ok(ThemeName::Custom),
      _ => Err(UnknownThemeError {
        value: input.to_string(),
      }),
    }
  }
}

impl<'de> Deserialize<'de> for ThemeName {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let raw = String::deserialize(deserializer)?;
    ThemeName::from_str(&raw).map_err(serde::de::Error::custom)
  }
}

/// A self-contained colour palette used by the TUI.
///
/// Slots are *semantic*, not visual: `accent` is the panel-border /
/// primary-action colour, `secondary` is the panel-title colour, `label`
/// is the table-column / field-label colour, and `muted` is the help /
/// hint tone. Renderers pick a slot by meaning so theme swaps don't
/// require call-site changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
  pub name: ThemeName,
  pub is_dark: bool,
  pub bg: Color,
  pub fg: Color,
  /// Primary action / panel borders (unfocused).
  pub accent: Color,
  /// Panel titles (focused and unfocused).
  pub secondary: Color,
  /// Field labels and table column headers.
  pub label: Color,
  /// Help / hint / divider text.
  pub muted: Color,
  pub success: Color,
  pub warning: Color,
  pub error: Color,
  /// Focused/active panel border tone (gold/amber on catppuccin).
  pub highlight: Color,
}

impl Palette {
  /// Border colour for focus-aware panes. Focused panes adopt
  /// `highlight`; the fallback to `accent` covers themes whose
  /// `highlight` is `Color::Reset` (Mono opts out so the focused
  /// border stays visible against the terminal default).
  pub fn focus_border(&self, focused: bool) -> Color {
    if focused && self.highlight != Color::Reset {
      self.highlight
    } else {
      self.accent
    }
  }
}

// ─── Built-in palettes ────────────────────────────────────────────────

// Catppuccin Macchiato — https://catppuccin.com/palette
const MACCHIATO: Palette = Palette {
  name: ThemeName::Macchiato,
  is_dark: true,
  bg: Color::Rgb(0x24, 0x27, 0x3A),
  fg: Color::Rgb(0xCA, 0xD3, 0xF5),
  accent: Color::Rgb(0xC6, 0xA0, 0xF6),
  secondary: Color::Rgb(0xEE, 0xD4, 0x9F),
  label: Color::Rgb(0x8A, 0xAD, 0xF4),
  muted: Color::Rgb(0xA5, 0xAD, 0xCB),
  success: Color::Rgb(0xA6, 0xDA, 0x95),
  warning: Color::Rgb(0xF5, 0xA9, 0x7F),
  error: Color::Rgb(0xED, 0x87, 0x96),
  highlight: Color::Rgb(0xEE, 0xD4, 0x9F),
};

// Catppuccin Latte — https://catppuccin.com/palette
const LATTE: Palette = Palette {
  name: ThemeName::Latte,
  is_dark: false,
  bg: Color::Rgb(0xEF, 0xF1, 0xF5),
  fg: Color::Rgb(0x4C, 0x4F, 0x69),
  accent: Color::Rgb(0x88, 0x39, 0xEF),
  secondary: Color::Rgb(0xDF, 0x8E, 0x1D),
  label: Color::Rgb(0x1E, 0x66, 0xF5),
  muted: Color::Rgb(0x6C, 0x6F, 0x85),
  success: Color::Rgb(0x40, 0xA0, 0x2B),
  warning: Color::Rgb(0xFE, 0x64, 0x0B),
  error: Color::Rgb(0xD2, 0x0F, 0x39),
  highlight: Color::Rgb(0xDF, 0x8E, 0x1D),
};

// Gruvbox Dark (hard) — https://github.com/morhetz/gruvbox
const GRUVBOX_DARK: Palette = Palette {
  name: ThemeName::GruvboxDark,
  is_dark: true,
  bg: Color::Rgb(0x1D, 0x20, 0x21),
  fg: Color::Rgb(0xEB, 0xDB, 0xB2),
  accent: Color::Rgb(0xFE, 0x80, 0x19),
  secondary: Color::Rgb(0xFA, 0xBD, 0x2F),
  label: Color::Rgb(0x83, 0xA5, 0x98),
  muted: Color::Rgb(0xA8, 0x99, 0x84),
  success: Color::Rgb(0xB8, 0xBB, 0x26),
  warning: Color::Rgb(0xFA, 0xBD, 0x2F),
  error: Color::Rgb(0xFB, 0x49, 0x34),
  highlight: Color::Rgb(0xFA, 0xBD, 0x2F),
};

// Solarized Dark — https://ethanschoonover.com/solarized
const SOLARIZED_DARK: Palette = Palette {
  name: ThemeName::SolarizedDark,
  is_dark: true,
  bg: Color::Rgb(0x00, 0x2B, 0x36),
  fg: Color::Rgb(0x93, 0xA1, 0xA1),
  accent: Color::Rgb(0x26, 0x8B, 0xD2),
  secondary: Color::Rgb(0xB5, 0x89, 0x00),
  label: Color::Rgb(0x2A, 0xA1, 0x98),
  muted: Color::Rgb(0x65, 0x7B, 0x83),
  success: Color::Rgb(0x85, 0x99, 0x00),
  warning: Color::Rgb(0xB5, 0x89, 0x00),
  error: Color::Rgb(0xDC, 0x32, 0x2F),
  highlight: Color::Rgb(0xB5, 0x89, 0x00),
};

// Monochrome — relies on glyph cues plus bold/reverse modifiers since colour
// cannot carry meaning here. `highlight = Reset` makes `focus_border` fall
// back to `accent` so the focused border stays visible.
const MONO: Palette = Palette {
  name: ThemeName::Mono,
  is_dark: true,
  bg: Color::Reset,
  fg: Color::White,
  accent: Color::White,
  secondary: Color::White,
  label: Color::Gray,
  muted: Color::Gray,
  success: Color::White,
  warning: Color::Gray,
  error: Color::White,
  highlight: Color::Reset,
};

/// Resolve a [`ThemeName`] to its concrete [`Palette`]. `Custom` resolves to
/// the macchiato fallback here; the user-loaded custom palette is overlaid by
/// `App::palette()`.
pub fn palette_for(theme: ThemeName) -> Palette {
  match theme {
    ThemeName::Macchiato | ThemeName::Custom => MACCHIATO,
    ThemeName::Latte => LATTE,
    ThemeName::GruvboxDark => GRUVBOX_DARK,
    ThemeName::SolarizedDark => SOLARIZED_DARK,
    ThemeName::Mono => MONO,
  }
}

// ─── Custom theme (YAML `custom_theme:` block) ────────────────────────

/// YAML-shaped user theme. Every colour slot is optional; the resolver
/// fills missing slots from `base` (default macchiato).
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct CustomThemeConfig {
  /// Built-in palette to fall back to for slots the user didn't override.
  /// `None` → macchiato. `Custom` is rejected (you can't base a custom
  /// theme on itself).
  pub base: Option<ThemeName>,
  pub is_dark: Option<bool>,
  pub bg: Option<String>,
  pub fg: Option<String>,
  pub accent: Option<String>,
  pub secondary: Option<String>,
  pub label: Option<String>,
  pub muted: Option<String>,
  pub success: Option<String>,
  pub warning: Option<String>,
  pub error: Option<String>,
  pub highlight: Option<String>,
}

impl CustomThemeConfig {
  /// Build a concrete [`Palette`] from this config. Accumulates warnings for
  /// any colour value that failed to parse; the returned palette substitutes
  /// the base value for those slots so the UI still renders cleanly.
  pub fn resolve(&self) -> (Palette, Vec<String>) {
    let mut warnings = vec![];
    let base_name = match self.base {
      Some(ThemeName::Custom) => {
        warnings
          .push("custom_theme.base cannot be `custom`; falling back to macchiato".to_string());
        ThemeName::Macchiato
      }
      Some(other) => other,
      None => ThemeName::Macchiato,
    };
    let base = palette_for(base_name);

    let mut palette = Palette {
      name: ThemeName::Custom,
      is_dark: self.is_dark.unwrap_or(base.is_dark),
      ..base
    };

    apply(&self.bg, "bg", &mut palette.bg, &mut warnings);
    apply(&self.fg, "fg", &mut palette.fg, &mut warnings);
    apply(&self.accent, "accent", &mut palette.accent, &mut warnings);
    apply(
      &self.secondary,
      "secondary",
      &mut palette.secondary,
      &mut warnings,
    );
    apply(&self.label, "label", &mut palette.label, &mut warnings);
    apply(&self.muted, "muted", &mut palette.muted, &mut warnings);
    apply(
      &self.success,
      "success",
      &mut palette.success,
      &mut warnings,
    );
    apply(
      &self.warning,
      "warning",
      &mut palette.warning,
      &mut warnings,
    );
    apply(&self.error, "error", &mut palette.error, &mut warnings);
    apply(
      &self.highlight,
      "highlight",
      &mut palette.highlight,
      &mut warnings,
    );

    (palette, warnings)
  }
}

fn apply(raw: &Option<String>, key: &str, target: &mut Color, warnings: &mut Vec<String>) {
  if let Some(value) = raw {
    match parse_color(value) {
      Ok(color) => *target = color,
      Err(error) => warnings.push(format!(
        "custom_theme.{}: '{}' — {}; keeping base value",
        key, value, error
      )),
    }
  }
}

// ─── Legacy override layer (`theme: { dark, light }` map) ──────────────

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ThemeOverrides {
  dark: BTreeMap<String, Color>,
  light: BTreeMap<String, Color>,
}

static ACTIVE_THEME_OVERRIDES: OnceLock<ThemeOverrides> = OnceLock::new();

/// Config keys accepted in the legacy `theme: { dark, light }` map. The
/// historical eight keep their names but now address the remapped slots
/// (`primary→accent`, `secondary→secondary`, `help→muted`, `text→fg`,
/// `failure→error`); `label` and `highlight` are new.
const THEME_KEYS: &[&str] = &[
  "text",
  "failure",
  "warning",
  "success",
  "primary",
  "secondary",
  "help",
  "label",
  "highlight",
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

/// Apply the legacy `theme: { dark, light }` overrides onto a built-in
/// catppuccin palette. The `dark` section tints Macchiato, the `light`
/// section tints Latte — matching the old two-theme toggle behaviour.
pub fn apply_legacy_overrides(palette: &mut Palette) {
  let Some(overrides) = ACTIVE_THEME_OVERRIDES.get() else {
    return;
  };
  let colors = if palette.is_dark {
    &overrides.dark
  } else {
    &overrides.light
  };
  for (key, color) in colors {
    match key.as_str() {
      "text" => palette.fg = *color,
      "failure" => palette.error = *color,
      "warning" => palette.warning = *color,
      "success" => palette.success = *color,
      "primary" => palette.accent = *color,
      "secondary" => palette.secondary = *color,
      "help" => palette.muted = *color,
      "label" => palette.label = *color,
      "highlight" => palette.highlight = *color,
      "background" => palette.bg = *color,
      _ => {}
    }
  }
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

  fn set_overrides(config: &KdashConfig) {
    // Tests share one process; build overrides directly instead of racing on
    // the global OnceLock that `initialize_theme` sets.
    let (overrides, _) = build_theme_overrides(config);
    let _ = ACTIVE_THEME_OVERRIDES.set(overrides);
  }

  #[test]
  fn test_theme_name_parse_accepts_canonical_and_aliases() {
    assert_eq!(ThemeName::from_str("macchiato"), Ok(ThemeName::Macchiato));
    assert_eq!(
      ThemeName::from_str("catppuccin-macchiato"),
      Ok(ThemeName::Macchiato)
    );
    assert_eq!(ThemeName::from_str("Latte"), Ok(ThemeName::Latte));
    assert_eq!(ThemeName::from_str("gruvbox"), Ok(ThemeName::GruvboxDark));
    assert_eq!(
      ThemeName::from_str("solarized-dark"),
      Ok(ThemeName::SolarizedDark)
    );
    assert_eq!(ThemeName::from_str("monochrome"), Ok(ThemeName::Mono));
  }

  #[test]
  fn test_theme_name_parse_rejects_unknown() {
    let err = ThemeName::from_str("dracula").unwrap_err();
    assert_eq!(err.value, "dracula");
    let rendered = err.to_string();
    assert!(rendered.contains("dracula"));
    assert!(rendered.contains("macchiato"));
  }

  #[test]
  fn test_canonical_round_trips_through_parse() {
    for theme in ThemeName::ALL {
      assert_eq!(ThemeName::from_str(theme.canonical()), Ok(theme));
    }
  }

  #[test]
  fn test_palette_for_matches_name() {
    for theme in ThemeName::ALL {
      let palette = palette_for(theme);
      if theme == ThemeName::Custom {
        assert_eq!(palette.name, ThemeName::Macchiato);
      } else {
        assert_eq!(palette.name, theme);
      }
    }
  }

  #[test]
  fn test_focus_border_uses_highlight_then_falls_back_to_accent() {
    let mac = palette_for(ThemeName::Macchiato);
    assert_eq!(mac.focus_border(true), mac.highlight);
    assert_eq!(mac.focus_border(false), mac.accent);
    // Mono opts out of highlight (Reset) → focus falls back to accent.
    let mono = palette_for(ThemeName::Mono);
    assert_eq!(mono.highlight, Color::Reset);
    assert_eq!(mono.focus_border(true), mono.accent);
  }

  #[test]
  fn test_custom_theme_empty_clones_macchiato_named_custom() {
    let (palette, warnings) = CustomThemeConfig::default().resolve();
    assert!(warnings.is_empty());
    assert_eq!(palette.name, ThemeName::Custom);
    let base = palette_for(ThemeName::Macchiato);
    assert_eq!(palette.bg, base.bg);
    assert_eq!(palette.accent, base.accent);
    assert_eq!(palette.label, base.label);
  }

  #[test]
  fn test_custom_theme_base_and_overrides() {
    let cfg = CustomThemeConfig {
      base: Some(ThemeName::Latte),
      accent: Some("#FF00AA".into()),
      label: Some("blue".into()),
      ..Default::default()
    };
    let (palette, warnings) = cfg.resolve();
    assert!(warnings.is_empty());
    let latte = palette_for(ThemeName::Latte);
    assert_eq!(palette.bg, latte.bg);
    assert_eq!(palette.accent, Color::Rgb(0xFF, 0x00, 0xAA));
    assert_eq!(palette.label, Color::Blue);
    assert!(!palette.is_dark);
  }

  #[test]
  fn test_custom_theme_base_custom_warns() {
    let cfg = CustomThemeConfig {
      base: Some(ThemeName::Custom),
      ..Default::default()
    };
    let (palette, warnings) = cfg.resolve();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("custom"));
    assert_eq!(palette.bg, palette_for(ThemeName::Macchiato).bg);
  }

  #[test]
  fn test_custom_theme_bad_color_warns_keeps_base() {
    let cfg = CustomThemeConfig {
      accent: Some("not-a-color".into()),
      ..Default::default()
    };
    let (palette, warnings) = cfg.resolve();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("accent"));
    assert_eq!(palette.accent, palette_for(ThemeName::Macchiato).accent);
  }

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

  #[test]
  fn test_apply_legacy_overrides_remaps_keys_onto_slots() {
    // `set_overrides` writes the shared OnceLock; keep this the only test that
    // relies on the global so the value is deterministic.
    let config = KdashConfig {
      theme: Some(ThemeConfig {
        dark: Some(BTreeMap::from([
          ("primary".into(), "green".into()),
          ("help".into(), "red".into()),
        ])),
        light: None,
      }),
      ..Default::default()
    };
    set_overrides(&config);

    let mut palette = palette_for(ThemeName::Macchiato);
    apply_legacy_overrides(&mut palette);
    // primary → accent, help → muted.
    assert_eq!(palette.accent, Color::Green);
    assert_eq!(palette.muted, Color::Red);
  }
}
