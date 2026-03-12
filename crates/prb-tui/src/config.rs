use crossterm::event::KeyCode;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Optional color overrides for theme customization
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColorOverrides {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub selected_row_fg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub selected_row_bg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub zebra_bg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub warning_bg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub focused_border: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub unfocused_border: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub focused_title_fg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub unfocused_title_fg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub header_fg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub status_bar_fg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub status_bar_bg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub filter_bar_fg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub filter_bar_bg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub filter_error_fg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub warning_fg: Option<String>,
}

impl ColorOverrides {
    /// Apply color overrides to a ThemeConfig
    pub fn apply_to_theme(&self, theme: &mut crate::theme::ThemeConfig) {
        if let Some(ref color_str) = self.selected_row_fg {
            if let Some(color) = parse_color(color_str) {
                theme.selected_row_fg = color;
            }
        }
        if let Some(ref color_str) = self.selected_row_bg {
            if let Some(color) = parse_color(color_str) {
                theme.selected_row_bg = color;
            }
        }
        if let Some(ref color_str) = self.zebra_bg {
            if let Some(color) = parse_color(color_str) {
                theme.zebra_bg = color;
            }
        }
        if let Some(ref color_str) = self.warning_bg {
            if let Some(color) = parse_color(color_str) {
                theme.warning_bg = color;
            }
        }
        if let Some(ref color_str) = self.focused_border {
            if let Some(color) = parse_color(color_str) {
                theme.focused_border = color;
            }
        }
        if let Some(ref color_str) = self.unfocused_border {
            if let Some(color) = parse_color(color_str) {
                theme.unfocused_border = color;
            }
        }
        if let Some(ref color_str) = self.focused_title_fg {
            if let Some(color) = parse_color(color_str) {
                theme.focused_title_fg = color;
            }
        }
        if let Some(ref color_str) = self.unfocused_title_fg {
            if let Some(color) = parse_color(color_str) {
                theme.unfocused_title_fg = color;
            }
        }
        if let Some(ref color_str) = self.header_fg {
            if let Some(color) = parse_color(color_str) {
                theme.header_fg = color;
            }
        }
        if let Some(ref color_str) = self.status_bar_fg {
            if let Some(color) = parse_color(color_str) {
                theme.status_bar_fg = color;
            }
        }
        if let Some(ref color_str) = self.status_bar_bg {
            if let Some(color) = parse_color(color_str) {
                theme.status_bar_bg = color;
            }
        }
        if let Some(ref color_str) = self.filter_bar_fg {
            if let Some(color) = parse_color(color_str) {
                theme.filter_bar_fg = color;
            }
        }
        if let Some(ref color_str) = self.filter_bar_bg {
            if let Some(color) = parse_color(color_str) {
                theme.filter_bar_bg = color;
            }
        }
        if let Some(ref color_str) = self.filter_error_fg {
            if let Some(color) = parse_color(color_str) {
                theme.filter_error_fg = color;
            }
        }
        if let Some(ref color_str) = self.warning_fg {
            if let Some(color) = parse_color(color_str) {
                theme.warning_fg = color;
            }
        }
    }

    /// Create ColorOverrides from a ThemeConfig (for saving custom themes)
    pub fn from_theme(theme: &crate::theme::ThemeConfig, base_theme: &crate::theme::ThemeConfig) -> Option<Self> {
        let mut overrides = ColorOverrides {
            selected_row_fg: None,
            selected_row_bg: None,
            zebra_bg: None,
            warning_bg: None,
            focused_border: None,
            unfocused_border: None,
            focused_title_fg: None,
            unfocused_title_fg: None,
            header_fg: None,
            status_bar_fg: None,
            status_bar_bg: None,
            filter_bar_fg: None,
            filter_bar_bg: None,
            filter_error_fg: None,
            warning_fg: None,
        };

        let mut has_overrides = false;

        // Check each field for differences
        if theme.selected_row_fg != base_theme.selected_row_fg {
            overrides.selected_row_fg = Some(color_to_string(theme.selected_row_fg));
            has_overrides = true;
        }
        if theme.selected_row_bg != base_theme.selected_row_bg {
            overrides.selected_row_bg = Some(color_to_string(theme.selected_row_bg));
            has_overrides = true;
        }
        if theme.zebra_bg != base_theme.zebra_bg {
            overrides.zebra_bg = Some(color_to_string(theme.zebra_bg));
            has_overrides = true;
        }
        if theme.warning_bg != base_theme.warning_bg {
            overrides.warning_bg = Some(color_to_string(theme.warning_bg));
            has_overrides = true;
        }
        if theme.focused_border != base_theme.focused_border {
            overrides.focused_border = Some(color_to_string(theme.focused_border));
            has_overrides = true;
        }
        if theme.unfocused_border != base_theme.unfocused_border {
            overrides.unfocused_border = Some(color_to_string(theme.unfocused_border));
            has_overrides = true;
        }
        if theme.focused_title_fg != base_theme.focused_title_fg {
            overrides.focused_title_fg = Some(color_to_string(theme.focused_title_fg));
            has_overrides = true;
        }
        if theme.unfocused_title_fg != base_theme.unfocused_title_fg {
            overrides.unfocused_title_fg = Some(color_to_string(theme.unfocused_title_fg));
            has_overrides = true;
        }
        if theme.header_fg != base_theme.header_fg {
            overrides.header_fg = Some(color_to_string(theme.header_fg));
            has_overrides = true;
        }
        if theme.status_bar_fg != base_theme.status_bar_fg {
            overrides.status_bar_fg = Some(color_to_string(theme.status_bar_fg));
            has_overrides = true;
        }
        if theme.status_bar_bg != base_theme.status_bar_bg {
            overrides.status_bar_bg = Some(color_to_string(theme.status_bar_bg));
            has_overrides = true;
        }
        if theme.filter_bar_fg != base_theme.filter_bar_fg {
            overrides.filter_bar_fg = Some(color_to_string(theme.filter_bar_fg));
            has_overrides = true;
        }
        if theme.filter_bar_bg != base_theme.filter_bar_bg {
            overrides.filter_bar_bg = Some(color_to_string(theme.filter_bar_bg));
            has_overrides = true;
        }
        if theme.filter_error_fg != base_theme.filter_error_fg {
            overrides.filter_error_fg = Some(color_to_string(theme.filter_error_fg));
            has_overrides = true;
        }
        if theme.warning_fg != base_theme.warning_fg {
            overrides.warning_fg = Some(color_to_string(theme.warning_fg));
            has_overrides = true;
        }

        if has_overrides {
            Some(overrides)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub tui: TuiConfig,
    #[serde(default)]
    pub ai: prb_ai::AiConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TuiConfig {
    #[serde(default = "default_theme")]
    pub theme: String,

    #[serde(default = "default_max_events")]
    pub max_events: usize,

    #[serde(default = "default_auto_follow")]
    pub auto_follow: bool,

    #[serde(default = "default_show_timeline")]
    pub show_timeline: bool,

    #[serde(default)]
    pub columns: ColumnConfig,

    #[serde(default)]
    pub keybindings: KeyBindings,

    #[serde(default)]
    pub profiles: HashMap<String, ProfileConfig>,

    #[serde(default)]
    pub colors: Option<ColorOverrides>,
}

impl Default for TuiConfig {
    fn default() -> Self {
        TuiConfig {
            theme: default_theme(),
            max_events: default_max_events(),
            auto_follow: default_auto_follow(),
            show_timeline: default_show_timeline(),
            columns: ColumnConfig::default(),
            keybindings: KeyBindings::default(),
            profiles: HashMap::new(),
            colors: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColumnConfig {
    #[serde(default = "default_visible_columns")]
    pub visible: Vec<String>,
}

impl Default for ColumnConfig {
    fn default() -> Self {
        ColumnConfig {
            visible: default_visible_columns(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeyBindings {
    #[serde(default = "default_quit_key")]
    pub quit: String,

    #[serde(default = "default_filter_key")]
    pub filter: String,

    #[serde(default = "default_zoom_key")]
    pub zoom: String,

    #[serde(default = "default_help_key")]
    pub help: String,

    #[serde(default = "default_theme_cycle_key")]
    pub theme_cycle: String,
}

impl Default for KeyBindings {
    fn default() -> Self {
        KeyBindings {
            quit: default_quit_key(),
            filter: default_filter_key(),
            zoom: default_zoom_key(),
            help: default_help_key(),
            theme_cycle: default_theme_cycle_key(),
        }
    }
}

impl KeyBindings {
    pub fn quit_keycode(&self) -> Option<KeyCode> {
        parse_keycode(&self.quit)
    }

    pub fn filter_keycode(&self) -> Option<KeyCode> {
        parse_keycode(&self.filter)
    }

    pub fn zoom_keycode(&self) -> Option<KeyCode> {
        parse_keycode(&self.zoom)
    }

    pub fn help_keycode(&self) -> Option<KeyCode> {
        parse_keycode(&self.help)
    }

    pub fn theme_cycle_keycode(&self) -> Option<KeyCode> {
        parse_keycode(&self.theme_cycle)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProfileConfig {
    pub theme: Option<String>,
    pub default_filter: Option<String>,
    pub columns: Option<Vec<String>>,
}

impl Config {
    pub fn load() -> Self {
        let config_path = Self::config_path();

        if let Some(path) = config_path {
            if path.exists() {
                match fs::read_to_string(&path) {
                    Ok(contents) => {
                        match toml::from_str::<Config>(&contents) {
                            Ok(config) => {
                                tracing::debug!("Loaded config from {:?}", path);
                                return config;
                            }
                            Err(e) => {
                                tracing::warn!("Failed to parse config file {:?}: {}", path, e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read config file {:?}: {}", path, e);
                    }
                }
            } else {
                tracing::debug!("Config file not found at {:?}, using defaults", path);
            }
        }

        Config::default()
    }

    pub fn config_path() -> Option<PathBuf> {
        if let Some(config_dir) = dirs::config_dir() {
            let prb_config = config_dir.join("prb");
            Some(prb_config.join("config.toml"))
        } else {
            None
        }
    }

    pub fn ensure_config_dir() -> anyhow::Result<PathBuf> {
        if let Some(config_dir) = dirs::config_dir() {
            let prb_config = config_dir.join("prb");
            if !prb_config.exists() {
                fs::create_dir_all(&prb_config)?;
            }
            Ok(prb_config)
        } else {
            anyhow::bail!("Could not determine config directory")
        }
    }

    pub fn save_default_config() -> anyhow::Result<PathBuf> {
        let config_dir = Self::ensure_config_dir()?;
        let config_path = config_dir.join("config.toml");

        if !config_path.exists() {
            let default_config = Config::default();
            let toml_str = toml::to_string_pretty(&default_config)?;
            fs::write(&config_path, toml_str)?;
            tracing::info!("Created default config at {:?}", config_path);
        }

        Ok(config_path)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let config_dir = Self::ensure_config_dir()?;
        let config_path = config_dir.join("config.toml");
        let toml_str = toml::to_string_pretty(self)?;
        fs::write(&config_path, toml_str)?;
        tracing::debug!("Saved config to {:?}", config_path);
        Ok(())
    }
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_max_events() -> usize {
    100_000
}

fn default_auto_follow() -> bool {
    true
}

fn default_show_timeline() -> bool {
    true
}

fn default_visible_columns() -> Vec<String> {
    vec![
        "#".to_string(),
        "time".to_string(),
        "source".to_string(),
        "destination".to_string(),
        "protocol".to_string(),
        "direction".to_string(),
        "summary".to_string(),
    ]
}

fn default_quit_key() -> String {
    "q".to_string()
}

fn default_filter_key() -> String {
    "/".to_string()
}

fn default_zoom_key() -> String {
    "z".to_string()
}

fn default_help_key() -> String {
    "?".to_string()
}

fn default_theme_cycle_key() -> String {
    "T".to_string()
}

fn parse_keycode(s: &str) -> Option<KeyCode> {
    match s {
        "q" => Some(KeyCode::Char('q')),
        "/" => Some(KeyCode::Char('/')),
        "z" => Some(KeyCode::Char('z')),
        "?" => Some(KeyCode::Char('?')),
        "T" => Some(KeyCode::Char('T')),
        "Esc" => Some(KeyCode::Esc),
        "Enter" => Some(KeyCode::Enter),
        "Tab" => Some(KeyCode::Tab),
        "BackTab" => Some(KeyCode::BackTab),
        _ => {
            if s.len() == 1 {
                s.chars().next().map(KeyCode::Char)
            } else {
                None
            }
        }
    }
}

/// Parse a color string in hex format (#RRGGBB) or color name
pub fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();

    // Try hex format
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Color::Rgb(r, g, b));
        }
    }

    // Try color names
    match s.to_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "darkgrey" => Some(Color::DarkGray),
        "white" => Some(Color::White),
        "reset" => Some(Color::Reset),
        _ => None,
    }
}

/// Convert a color to a string representation
pub fn color_to_string(color: Color) -> String {
    match color {
        Color::Reset => "reset".to_string(),
        Color::Black => "black".to_string(),
        Color::Red => "red".to_string(),
        Color::Green => "green".to_string(),
        Color::Yellow => "yellow".to_string(),
        Color::Blue => "blue".to_string(),
        Color::Magenta => "magenta".to_string(),
        Color::Cyan => "cyan".to_string(),
        Color::Gray => "gray".to_string(),
        Color::DarkGray => "darkgray".to_string(),
        Color::White => "white".to_string(),
        Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
        Color::Indexed(i) => format!("indexed{}", i),
        _ => "reset".to_string(),
    }
}
