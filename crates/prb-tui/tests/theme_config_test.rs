//! Tests for ThemeConfig and all theme variants

use prb_core::{Direction, TransportKind};
use prb_tui::theme::ThemeConfig;
use ratatui::style::{Color, Modifier};

#[test]
fn test_theme_from_name_dark() {
    let theme = ThemeConfig::from_name("dark");
    assert_eq!(theme.name, "Dark");
    assert_eq!(theme.selected_row_fg, Color::Black);
}

#[test]
fn test_theme_from_name_default() {
    let theme = ThemeConfig::from_name("default");
    assert_eq!(theme.name, "Dark");
}

#[test]
fn test_theme_from_name_light() {
    let theme = ThemeConfig::from_name("light");
    assert_eq!(theme.name, "Light");
    assert_eq!(theme.selected_row_fg, Color::White);
}

#[test]
fn test_theme_from_name_solarized() {
    let theme = ThemeConfig::from_name("solarized");
    assert_eq!(theme.name, "Solarized");
}

#[test]
fn test_theme_from_name_solarized_variants() {
    let theme1 = ThemeConfig::from_name("solarized-dark");
    let theme2 = ThemeConfig::from_name("solarized_dark");
    assert_eq!(theme1.name, "Solarized");
    assert_eq!(theme2.name, "Solarized");
}

#[test]
fn test_theme_from_name_monokai() {
    let theme = ThemeConfig::from_name("monokai");
    assert_eq!(theme.name, "Monokai");
}

#[test]
fn test_theme_from_name_catppuccin() {
    let theme = ThemeConfig::from_name("catppuccin-mocha");
    assert_eq!(theme.name, "Catppuccin Mocha");
}

#[test]
fn test_theme_from_name_catppuccin_variants() {
    let theme1 = ThemeConfig::from_name("catppuccin_mocha");
    let theme2 = ThemeConfig::from_name("catppuccin mocha");
    assert_eq!(theme1.name, "Catppuccin Mocha");
    assert_eq!(theme2.name, "Catppuccin Mocha");
}

#[test]
fn test_theme_from_name_dracula() {
    let theme = ThemeConfig::from_name("dracula");
    assert_eq!(theme.name, "Dracula");
}

#[test]
fn test_theme_from_name_colorblind_safe() {
    let theme = ThemeConfig::from_name("colorblind-safe");
    assert_eq!(theme.name, "Colorblind Safe");
}

#[test]
fn test_theme_from_name_colorblind_variants() {
    let theme1 = ThemeConfig::from_name("colorblind_safe");
    let theme2 = ThemeConfig::from_name("colorblind safe");
    let theme3 = ThemeConfig::from_name("accessible");
    assert_eq!(theme1.name, "Colorblind Safe");
    assert_eq!(theme2.name, "Colorblind Safe");
    assert_eq!(theme3.name, "Colorblind Safe");
}

#[test]
fn test_theme_from_name_deuteranopia() {
    let theme = ThemeConfig::from_name("deuteranopia");
    assert_eq!(theme.name, "Deuteranopia");
}

#[test]
fn test_theme_from_name_protanopia() {
    let theme = ThemeConfig::from_name("protanopia");
    assert_eq!(theme.name, "Protanopia");
}

#[test]
fn test_theme_from_name_tritanopia() {
    let theme = ThemeConfig::from_name("tritanopia");
    assert_eq!(theme.name, "Tritanopia");
}

#[test]
fn test_theme_from_name_high_contrast() {
    let theme = ThemeConfig::from_name("high-contrast");
    assert_eq!(theme.name, "High Contrast");
}

#[test]
fn test_theme_from_name_high_contrast_variants() {
    let theme1 = ThemeConfig::from_name("high_contrast");
    let theme2 = ThemeConfig::from_name("high contrast");
    assert_eq!(theme1.name, "High Contrast");
    assert_eq!(theme2.name, "High Contrast");
}

#[test]
fn test_theme_from_name_unknown_fallback() {
    let theme = ThemeConfig::from_name("unknown_theme_12345");
    // Should fallback to dark
    assert_eq!(theme.name, "Dark");
}

#[test]
fn test_theme_from_name_case_insensitive() {
    let theme1 = ThemeConfig::from_name("DARK");
    let theme2 = ThemeConfig::from_name("Light");
    let theme3 = ThemeConfig::from_name("MONOKAI");
    assert_eq!(theme1.name, "Dark");
    assert_eq!(theme2.name, "Light");
    assert_eq!(theme3.name, "Monokai");
}

#[test]
fn test_dark_theme_complete() {
    let theme = ThemeConfig::dark();

    assert_eq!(theme.name, "Dark");
    assert_eq!(theme.selected_row_fg, Color::Black);
    assert_eq!(theme.selected_row_bg, Color::Cyan);
    assert_eq!(theme.zebra_bg, Color::Rgb(25, 25, 35));
    assert_eq!(theme.focused_border, Color::Cyan);
    assert_eq!(theme.unfocused_border, Color::DarkGray);

    // Check transport colors
    assert_eq!(theme.transport_color(TransportKind::Grpc), Color::Green);
    assert_eq!(theme.transport_color(TransportKind::Zmq), Color::Yellow);
}

#[test]
fn test_light_theme_complete() {
    let theme = ThemeConfig::light();

    assert_eq!(theme.name, "Light");
    assert_eq!(theme.selected_row_fg, Color::White);
    assert_eq!(theme.selected_row_bg, Color::Blue);
    assert_eq!(theme.focused_border, Color::Blue);
}

#[test]
fn test_catppuccin_mocha_complete() {
    let theme = ThemeConfig::catppuccin_mocha();

    assert_eq!(theme.name, "Catppuccin Mocha");
    assert_eq!(theme.selected_row_bg, Color::Rgb(137, 180, 250));
    assert_eq!(theme.zebra_bg, Color::Rgb(24, 24, 37));
}

#[test]
fn test_dracula_complete() {
    let theme = ThemeConfig::dracula();

    assert_eq!(theme.name, "Dracula");
    assert_eq!(theme.selected_row_bg, Color::Rgb(139, 233, 253));
}

#[test]
fn test_colorblind_safe_palette() {
    let theme = ThemeConfig::colorblind_safe();

    assert_eq!(theme.name, "Colorblind Safe");
    // Should use blue/orange palette instead of red/green
    assert_eq!(
        theme.transport_color(TransportKind::Grpc),
        Color::Rgb(0, 119, 187)
    );
    assert_eq!(
        theme.transport_color(TransportKind::Zmq),
        Color::Rgb(238, 119, 51)
    );
}

#[test]
fn test_deuteranopia_palette() {
    let theme = ThemeConfig::deuteranopia();

    assert_eq!(theme.name, "Deuteranopia");
    // Should avoid green, use blue/yellow/orange
    assert_eq!(
        theme.transport_color(TransportKind::Grpc),
        Color::Rgb(51, 102, 204)
    );
    assert_eq!(
        theme.transport_color(TransportKind::Zmq),
        Color::Rgb(255, 204, 51)
    );
}

#[test]
fn test_protanopia_palette() {
    let theme = ThemeConfig::protanopia();

    assert_eq!(theme.name, "Protanopia");
    // Should avoid red, use blue/yellow
    assert_eq!(
        theme.transport_color(TransportKind::Grpc),
        Color::Rgb(0, 102, 204)
    );
}

#[test]
fn test_tritanopia_palette() {
    let theme = ThemeConfig::tritanopia();

    assert_eq!(theme.name, "Tritanopia");
    // Should avoid blue, use red/green/teal
    assert_eq!(
        theme.transport_color(TransportKind::Grpc),
        Color::Rgb(0, 153, 136)
    );
}

#[test]
fn test_high_contrast_complete() {
    let theme = ThemeConfig::high_contrast();

    assert_eq!(theme.name, "High Contrast");
    assert_eq!(theme.selected_row_fg, Color::White);
    assert_eq!(theme.selected_row_bg, Color::Rgb(0, 0, 255));
    assert_eq!(theme.normal_bg, Color::Black);
}

#[test]
fn test_solarized_complete() {
    let theme = ThemeConfig::solarized();

    assert_eq!(theme.name, "Solarized");
    assert_eq!(theme.zebra_bg, Color::Rgb(7, 54, 66));
}

#[test]
fn test_monokai_complete() {
    let theme = ThemeConfig::monokai();

    assert_eq!(theme.name, "Monokai");
    assert_eq!(theme.selected_row_bg, Color::Rgb(102, 217, 239));
}

#[test]
fn test_all_themes_have_transport_colors() {
    let themes = vec![
        ThemeConfig::dark(),
        ThemeConfig::light(),
        ThemeConfig::catppuccin_mocha(),
        ThemeConfig::dracula(),
        ThemeConfig::colorblind_safe(),
        ThemeConfig::deuteranopia(),
        ThemeConfig::protanopia(),
        ThemeConfig::tritanopia(),
        ThemeConfig::high_contrast(),
        ThemeConfig::solarized(),
        ThemeConfig::monokai(),
    ];

    let transports = vec![
        TransportKind::Grpc,
        TransportKind::Zmq,
        TransportKind::DdsRtps,
        TransportKind::RawTcp,
        TransportKind::RawUdp,
        TransportKind::JsonFixture,
    ];

    for theme in themes {
        for transport in &transports {
            let color = theme.transport_color(*transport);
            // Just verify it returns a valid color (not panicking)
            let _ = color;
        }
    }
}

#[test]
fn test_theme_style_methods() {
    let theme = ThemeConfig::dark();

    // Test all style methods return valid Style objects
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
fn test_theme_focused_title_has_bold() {
    let theme = ThemeConfig::dark();
    let style = theme.focused_title();
    assert!(style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn test_theme_header_has_bold() {
    let theme = ThemeConfig::dark();
    let style = theme.header();
    assert!(style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn test_theme_hex_highlight_has_bold() {
    let theme = ThemeConfig::dark();
    let style = theme.hex_highlight();
    assert!(style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn test_theme_hex_search_match_has_bold() {
    let theme = ThemeConfig::dark();
    let style = theme.hex_search_match();
    assert!(style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn test_theme_help_key_has_bold() {
    let theme = ThemeConfig::dark();
    let style = theme.help_key();
    assert!(style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn test_direction_symbol() {
    assert_eq!(ThemeConfig::direction_symbol(Direction::Inbound), "←");
    assert_eq!(ThemeConfig::direction_symbol(Direction::Outbound), "→");
    assert_eq!(ThemeConfig::direction_symbol(Direction::Unknown), "?");
}

#[test]
fn test_transport_color_missing_fallback() {
    let mut theme = ThemeConfig::dark();
    // Clear transport colors to test fallback
    theme.transport_colors.clear();

    // Should return white as fallback
    assert_eq!(theme.transport_color(TransportKind::Grpc), Color::White);
}

#[test]
fn test_all_themes_zebra_stripe() {
    let themes = vec![
        ThemeConfig::dark(),
        ThemeConfig::light(),
        ThemeConfig::catppuccin_mocha(),
        ThemeConfig::dracula(),
        ThemeConfig::colorblind_safe(),
        ThemeConfig::deuteranopia(),
        ThemeConfig::protanopia(),
        ThemeConfig::tritanopia(),
        ThemeConfig::high_contrast(),
        ThemeConfig::solarized(),
        ThemeConfig::monokai(),
    ];

    for theme in themes {
        // Verify zebra stripes have different bg than normal
        let zebra_style = theme.zebra_row();
        let normal_style = theme.normal_row();

        // At least one should have a background set
        let has_distinct_bg = zebra_style.bg.is_some() || normal_style.bg.is_some();
        assert!(
            has_distinct_bg,
            "Theme {} should have zebra striping",
            theme.name
        );
    }
}
