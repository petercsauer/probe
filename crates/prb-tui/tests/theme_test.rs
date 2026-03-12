//! Unit tests for theme.rs

use prb_core::{Direction, TransportKind};
use prb_tui::theme::Theme;
use ratatui::style::{Color, Modifier};

#[test]
fn test_selected_row_style() {
    let style = Theme::selected_row();
    assert_eq!(style.fg, Some(Color::Black));
    assert_eq!(style.bg, Some(Color::Cyan));
}

#[test]
fn test_focused_border_style() {
    let style = Theme::focused_border();
    assert_eq!(style.fg, Some(Color::Cyan));
}

#[test]
fn test_unfocused_border_style() {
    let style = Theme::unfocused_border();
    assert_eq!(style.fg, Some(Color::DarkGray));
}

#[test]
fn test_header_style() {
    let style = Theme::header();
    assert_eq!(style.fg, Some(Color::Yellow));
    assert!(style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn test_status_bar_style() {
    let style = Theme::status_bar();
    assert_eq!(style.fg, Some(Color::White));
    assert_eq!(style.bg, Some(Color::DarkGray));
}

#[test]
fn test_filter_bar_style() {
    let style = Theme::filter_bar();
    assert_eq!(style.fg, Some(Color::White));
    assert_eq!(style.bg, Some(Color::Black));
}

#[test]
fn test_filter_error_style() {
    let style = Theme::filter_error();
    assert_eq!(style.fg, Some(Color::Red));
}

#[test]
fn test_transport_color_grpc() {
    let color = Theme::transport_color(TransportKind::Grpc);
    assert_eq!(color, Color::Green);
}

#[test]
fn test_transport_color_zmq() {
    let color = Theme::transport_color(TransportKind::Zmq);
    assert_eq!(color, Color::Yellow);
}

#[test]
fn test_transport_color_dds() {
    let color = Theme::transport_color(TransportKind::DdsRtps);
    assert_eq!(color, Color::Magenta);
}

#[test]
fn test_transport_color_raw_tcp() {
    let color = Theme::transport_color(TransportKind::RawTcp);
    assert_eq!(color, Color::Blue);
}

#[test]
fn test_transport_color_raw_udp() {
    let color = Theme::transport_color(TransportKind::RawUdp);
    assert_eq!(color, Color::Cyan);
}

#[test]
fn test_transport_color_json_fixture() {
    let color = Theme::transport_color(TransportKind::JsonFixture);
    assert_eq!(color, Color::White);
}

#[test]
fn test_transport_color_all_kinds() {
    // Test all transport kinds are mapped
    let kinds = vec![
        TransportKind::Grpc,
        TransportKind::Zmq,
        TransportKind::DdsRtps,
        TransportKind::RawTcp,
        TransportKind::RawUdp,
        TransportKind::JsonFixture,
    ];

    for kind in kinds {
        let color = Theme::transport_color(kind);
        // Each should return a valid color (just verify it doesn't panic)
        let _ = color;
    }
}

#[test]
fn test_direction_symbol_inbound() {
    let symbol = Theme::direction_symbol(Direction::Inbound);
    assert_eq!(symbol, "←");
}

#[test]
fn test_direction_symbol_outbound() {
    let symbol = Theme::direction_symbol(Direction::Outbound);
    assert_eq!(symbol, "→");
}

#[test]
fn test_direction_symbol_unknown() {
    let symbol = Theme::direction_symbol(Direction::Unknown);
    assert_eq!(symbol, "?");
}

#[test]
fn test_warning_style() {
    let style = Theme::warning();
    assert_eq!(style.fg, Some(Color::Red));
}

#[test]
fn test_tree_key_style() {
    let style = Theme::tree_key();
    assert_eq!(style.fg, Some(Color::Cyan));
}

#[test]
fn test_tree_value_style() {
    let style = Theme::tree_value();
    assert_eq!(style.fg, Some(Color::White));
}

#[test]
fn test_hex_offset_style() {
    let style = Theme::hex_offset();
    assert_eq!(style.fg, Some(Color::DarkGray));
}

#[test]
fn test_hex_byte_style() {
    let style = Theme::hex_byte();
    assert_eq!(style.fg, Some(Color::White));
}

#[test]
fn test_hex_highlight_style() {
    let style = Theme::hex_highlight();
    assert_eq!(style.fg, Some(Color::Black));
    assert_eq!(style.bg, Some(Color::Yellow));
    assert!(style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn test_hex_ascii_style() {
    let style = Theme::hex_ascii();
    assert_eq!(style.fg, Some(Color::Green));
}

#[test]
fn test_hex_nonprint_style() {
    let style = Theme::hex_nonprint();
    assert_eq!(style.fg, Some(Color::DarkGray));
}

#[test]
fn test_sparkline_style() {
    let style = Theme::sparkline();
    assert_eq!(style.fg, Some(Color::Cyan));
}

#[test]
fn test_help_key_style() {
    let style = Theme::help_key();
    assert_eq!(style.fg, Some(Color::Yellow));
    assert!(style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn test_help_desc_style() {
    let style = Theme::help_desc();
    assert_eq!(style.fg, Some(Color::White));
}

#[test]
fn test_all_styles_are_valid() {
    // Ensure all theme functions return valid Style objects
    let _ = Theme::selected_row();
    let _ = Theme::focused_border();
    let _ = Theme::unfocused_border();
    let _ = Theme::header();
    let _ = Theme::status_bar();
    let _ = Theme::filter_bar();
    let _ = Theme::filter_error();
    let _ = Theme::warning();
    let _ = Theme::tree_key();
    let _ = Theme::tree_value();
    let _ = Theme::hex_offset();
    let _ = Theme::hex_byte();
    let _ = Theme::hex_highlight();
    let _ = Theme::hex_ascii();
    let _ = Theme::hex_nonprint();
    let _ = Theme::sparkline();
    let _ = Theme::help_key();
    let _ = Theme::help_desc();
}

// Tests for ThemeConfig
use prb_tui::theme::ThemeConfig;

#[test]
fn test_theme_config_from_name_dark() {
    let theme = ThemeConfig::from_name("dark");
    assert_eq!(theme.name, "Dark");
}

#[test]
fn test_theme_config_from_name_default() {
    let theme = ThemeConfig::from_name("default");
    assert_eq!(theme.name, "Dark");
}

#[test]
fn test_theme_config_from_name_light() {
    let theme = ThemeConfig::from_name("light");
    assert_eq!(theme.name, "Light");
}

#[test]
fn test_theme_config_from_name_solarized() {
    let theme = ThemeConfig::from_name("solarized");
    assert_eq!(theme.name, "Solarized");
}

#[test]
fn test_theme_config_from_name_solarized_variants() {
    let theme1 = ThemeConfig::from_name("solarized-dark");
    assert_eq!(theme1.name, "Solarized");

    let theme2 = ThemeConfig::from_name("solarized_dark");
    assert_eq!(theme2.name, "Solarized");
}

#[test]
fn test_theme_config_from_name_monokai() {
    let theme = ThemeConfig::from_name("monokai");
    assert_eq!(theme.name, "Monokai");
}

#[test]
fn test_theme_config_from_name_catppuccin() {
    let theme1 = ThemeConfig::from_name("catppuccin-mocha");
    assert_eq!(theme1.name, "Catppuccin Mocha");

    let theme2 = ThemeConfig::from_name("catppuccin_mocha");
    assert_eq!(theme2.name, "Catppuccin Mocha");

    let theme3 = ThemeConfig::from_name("catppuccin mocha");
    assert_eq!(theme3.name, "Catppuccin Mocha");
}

#[test]
fn test_theme_config_from_name_dracula() {
    let theme = ThemeConfig::from_name("dracula");
    assert_eq!(theme.name, "Dracula");
}

#[test]
fn test_theme_config_from_name_colorblind_safe() {
    let theme1 = ThemeConfig::from_name("colorblind-safe");
    assert_eq!(theme1.name, "Colorblind Safe");

    let theme2 = ThemeConfig::from_name("colorblind_safe");
    assert_eq!(theme2.name, "Colorblind Safe");

    let theme3 = ThemeConfig::from_name("colorblind safe");
    assert_eq!(theme3.name, "Colorblind Safe");

    let theme4 = ThemeConfig::from_name("accessible");
    assert_eq!(theme4.name, "Colorblind Safe");
}

#[test]
fn test_theme_config_from_name_deuteranopia() {
    let theme = ThemeConfig::from_name("deuteranopia");
    assert_eq!(theme.name, "Deuteranopia");
}

#[test]
fn test_theme_config_from_name_protanopia() {
    let theme = ThemeConfig::from_name("protanopia");
    assert_eq!(theme.name, "Protanopia");
}

#[test]
fn test_theme_config_from_name_tritanopia() {
    let theme = ThemeConfig::from_name("tritanopia");
    assert_eq!(theme.name, "Tritanopia");
}

#[test]
fn test_theme_config_from_name_high_contrast() {
    let theme1 = ThemeConfig::from_name("high-contrast");
    assert_eq!(theme1.name, "High Contrast");

    let theme2 = ThemeConfig::from_name("high_contrast");
    assert_eq!(theme2.name, "High Contrast");

    let theme3 = ThemeConfig::from_name("high contrast");
    assert_eq!(theme3.name, "High Contrast");
}

#[test]
fn test_theme_config_from_name_unknown_defaults_to_dark() {
    let theme = ThemeConfig::from_name("unknown-theme");
    assert_eq!(theme.name, "Dark");
}

#[test]
fn test_theme_config_from_name_case_insensitive() {
    let theme1 = ThemeConfig::from_name("LIGHT");
    assert_eq!(theme1.name, "Light");

    let theme2 = ThemeConfig::from_name("DaRk");
    assert_eq!(theme2.name, "Dark");
}

#[test]
fn test_theme_config_dark_has_transport_colors() {
    let theme = ThemeConfig::dark();
    assert!(theme.transport_colors.contains_key(&TransportKind::Grpc));
    assert!(theme.transport_colors.contains_key(&TransportKind::Zmq));
    assert!(theme.transport_colors.contains_key(&TransportKind::DdsRtps));
    assert!(theme.transport_colors.contains_key(&TransportKind::RawTcp));
    assert!(theme.transport_colors.contains_key(&TransportKind::RawUdp));
    assert!(
        theme
            .transport_colors
            .contains_key(&TransportKind::JsonFixture)
    );
}

#[test]
fn test_theme_config_light_has_transport_colors() {
    let theme = ThemeConfig::light();
    assert!(theme.transport_colors.contains_key(&TransportKind::Grpc));
    assert_eq!(theme.transport_colors.len(), 6);
}

#[test]
fn test_theme_config_transport_color_method() {
    let theme = ThemeConfig::dark();
    let color = theme.transport_color(TransportKind::Grpc);
    assert_eq!(color, Color::Green);
}

#[test]
fn test_theme_config_transport_color_unknown_kind() {
    let _theme = ThemeConfig::dark();
    // Create a minimal transport map and test fallback
    let mut minimal_theme = ThemeConfig::dark();
    minimal_theme.transport_colors.clear();

    let color = minimal_theme.transport_color(TransportKind::Grpc);
    assert_eq!(color, Color::White); // fallback color
}

#[test]
fn test_theme_config_style_methods() {
    let theme = ThemeConfig::dark();

    let _ = theme.selected_row();
    let _ = theme.zebra_row();
    let _ = theme.normal_row();
    let _ = theme.warning_row();
    let _ = theme.focused_border();
    let _ = theme.unfocused_border();
    let _ = theme.focused_title();
    let _ = theme.unfocused_title();
    let _ = theme.header();
    let _ = theme.status_bar();
    let _ = theme.filter_bar();
    let _ = theme.filter_error();
    let _ = theme.warning();
    let _ = theme.tree_key();
    let _ = theme.tree_value();
    let _ = theme.hex_offset();
    let _ = theme.hex_byte();
    let _ = theme.hex_highlight();
    let _ = theme.hex_search_match();
    let _ = theme.hex_ascii();
    let _ = theme.hex_nonprint();
    let _ = theme.sparkline();
    let _ = theme.help_key();
    let _ = theme.help_desc();
}

#[test]
fn test_all_themes_have_complete_transport_colors() {
    let themes = vec![
        ThemeConfig::dark(),
        ThemeConfig::light(),
        ThemeConfig::solarized(),
        ThemeConfig::monokai(),
        ThemeConfig::catppuccin_mocha(),
        ThemeConfig::dracula(),
        ThemeConfig::colorblind_safe(),
        ThemeConfig::deuteranopia(),
        ThemeConfig::protanopia(),
        ThemeConfig::tritanopia(),
        ThemeConfig::high_contrast(),
    ];

    for theme in themes {
        assert_eq!(
            theme.transport_colors.len(),
            6,
            "Theme {} missing transport colors",
            theme.name
        );
    }
}

#[test]
fn test_theme_config_zebra_row_style() {
    let theme = ThemeConfig::dark();
    let style = theme.zebra_row();
    assert!(style.bg.is_some());
}

#[test]
fn test_theme_config_normal_row_style() {
    let theme = ThemeConfig::dark();
    let style = theme.normal_row();
    assert_eq!(style.bg, Some(Color::Reset));
}

#[test]
fn test_theme_config_warning_row_style() {
    let theme = ThemeConfig::dark();
    let style = theme.warning_row();
    assert!(style.bg.is_some());
}

#[test]
fn test_theme_config_hex_search_match_has_bold() {
    let theme = ThemeConfig::dark();
    let style = theme.hex_search_match();
    assert!(style.add_modifier.contains(Modifier::BOLD));
}
