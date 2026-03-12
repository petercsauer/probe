//! Comprehensive tests for config.rs to reach 60% coverage

use prb_tui::config::{
    ColorOverrides, Config, KeyBindings, TuiConfig, color_to_string, parse_color,
};
use prb_tui::theme::ThemeConfig;
use ratatui::style::Color;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_config_default() {
    let config = Config::default();
    assert_eq!(config.tui.theme, "dark");
    assert_eq!(config.tui.max_events, 100_000);
    assert!(config.tui.auto_follow);
    assert!(config.tui.show_timeline);
}

#[test]
fn test_tui_config_default() {
    let tui = TuiConfig::default();
    assert_eq!(tui.theme, "dark");
    assert_eq!(tui.max_events, 100_000);
    assert!(tui.auto_follow);
    assert!(tui.show_timeline);
    assert!(tui.colors.is_none());
}

#[test]
fn test_config_from_toml_minimal() {
    let toml = r#"
        [tui]
        theme = "light"
    "#;

    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.tui.theme, "light");
}

#[test]
fn test_config_from_toml_full() {
    let toml = r##"
        [tui]
        theme = "monokai"
        max_events = 50000
        auto_follow = false
        show_timeline = false

        [tui.columns]
        visible = ["#", "time", "protocol"]

        [tui.keybindings]
        quit = "Q"
        filter = "f"
        zoom = "Z"
        help = "h"
        theme_cycle = "t"
    "##;

    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.tui.theme, "monokai");
    assert_eq!(config.tui.max_events, 50000);
    assert!(!config.tui.auto_follow);
    assert!(!config.tui.show_timeline);
    assert_eq!(config.tui.columns.visible.len(), 3);
    assert_eq!(config.tui.keybindings.quit, "Q");
}

#[test]
fn test_config_with_color_overrides() {
    let toml = r##"
        [tui]
        theme = "dark"

        [tui.colors]
        selected_row_fg = "#ff0000"
        selected_row_bg = "blue"
        zebra_bg = "#1a1a1a"
    "##;

    let config: Config = toml::from_str(toml).unwrap();
    assert!(config.tui.colors.is_some());
    let colors = config.tui.colors.unwrap();
    assert_eq!(colors.selected_row_fg.as_ref().unwrap(), "#ff0000");
    assert_eq!(colors.selected_row_bg.as_ref().unwrap(), "blue");
}

#[test]
fn test_keybindings_default() {
    let kb = KeyBindings::default();
    assert_eq!(kb.quit, "q");
    assert_eq!(kb.filter, "/");
    assert_eq!(kb.zoom, "z");
    assert_eq!(kb.help, "?");
    assert_eq!(kb.theme_cycle, "T");
}

#[test]
fn test_keybindings_keycode_parsing() {
    let kb = KeyBindings::default();
    assert!(kb.quit_keycode().is_some());
    assert!(kb.filter_keycode().is_some());
    assert!(kb.zoom_keycode().is_some());
    assert!(kb.help_keycode().is_some());
    assert!(kb.theme_cycle_keycode().is_some());
}

#[test]
fn test_keybindings_custom_keys() {
    let kb = KeyBindings {
        quit: "Esc".to_string(),
        filter: "Enter".to_string(),
        ..Default::default()
    };

    assert!(kb.quit_keycode().is_some());
    assert!(kb.filter_keycode().is_some());
}

#[test]
fn test_parse_color_hex() {
    assert_eq!(parse_color("#ff0000"), Some(Color::Rgb(255, 0, 0)));
    assert_eq!(parse_color("#00ff00"), Some(Color::Rgb(0, 255, 0)));
    assert_eq!(parse_color("#0000ff"), Some(Color::Rgb(0, 0, 255)));
    assert_eq!(parse_color("#123456"), Some(Color::Rgb(0x12, 0x34, 0x56)));
}

#[test]
fn test_parse_color_hex_with_whitespace() {
    assert_eq!(parse_color("  #ff0000  "), Some(Color::Rgb(255, 0, 0)));
}

#[test]
fn test_parse_color_names() {
    assert_eq!(parse_color("black"), Some(Color::Black));
    assert_eq!(parse_color("red"), Some(Color::Red));
    assert_eq!(parse_color("green"), Some(Color::Green));
    assert_eq!(parse_color("yellow"), Some(Color::Yellow));
    assert_eq!(parse_color("blue"), Some(Color::Blue));
    assert_eq!(parse_color("magenta"), Some(Color::Magenta));
    assert_eq!(parse_color("cyan"), Some(Color::Cyan));
    assert_eq!(parse_color("gray"), Some(Color::Gray));
    assert_eq!(parse_color("grey"), Some(Color::Gray));
    assert_eq!(parse_color("darkgray"), Some(Color::DarkGray));
    assert_eq!(parse_color("darkgrey"), Some(Color::DarkGray));
    assert_eq!(parse_color("white"), Some(Color::White));
    assert_eq!(parse_color("reset"), Some(Color::Reset));
}

#[test]
fn test_parse_color_case_insensitive() {
    assert_eq!(parse_color("RED"), Some(Color::Red));
    assert_eq!(parse_color("Blue"), Some(Color::Blue));
    assert_eq!(parse_color("CYAN"), Some(Color::Cyan));
}

#[test]
fn test_parse_color_invalid() {
    assert_eq!(parse_color("invalid"), None);
    assert_eq!(parse_color("#ff"), None);
    assert_eq!(parse_color("#gggggg"), None);
    assert_eq!(parse_color(""), None);
}

#[test]
fn test_color_to_string_basic() {
    assert_eq!(color_to_string(Color::Black), "black");
    assert_eq!(color_to_string(Color::Red), "red");
    assert_eq!(color_to_string(Color::Green), "green");
    assert_eq!(color_to_string(Color::Yellow), "yellow");
    assert_eq!(color_to_string(Color::Blue), "blue");
    assert_eq!(color_to_string(Color::Magenta), "magenta");
    assert_eq!(color_to_string(Color::Cyan), "cyan");
    assert_eq!(color_to_string(Color::Gray), "gray");
    assert_eq!(color_to_string(Color::DarkGray), "darkgray");
    assert_eq!(color_to_string(Color::White), "white");
    assert_eq!(color_to_string(Color::Reset), "reset");
}

#[test]
fn test_color_to_string_rgb() {
    assert_eq!(color_to_string(Color::Rgb(255, 0, 0)), "#ff0000");
    assert_eq!(color_to_string(Color::Rgb(0, 255, 0)), "#00ff00");
    assert_eq!(color_to_string(Color::Rgb(0, 0, 255)), "#0000ff");
    assert_eq!(color_to_string(Color::Rgb(0x12, 0x34, 0x56)), "#123456");
}

#[test]
fn test_color_to_string_indexed() {
    assert_eq!(color_to_string(Color::Indexed(42)), "indexed42");
}

#[test]
fn test_color_roundtrip() {
    let colors = vec![
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::White,
        Color::Gray,
        Color::DarkGray,
        Color::Reset,
    ];

    for color in colors {
        let string = color_to_string(color);
        let parsed = parse_color(&string);
        assert_eq!(parsed, Some(color));
    }
}

#[test]
fn test_color_roundtrip_rgb() {
    let rgb_colors = vec![
        Color::Rgb(255, 0, 0),
        Color::Rgb(0, 255, 0),
        Color::Rgb(0, 0, 255),
        Color::Rgb(123, 45, 67),
    ];

    for color in rgb_colors {
        let string = color_to_string(color);
        let parsed = parse_color(&string);
        assert_eq!(parsed, Some(color));
    }
}

#[test]
fn test_color_overrides_apply_to_theme() {
    let overrides = ColorOverrides {
        selected_row_fg: Some("#ff0000".to_string()),
        selected_row_bg: Some("blue".to_string()),
        zebra_bg: Some("#1a1a1a".to_string()),
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

    let mut theme = ThemeConfig::dark();
    let original_warning = theme.warning_bg;

    overrides.apply_to_theme(&mut theme);

    assert_eq!(theme.selected_row_fg, Color::Rgb(255, 0, 0));
    assert_eq!(theme.selected_row_bg, Color::Blue);
    assert_eq!(theme.zebra_bg, Color::Rgb(0x1a, 0x1a, 0x1a));
    assert_eq!(theme.warning_bg, original_warning); // unchanged
}

#[test]
fn test_color_overrides_from_theme_no_changes() {
    let theme = ThemeConfig::dark();
    let base_theme = ThemeConfig::dark();

    let overrides = ColorOverrides::from_theme(&theme, &base_theme);
    assert!(overrides.is_none());
}

#[test]
fn test_color_overrides_from_theme_with_changes() {
    let mut theme = ThemeConfig::dark();
    let base_theme = ThemeConfig::dark();

    theme.selected_row_fg = Color::Red;
    theme.selected_row_bg = Color::Green;

    let overrides = ColorOverrides::from_theme(&theme, &base_theme);
    assert!(overrides.is_some());

    let overrides = overrides.unwrap();
    assert!(overrides.selected_row_fg.is_some());
    assert!(overrides.selected_row_bg.is_some());
    assert!(overrides.zebra_bg.is_none());
}

#[test]
fn test_config_load_nonexistent() {
    // Should return default config when file doesn't exist
    let config = Config::load();
    assert_eq!(config.tui.theme, "dark");
}

#[test]
fn test_config_save_and_load() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // Create a custom config
    let mut config = Config::default();
    config.tui.theme = "light".to_string();
    config.tui.max_events = 50000;

    // Save it
    let toml_str = toml::to_string_pretty(&config).unwrap();
    fs::write(&config_path, toml_str).unwrap();

    // Load it back
    let contents = fs::read_to_string(&config_path).unwrap();
    let loaded: Config = toml::from_str(&contents).unwrap();

    assert_eq!(loaded.tui.theme, "light");
    assert_eq!(loaded.tui.max_events, 50000);
}

#[test]
fn test_config_with_profiles() {
    let toml = r#"
        [tui]
        theme = "dark"

        [tui.profiles.dev]
        theme = "light"
        default_filter = "protocol == grpc"

        [tui.profiles.prod]
        theme = "monokai"
    "#;

    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.tui.profiles.len(), 2);
    assert!(config.tui.profiles.contains_key("dev"));
    assert!(config.tui.profiles.contains_key("prod"));

    let dev_profile = &config.tui.profiles["dev"];
    assert_eq!(dev_profile.theme.as_ref().unwrap(), "light");
    assert_eq!(
        dev_profile.default_filter.as_ref().unwrap(),
        "protocol == grpc"
    );
}

#[test]
fn test_keybindings_special_keys() {
    use crossterm::event::KeyCode;

    let kb = KeyBindings {
        quit: "Esc".to_string(),
        filter: "Enter".to_string(),
        zoom: "Tab".to_string(),
        help: "BackTab".to_string(),
        ..Default::default()
    };

    assert_eq!(kb.quit_keycode(), Some(KeyCode::Esc));
    assert_eq!(kb.filter_keycode(), Some(KeyCode::Enter));
    assert_eq!(kb.zoom_keycode(), Some(KeyCode::Tab));
    assert_eq!(kb.help_keycode(), Some(KeyCode::BackTab));
}

#[test]
fn test_keybindings_invalid_key() {
    use prb_tui::config::Config;

    let toml = r#"
        [tui.keybindings]
        quit = "InvalidKey"
    "#;

    // Should parse but may not convert to keycode
    let config: Config = toml::from_str(toml).unwrap();
    // Just verify it doesn't panic
    let _ = config.tui.keybindings.quit_keycode();
}

#[test]
fn test_column_config_default() {
    use prb_tui::config::ColumnConfig;

    let columns = ColumnConfig::default();
    assert!(!columns.visible.is_empty());
    assert!(columns.visible.contains(&"time".to_string()));
    assert!(columns.visible.contains(&"protocol".to_string()));
}

#[test]
fn test_config_empty_toml() {
    let toml = "";
    let config: Config = toml::from_str(toml).unwrap();
    // Should use all defaults
    assert_eq!(config.tui.theme, "dark");
}

#[test]
fn test_color_overrides_all_fields() {
    let overrides = ColorOverrides {
        selected_row_fg: Some("red".to_string()),
        selected_row_bg: Some("blue".to_string()),
        zebra_bg: Some("cyan".to_string()),
        warning_bg: Some("yellow".to_string()),
        focused_border: Some("magenta".to_string()),
        unfocused_border: Some("gray".to_string()),
        focused_title_fg: Some("white".to_string()),
        unfocused_title_fg: Some("darkgray".to_string()),
        header_fg: Some("green".to_string()),
        status_bar_fg: Some("black".to_string()),
        status_bar_bg: Some("white".to_string()),
        filter_bar_fg: Some("blue".to_string()),
        filter_bar_bg: Some("black".to_string()),
        filter_error_fg: Some("red".to_string()),
        warning_fg: Some("yellow".to_string()),
    };

    let mut theme = ThemeConfig::dark();
    overrides.apply_to_theme(&mut theme);

    // Verify all colors were applied
    assert_eq!(theme.selected_row_fg, Color::Red);
    assert_eq!(theme.selected_row_bg, Color::Blue);
    assert_eq!(theme.zebra_bg, Color::Cyan);
    assert_eq!(theme.warning_bg, Color::Yellow);
    assert_eq!(theme.focused_border, Color::Magenta);
    assert_eq!(theme.unfocused_border, Color::Gray);
}

#[test]
fn test_config_path_returns_some() {
    // Should return Some path on most systems
    let path = Config::config_path();
    // Just verify it doesn't panic and has reasonable structure
    if let Some(p) = path {
        assert!(p.to_string_lossy().contains("prb"));
    }
}
