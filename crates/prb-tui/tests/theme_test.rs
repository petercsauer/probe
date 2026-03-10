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
