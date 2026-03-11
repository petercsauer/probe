use crossterm::event::KeyCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub tui: TuiConfig,
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
