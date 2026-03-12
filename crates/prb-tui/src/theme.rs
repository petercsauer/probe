use ratatui::style::{Color, Modifier, Style};
use std::collections::HashMap;

use prb_core::TransportKind;

#[derive(Debug, Clone)]
pub struct ThemeConfig {
    pub name: String,
    pub selected_row_fg: Color,
    pub selected_row_bg: Color,
    pub zebra_bg: Color,
    pub normal_bg: Color,
    pub warning_bg: Color,
    pub focused_border: Color,
    pub unfocused_border: Color,
    pub focused_title_fg: Color,
    pub unfocused_title_fg: Color,
    pub header_fg: Color,
    pub status_bar_fg: Color,
    pub status_bar_bg: Color,
    pub filter_bar_fg: Color,
    pub filter_bar_bg: Color,
    pub filter_error_fg: Color,
    pub warning_fg: Color,
    pub tree_key_fg: Color,
    pub tree_value_fg: Color,
    pub hex_offset_fg: Color,
    pub hex_byte_fg: Color,
    pub hex_highlight_fg: Color,
    pub hex_highlight_bg: Color,
    pub hex_search_match_fg: Color,
    pub hex_search_match_bg: Color,
    pub hex_ascii_fg: Color,
    pub hex_nonprint_fg: Color,
    pub sparkline_fg: Color,
    pub help_key_fg: Color,
    pub help_desc_fg: Color,
    pub transport_colors: HashMap<TransportKind, Color>,
}

impl ThemeConfig {
    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "dark" | "default" => Self::dark(),
            "light" => Self::light(),
            "solarized" | "solarized-dark" | "solarized_dark" => Self::solarized(),
            "monokai" => Self::monokai(),
            "catppuccin-mocha" | "catppuccin_mocha" | "catppuccin mocha" => Self::catppuccin_mocha(),
            "dracula" => Self::dracula(),
            "colorblind-safe" | "colorblind_safe" | "colorblind safe" | "accessible" => Self::colorblind_safe(),
            "deuteranopia" => Self::deuteranopia(),
            "protanopia" => Self::protanopia(),
            "tritanopia" => Self::tritanopia(),
            "high-contrast" | "high_contrast" | "high contrast" => Self::high_contrast(),
            _ => {
                tracing::warn!("Unknown theme '{}', defaulting to dark", name);
                Self::dark()
            }
        }
    }

    pub fn dark() -> Self {
        let mut transport_colors = HashMap::new();
        transport_colors.insert(TransportKind::Grpc, Color::Green);
        transport_colors.insert(TransportKind::Zmq, Color::Yellow);
        transport_colors.insert(TransportKind::DdsRtps, Color::Magenta);
        transport_colors.insert(TransportKind::RawTcp, Color::Blue);
        transport_colors.insert(TransportKind::RawUdp, Color::Cyan);
        transport_colors.insert(TransportKind::JsonFixture, Color::White);

        ThemeConfig {
            name: "Dark".to_string(),
            selected_row_fg: Color::Black,
            selected_row_bg: Color::Cyan,
            zebra_bg: Color::Rgb(25, 25, 35),
            normal_bg: Color::Reset,
            warning_bg: Color::Rgb(50, 20, 20),
            focused_border: Color::Cyan,
            unfocused_border: Color::DarkGray,
            focused_title_fg: Color::Cyan,
            unfocused_title_fg: Color::DarkGray,
            header_fg: Color::Yellow,
            status_bar_fg: Color::White,
            status_bar_bg: Color::DarkGray,
            filter_bar_fg: Color::White,
            filter_bar_bg: Color::Black,
            filter_error_fg: Color::Red,
            warning_fg: Color::Red,
            tree_key_fg: Color::Cyan,
            tree_value_fg: Color::White,
            hex_offset_fg: Color::DarkGray,
            hex_byte_fg: Color::White,
            hex_highlight_fg: Color::Black,
            hex_highlight_bg: Color::Yellow,
            hex_search_match_fg: Color::Black,
            hex_search_match_bg: Color::Magenta,
            hex_ascii_fg: Color::Green,
            hex_nonprint_fg: Color::DarkGray,
            sparkline_fg: Color::Cyan,
            help_key_fg: Color::Yellow,
            help_desc_fg: Color::White,
            transport_colors,
        }
    }

    pub fn light() -> Self {
        let mut transport_colors = HashMap::new();
        transport_colors.insert(TransportKind::Grpc, Color::Green);
        transport_colors.insert(TransportKind::Zmq, Color::Rgb(184, 134, 11)); // DarkGoldenrod
        transport_colors.insert(TransportKind::DdsRtps, Color::Magenta);
        transport_colors.insert(TransportKind::RawTcp, Color::Blue);
        transport_colors.insert(TransportKind::RawUdp, Color::Cyan);
        transport_colors.insert(TransportKind::JsonFixture, Color::Black);

        ThemeConfig {
            name: "Light".to_string(),
            selected_row_fg: Color::White,
            selected_row_bg: Color::Blue,
            zebra_bg: Color::Rgb(245, 245, 250),
            normal_bg: Color::Reset,
            warning_bg: Color::Rgb(255, 220, 220),
            focused_border: Color::Blue,
            unfocused_border: Color::Gray,
            focused_title_fg: Color::Blue,
            unfocused_title_fg: Color::Gray,
            header_fg: Color::Rgb(184, 134, 11), // DarkGoldenrod
            status_bar_fg: Color::Black,
            status_bar_bg: Color::Rgb(200, 200, 200),
            filter_bar_fg: Color::Black,
            filter_bar_bg: Color::White,
            filter_error_fg: Color::Red,
            warning_fg: Color::Red,
            tree_key_fg: Color::Blue,
            tree_value_fg: Color::Black,
            hex_offset_fg: Color::Gray,
            hex_byte_fg: Color::Black,
            hex_highlight_fg: Color::White,
            hex_highlight_bg: Color::Rgb(184, 134, 11),
            hex_search_match_fg: Color::White,
            hex_search_match_bg: Color::Magenta,
            hex_ascii_fg: Color::Green,
            hex_nonprint_fg: Color::Gray,
            sparkline_fg: Color::Blue,
            help_key_fg: Color::Rgb(184, 134, 11),
            help_desc_fg: Color::Black,
            transport_colors,
        }
    }

    pub fn catppuccin_mocha() -> Self {
        let mut transport_colors = HashMap::new();
        transport_colors.insert(TransportKind::Grpc, Color::Rgb(166, 227, 161)); // Green
        transport_colors.insert(TransportKind::Zmq, Color::Rgb(249, 226, 175)); // Yellow
        transport_colors.insert(TransportKind::DdsRtps, Color::Rgb(203, 166, 247)); // Mauve
        transport_colors.insert(TransportKind::RawTcp, Color::Rgb(137, 180, 250)); // Blue
        transport_colors.insert(TransportKind::RawUdp, Color::Rgb(148, 226, 213)); // Teal
        transport_colors.insert(TransportKind::JsonFixture, Color::Rgb(205, 214, 244)); // Text

        ThemeConfig {
            name: "Catppuccin Mocha".to_string(),
            selected_row_fg: Color::Rgb(30, 30, 46), // Base
            selected_row_bg: Color::Rgb(137, 180, 250), // Blue
            zebra_bg: Color::Rgb(24, 24, 37), // Mantle
            normal_bg: Color::Reset,
            warning_bg: Color::Rgb(88, 28, 36), // Dark red
            focused_border: Color::Rgb(137, 180, 250), // Blue
            unfocused_border: Color::Rgb(88, 91, 112), // Surface2
            focused_title_fg: Color::Rgb(137, 180, 250), // Blue
            unfocused_title_fg: Color::Rgb(88, 91, 112), // Surface2
            header_fg: Color::Rgb(249, 226, 175), // Yellow
            status_bar_fg: Color::Rgb(205, 214, 244), // Text
            status_bar_bg: Color::Rgb(49, 50, 68), // Surface1
            filter_bar_fg: Color::Rgb(205, 214, 244), // Text
            filter_bar_bg: Color::Rgb(30, 30, 46), // Base
            filter_error_fg: Color::Rgb(243, 139, 168), // Red
            warning_fg: Color::Rgb(243, 139, 168), // Red
            tree_key_fg: Color::Rgb(137, 180, 250), // Blue
            tree_value_fg: Color::Rgb(205, 214, 244), // Text
            hex_offset_fg: Color::Rgb(88, 91, 112), // Surface2
            hex_byte_fg: Color::Rgb(205, 214, 244), // Text
            hex_highlight_fg: Color::Rgb(30, 30, 46), // Base
            hex_highlight_bg: Color::Rgb(249, 226, 175), // Yellow
            hex_search_match_fg: Color::Rgb(30, 30, 46), // Base
            hex_search_match_bg: Color::Rgb(203, 166, 247), // Mauve
            hex_ascii_fg: Color::Rgb(166, 227, 161), // Green
            hex_nonprint_fg: Color::Rgb(88, 91, 112), // Surface2
            sparkline_fg: Color::Rgb(137, 180, 250), // Blue
            help_key_fg: Color::Rgb(249, 226, 175), // Yellow
            help_desc_fg: Color::Rgb(205, 214, 244), // Text
            transport_colors,
        }
    }

    pub fn dracula() -> Self {
        let mut transport_colors = HashMap::new();
        transport_colors.insert(TransportKind::Grpc, Color::Rgb(80, 250, 123)); // Green
        transport_colors.insert(TransportKind::Zmq, Color::Rgb(241, 250, 140)); // Yellow
        transport_colors.insert(TransportKind::DdsRtps, Color::Rgb(189, 147, 249)); // Purple
        transport_colors.insert(TransportKind::RawTcp, Color::Rgb(139, 233, 253)); // Cyan
        transport_colors.insert(TransportKind::RawUdp, Color::Rgb(98, 114, 164)); // Blue
        transport_colors.insert(TransportKind::JsonFixture, Color::Rgb(248, 248, 242)); // Foreground

        ThemeConfig {
            name: "Dracula".to_string(),
            selected_row_fg: Color::Rgb(40, 42, 54), // Background
            selected_row_bg: Color::Rgb(139, 233, 253), // Cyan
            zebra_bg: Color::Rgb(30, 31, 41), // Darker background
            normal_bg: Color::Reset,
            warning_bg: Color::Rgb(68, 23, 36), // Dark red
            focused_border: Color::Rgb(139, 233, 253), // Cyan
            unfocused_border: Color::Rgb(68, 71, 90), // Current Line
            focused_title_fg: Color::Rgb(139, 233, 253), // Cyan
            unfocused_title_fg: Color::Rgb(98, 114, 164), // Comment
            header_fg: Color::Rgb(241, 250, 140), // Yellow
            status_bar_fg: Color::Rgb(248, 248, 242), // Foreground
            status_bar_bg: Color::Rgb(68, 71, 90), // Current Line
            filter_bar_fg: Color::Rgb(248, 248, 242), // Foreground
            filter_bar_bg: Color::Rgb(40, 42, 54), // Background
            filter_error_fg: Color::Rgb(255, 85, 85), // Red
            warning_fg: Color::Rgb(255, 85, 85), // Red
            tree_key_fg: Color::Rgb(139, 233, 253), // Cyan
            tree_value_fg: Color::Rgb(248, 248, 242), // Foreground
            hex_offset_fg: Color::Rgb(98, 114, 164), // Comment
            hex_byte_fg: Color::Rgb(248, 248, 242), // Foreground
            hex_highlight_fg: Color::Rgb(40, 42, 54), // Background
            hex_highlight_bg: Color::Rgb(241, 250, 140), // Yellow
            hex_search_match_fg: Color::Rgb(40, 42, 54), // Background
            hex_search_match_bg: Color::Rgb(189, 147, 249), // Purple
            hex_ascii_fg: Color::Rgb(80, 250, 123), // Green
            hex_nonprint_fg: Color::Rgb(98, 114, 164), // Comment
            sparkline_fg: Color::Rgb(139, 233, 253), // Cyan
            help_key_fg: Color::Rgb(241, 250, 140), // Yellow
            help_desc_fg: Color::Rgb(248, 248, 242), // Foreground
            transport_colors,
        }
    }

    pub fn colorblind_safe() -> Self {
        let mut transport_colors = HashMap::new();
        // Colorblind-safe palette: avoids red/green confusion
        transport_colors.insert(TransportKind::Grpc, Color::Rgb(0, 119, 187)); // Blue
        transport_colors.insert(TransportKind::Zmq, Color::Rgb(238, 119, 51)); // Orange
        transport_colors.insert(TransportKind::DdsRtps, Color::Rgb(0, 153, 136)); // Teal
        transport_colors.insert(TransportKind::RawTcp, Color::Rgb(204, 187, 68)); // Yellow
        transport_colors.insert(TransportKind::RawUdp, Color::Rgb(170, 51, 119)); // Purple
        transport_colors.insert(TransportKind::JsonFixture, Color::White);

        ThemeConfig {
            name: "Colorblind Safe".to_string(),
            selected_row_fg: Color::Black,
            selected_row_bg: Color::Rgb(0, 119, 187), // Blue (matches gRPC)
            zebra_bg: Color::Rgb(25, 25, 35),
            normal_bg: Color::Reset,
            warning_bg: Color::Rgb(50, 20, 20),
            focused_border: Color::Rgb(0, 119, 187), // Blue
            unfocused_border: Color::DarkGray,
            focused_title_fg: Color::Rgb(0, 119, 187), // Blue
            unfocused_title_fg: Color::DarkGray,
            header_fg: Color::Rgb(238, 119, 51), // Orange
            status_bar_fg: Color::White,
            status_bar_bg: Color::DarkGray,
            filter_bar_fg: Color::White,
            filter_bar_bg: Color::Black,
            filter_error_fg: Color::Rgb(238, 119, 51), // Orange instead of red
            warning_fg: Color::Rgb(238, 119, 51), // Orange instead of red
            tree_key_fg: Color::Rgb(0, 119, 187), // Blue
            tree_value_fg: Color::White,
            hex_offset_fg: Color::DarkGray,
            hex_byte_fg: Color::White,
            hex_highlight_fg: Color::Black,
            hex_highlight_bg: Color::Rgb(204, 187, 68), // Yellow
            hex_search_match_fg: Color::Black,
            hex_search_match_bg: Color::Rgb(170, 51, 119), // Purple
            hex_ascii_fg: Color::Rgb(0, 153, 136), // Teal
            hex_nonprint_fg: Color::DarkGray,
            sparkline_fg: Color::Rgb(0, 119, 187), // Blue
            help_key_fg: Color::Rgb(238, 119, 51), // Orange
            help_desc_fg: Color::White,
            transport_colors,
        }
    }

    pub fn deuteranopia() -> Self {
        let mut transport_colors = HashMap::new();
        // Deuteranopia (no green cones): Use blue/yellow/orange palette
        transport_colors.insert(TransportKind::Grpc, Color::Rgb(51, 102, 204)); // Blue
        transport_colors.insert(TransportKind::Zmq, Color::Rgb(255, 204, 51)); // Yellow
        transport_colors.insert(TransportKind::DdsRtps, Color::Rgb(153, 102, 204)); // Purple
        transport_colors.insert(TransportKind::RawTcp, Color::Rgb(51, 153, 204)); // Cyan
        transport_colors.insert(TransportKind::RawUdp, Color::Rgb(255, 153, 51)); // Orange
        transport_colors.insert(TransportKind::JsonFixture, Color::White);

        ThemeConfig {
            name: "Deuteranopia".to_string(),
            selected_row_fg: Color::Black,
            selected_row_bg: Color::Rgb(51, 102, 204), // Blue
            zebra_bg: Color::Rgb(25, 25, 35),
            normal_bg: Color::Reset,
            warning_bg: Color::Rgb(50, 30, 20),
            focused_border: Color::Rgb(51, 102, 204), // Blue
            unfocused_border: Color::DarkGray,
            focused_title_fg: Color::Rgb(51, 102, 204), // Blue
            unfocused_title_fg: Color::DarkGray,
            header_fg: Color::Rgb(255, 204, 51), // Yellow
            status_bar_fg: Color::White,
            status_bar_bg: Color::DarkGray,
            filter_bar_fg: Color::White,
            filter_bar_bg: Color::Black,
            filter_error_fg: Color::Rgb(255, 153, 51), // Orange instead of red
            warning_fg: Color::Rgb(255, 153, 51), // Orange instead of red
            tree_key_fg: Color::Rgb(51, 102, 204), // Blue
            tree_value_fg: Color::White,
            hex_offset_fg: Color::DarkGray,
            hex_byte_fg: Color::White,
            hex_highlight_fg: Color::Black,
            hex_highlight_bg: Color::Rgb(255, 204, 51), // Yellow
            hex_search_match_fg: Color::Black,
            hex_search_match_bg: Color::Rgb(153, 102, 204), // Purple
            hex_ascii_fg: Color::Rgb(51, 153, 204), // Cyan
            hex_nonprint_fg: Color::DarkGray,
            sparkline_fg: Color::Rgb(51, 102, 204), // Blue
            help_key_fg: Color::Rgb(255, 204, 51), // Yellow
            help_desc_fg: Color::White,
            transport_colors,
        }
    }

    pub fn protanopia() -> Self {
        let mut transport_colors = HashMap::new();
        // Protanopia (no red cones): Use blue/yellow palette
        transport_colors.insert(TransportKind::Grpc, Color::Rgb(0, 102, 204)); // Blue
        transport_colors.insert(TransportKind::Zmq, Color::Rgb(255, 221, 51)); // Yellow
        transport_colors.insert(TransportKind::DdsRtps, Color::Rgb(102, 153, 204)); // Light blue
        transport_colors.insert(TransportKind::RawTcp, Color::Rgb(0, 153, 204)); // Cyan
        transport_colors.insert(TransportKind::RawUdp, Color::Rgb(204, 187, 51)); // Gold
        transport_colors.insert(TransportKind::JsonFixture, Color::White);

        ThemeConfig {
            name: "Protanopia".to_string(),
            selected_row_fg: Color::Black,
            selected_row_bg: Color::Rgb(0, 102, 204), // Blue
            zebra_bg: Color::Rgb(25, 25, 35),
            normal_bg: Color::Reset,
            warning_bg: Color::Rgb(40, 35, 20),
            focused_border: Color::Rgb(0, 102, 204), // Blue
            unfocused_border: Color::DarkGray,
            focused_title_fg: Color::Rgb(0, 102, 204), // Blue
            unfocused_title_fg: Color::DarkGray,
            header_fg: Color::Rgb(255, 221, 51), // Yellow
            status_bar_fg: Color::White,
            status_bar_bg: Color::DarkGray,
            filter_bar_fg: Color::White,
            filter_bar_bg: Color::Black,
            filter_error_fg: Color::Rgb(204, 187, 51), // Gold instead of red
            warning_fg: Color::Rgb(204, 187, 51), // Gold instead of red
            tree_key_fg: Color::Rgb(0, 102, 204), // Blue
            tree_value_fg: Color::White,
            hex_offset_fg: Color::DarkGray,
            hex_byte_fg: Color::White,
            hex_highlight_fg: Color::Black,
            hex_highlight_bg: Color::Rgb(255, 221, 51), // Yellow
            hex_search_match_fg: Color::Black,
            hex_search_match_bg: Color::Rgb(102, 153, 204), // Light blue
            hex_ascii_fg: Color::Rgb(0, 153, 204), // Cyan
            hex_nonprint_fg: Color::DarkGray,
            sparkline_fg: Color::Rgb(0, 102, 204), // Blue
            help_key_fg: Color::Rgb(255, 221, 51), // Yellow
            help_desc_fg: Color::White,
            transport_colors,
        }
    }

    pub fn tritanopia() -> Self {
        let mut transport_colors = HashMap::new();
        // Tritanopia (no blue cones): Use red/pink and green/teal palette
        transport_colors.insert(TransportKind::Grpc, Color::Rgb(0, 153, 136)); // Teal
        transport_colors.insert(TransportKind::Zmq, Color::Rgb(204, 51, 102)); // Pink
        transport_colors.insert(TransportKind::DdsRtps, Color::Rgb(153, 0, 102)); // Magenta
        transport_colors.insert(TransportKind::RawTcp, Color::Rgb(0, 187, 153)); // Cyan-green
        transport_colors.insert(TransportKind::RawUdp, Color::Rgb(255, 51, 51)); // Red
        transport_colors.insert(TransportKind::JsonFixture, Color::White);

        ThemeConfig {
            name: "Tritanopia".to_string(),
            selected_row_fg: Color::Black,
            selected_row_bg: Color::Rgb(0, 153, 136), // Teal
            zebra_bg: Color::Rgb(25, 25, 35),
            normal_bg: Color::Reset,
            warning_bg: Color::Rgb(50, 20, 30),
            focused_border: Color::Rgb(0, 153, 136), // Teal
            unfocused_border: Color::DarkGray,
            focused_title_fg: Color::Rgb(0, 153, 136), // Teal
            unfocused_title_fg: Color::DarkGray,
            header_fg: Color::Rgb(204, 51, 102), // Pink
            status_bar_fg: Color::White,
            status_bar_bg: Color::DarkGray,
            filter_bar_fg: Color::White,
            filter_bar_bg: Color::Black,
            filter_error_fg: Color::Rgb(255, 51, 51), // Red
            warning_fg: Color::Rgb(255, 51, 51), // Red
            tree_key_fg: Color::Rgb(0, 153, 136), // Teal
            tree_value_fg: Color::White,
            hex_offset_fg: Color::DarkGray,
            hex_byte_fg: Color::White,
            hex_highlight_fg: Color::Black,
            hex_highlight_bg: Color::Rgb(204, 51, 102), // Pink
            hex_search_match_fg: Color::Black,
            hex_search_match_bg: Color::Rgb(153, 0, 102), // Magenta
            hex_ascii_fg: Color::Rgb(0, 187, 153), // Cyan-green
            hex_nonprint_fg: Color::DarkGray,
            sparkline_fg: Color::Rgb(0, 153, 136), // Teal
            help_key_fg: Color::Rgb(204, 51, 102), // Pink
            help_desc_fg: Color::White,
            transport_colors,
        }
    }

    pub fn high_contrast() -> Self {
        let mut transport_colors = HashMap::new();
        // High contrast with maximum luminance difference
        transport_colors.insert(TransportKind::Grpc, Color::Rgb(100, 200, 255)); // Bright blue
        transport_colors.insert(TransportKind::Zmq, Color::Rgb(255, 255, 100)); // Bright yellow
        transport_colors.insert(TransportKind::DdsRtps, Color::Rgb(100, 255, 200)); // Bright cyan
        transport_colors.insert(TransportKind::RawTcp, Color::Rgb(255, 150, 255)); // Bright magenta
        transport_colors.insert(TransportKind::RawUdp, Color::Rgb(255, 150, 100)); // Bright orange
        transport_colors.insert(TransportKind::JsonFixture, Color::White);

        ThemeConfig {
            name: "High Contrast".to_string(),
            selected_row_fg: Color::White,
            selected_row_bg: Color::Rgb(0, 0, 255), // Pure blue
            zebra_bg: Color::Rgb(20, 20, 20), // Very dark gray
            normal_bg: Color::Black,
            warning_bg: Color::Rgb(80, 0, 0), // Dark red
            focused_border: Color::White,
            unfocused_border: Color::Rgb(128, 128, 128), // Mid gray
            focused_title_fg: Color::White,
            unfocused_title_fg: Color::Rgb(180, 180, 180), // Light gray
            header_fg: Color::White,
            status_bar_fg: Color::White,
            status_bar_bg: Color::Black,
            filter_bar_fg: Color::White,
            filter_bar_bg: Color::Black,
            filter_error_fg: Color::Rgb(255, 100, 100), // Bright red
            warning_fg: Color::Rgb(255, 100, 100), // Bright red
            tree_key_fg: Color::White,
            tree_value_fg: Color::Rgb(200, 200, 200), // Light gray
            hex_offset_fg: Color::Rgb(150, 150, 150), // Mid-light gray
            hex_byte_fg: Color::White,
            hex_highlight_fg: Color::Black,
            hex_highlight_bg: Color::White,
            hex_search_match_fg: Color::Black,
            hex_search_match_bg: Color::Rgb(255, 255, 0), // Bright yellow
            hex_ascii_fg: Color::Rgb(150, 255, 150), // Bright green
            hex_nonprint_fg: Color::Rgb(120, 120, 120), // Gray
            sparkline_fg: Color::White,
            help_key_fg: Color::White,
            help_desc_fg: Color::Rgb(200, 200, 200), // Light gray
            transport_colors,
        }
    }

    pub fn solarized() -> Self {
        let mut transport_colors = HashMap::new();
        // Solarized dark palette
        transport_colors.insert(TransportKind::Grpc, Color::Rgb(133, 153, 0)); // Green
        transport_colors.insert(TransportKind::Zmq, Color::Rgb(181, 137, 0)); // Yellow
        transport_colors.insert(TransportKind::DdsRtps, Color::Rgb(211, 54, 130)); // Magenta
        transport_colors.insert(TransportKind::RawTcp, Color::Rgb(38, 139, 210)); // Blue
        transport_colors.insert(TransportKind::RawUdp, Color::Rgb(42, 161, 152)); // Cyan
        transport_colors.insert(TransportKind::JsonFixture, Color::Rgb(131, 148, 150)); // Base0

        ThemeConfig {
            name: "Solarized".to_string(),
            selected_row_fg: Color::Rgb(0, 43, 54), // Base03
            selected_row_bg: Color::Rgb(38, 139, 210), // Blue
            zebra_bg: Color::Rgb(7, 54, 66), // Base02
            normal_bg: Color::Reset,
            warning_bg: Color::Rgb(88, 28, 36), // Dark red
            focused_border: Color::Rgb(38, 139, 210), // Blue
            unfocused_border: Color::Rgb(88, 110, 117), // Base01
            focused_title_fg: Color::Rgb(38, 139, 210), // Blue
            unfocused_title_fg: Color::Rgb(88, 110, 117), // Base01
            header_fg: Color::Rgb(181, 137, 0), // Yellow
            status_bar_fg: Color::Rgb(131, 148, 150), // Base0
            status_bar_bg: Color::Rgb(7, 54, 66), // Base02
            filter_bar_fg: Color::Rgb(131, 148, 150), // Base0
            filter_bar_bg: Color::Rgb(0, 43, 54), // Base03
            filter_error_fg: Color::Rgb(220, 50, 47), // Red
            warning_fg: Color::Rgb(220, 50, 47), // Red
            tree_key_fg: Color::Rgb(38, 139, 210), // Blue
            tree_value_fg: Color::Rgb(131, 148, 150), // Base0
            hex_offset_fg: Color::Rgb(88, 110, 117), // Base01
            hex_byte_fg: Color::Rgb(131, 148, 150), // Base0
            hex_highlight_fg: Color::Rgb(0, 43, 54), // Base03
            hex_highlight_bg: Color::Rgb(181, 137, 0), // Yellow
            hex_search_match_fg: Color::Rgb(0, 43, 54), // Base03
            hex_search_match_bg: Color::Rgb(211, 54, 130), // Magenta
            hex_ascii_fg: Color::Rgb(133, 153, 0), // Green
            hex_nonprint_fg: Color::Rgb(88, 110, 117), // Base01
            sparkline_fg: Color::Rgb(38, 139, 210), // Blue
            help_key_fg: Color::Rgb(181, 137, 0), // Yellow
            help_desc_fg: Color::Rgb(131, 148, 150), // Base0
            transport_colors,
        }
    }

    pub fn monokai() -> Self {
        let mut transport_colors = HashMap::new();
        // Monokai palette
        transport_colors.insert(TransportKind::Grpc, Color::Rgb(166, 226, 46)); // Green
        transport_colors.insert(TransportKind::Zmq, Color::Rgb(230, 219, 116)); // Yellow
        transport_colors.insert(TransportKind::DdsRtps, Color::Rgb(174, 129, 255)); // Purple
        transport_colors.insert(TransportKind::RawTcp, Color::Rgb(102, 217, 239)); // Blue/Cyan
        transport_colors.insert(TransportKind::RawUdp, Color::Rgb(249, 38, 114)); // Pink
        transport_colors.insert(TransportKind::JsonFixture, Color::Rgb(248, 248, 242)); // Foreground

        ThemeConfig {
            name: "Monokai".to_string(),
            selected_row_fg: Color::Rgb(39, 40, 34), // Background
            selected_row_bg: Color::Rgb(102, 217, 239), // Blue/Cyan
            zebra_bg: Color::Rgb(30, 31, 27), // Darker background
            normal_bg: Color::Reset,
            warning_bg: Color::Rgb(68, 23, 36), // Dark red
            focused_border: Color::Rgb(102, 217, 239), // Blue/Cyan
            unfocused_border: Color::Rgb(117, 113, 94), // Comment
            focused_title_fg: Color::Rgb(102, 217, 239), // Blue/Cyan
            unfocused_title_fg: Color::Rgb(117, 113, 94), // Comment
            header_fg: Color::Rgb(230, 219, 116), // Yellow
            status_bar_fg: Color::Rgb(248, 248, 242), // Foreground
            status_bar_bg: Color::Rgb(73, 72, 62), // Line highlight
            filter_bar_fg: Color::Rgb(248, 248, 242), // Foreground
            filter_bar_bg: Color::Rgb(39, 40, 34), // Background
            filter_error_fg: Color::Rgb(249, 38, 114), // Pink
            warning_fg: Color::Rgb(249, 38, 114), // Pink
            tree_key_fg: Color::Rgb(102, 217, 239), // Blue/Cyan
            tree_value_fg: Color::Rgb(248, 248, 242), // Foreground
            hex_offset_fg: Color::Rgb(117, 113, 94), // Comment
            hex_byte_fg: Color::Rgb(248, 248, 242), // Foreground
            hex_highlight_fg: Color::Rgb(39, 40, 34), // Background
            hex_highlight_bg: Color::Rgb(230, 219, 116), // Yellow
            hex_search_match_fg: Color::Rgb(39, 40, 34), // Background
            hex_search_match_bg: Color::Rgb(174, 129, 255), // Purple
            hex_ascii_fg: Color::Rgb(166, 226, 46), // Green
            hex_nonprint_fg: Color::Rgb(117, 113, 94), // Comment
            sparkline_fg: Color::Rgb(102, 217, 239), // Blue/Cyan
            help_key_fg: Color::Rgb(230, 219, 116), // Yellow
            help_desc_fg: Color::Rgb(248, 248, 242), // Foreground
            transport_colors,
        }
    }

    pub fn selected_row(&self) -> Style {
        Style::default()
            .fg(self.selected_row_fg)
            .bg(self.selected_row_bg)
    }

    pub fn zebra_row(&self) -> Style {
        Style::default().bg(self.zebra_bg)
    }

    pub fn normal_row(&self) -> Style {
        Style::default().bg(self.normal_bg)
    }

    pub fn warning_row(&self) -> Style {
        Style::default().bg(self.warning_bg)
    }

    pub fn focused_border(&self) -> Style {
        Style::default().fg(self.focused_border)
    }

    pub fn unfocused_border(&self) -> Style {
        Style::default().fg(self.unfocused_border)
    }

    pub fn focused_title(&self) -> Style {
        Style::default()
            .fg(self.focused_title_fg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn unfocused_title(&self) -> Style {
        Style::default().fg(self.unfocused_title_fg)
    }

    pub fn header(&self) -> Style {
        Style::default()
            .fg(self.header_fg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn status_bar(&self) -> Style {
        Style::default()
            .fg(self.status_bar_fg)
            .bg(self.status_bar_bg)
    }

    pub fn filter_bar(&self) -> Style {
        Style::default()
            .fg(self.filter_bar_fg)
            .bg(self.filter_bar_bg)
    }

    pub fn filter_error(&self) -> Style {
        Style::default().fg(self.filter_error_fg)
    }

    pub fn transport_color(&self, kind: TransportKind) -> Color {
        self.transport_colors
            .get(&kind)
            .copied()
            .unwrap_or(Color::White)
    }

    pub fn direction_symbol(dir: prb_core::Direction) -> &'static str {
        match dir {
            prb_core::Direction::Inbound => "←",
            prb_core::Direction::Outbound => "→",
            prb_core::Direction::Unknown => "?",
        }
    }

    pub fn warning(&self) -> Style {
        Style::default().fg(self.warning_fg)
    }

    pub fn tree_key(&self) -> Style {
        Style::default().fg(self.tree_key_fg)
    }

    pub fn tree_value(&self) -> Style {
        Style::default().fg(self.tree_value_fg)
    }

    pub fn hex_offset(&self) -> Style {
        Style::default().fg(self.hex_offset_fg)
    }

    pub fn hex_byte(&self) -> Style {
        Style::default().fg(self.hex_byte_fg)
    }

    pub fn hex_highlight(&self) -> Style {
        Style::default()
            .fg(self.hex_highlight_fg)
            .bg(self.hex_highlight_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn hex_search_match(&self) -> Style {
        Style::default()
            .fg(self.hex_search_match_fg)
            .bg(self.hex_search_match_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn hex_ascii(&self) -> Style {
        Style::default().fg(self.hex_ascii_fg)
    }

    pub fn hex_nonprint(&self) -> Style {
        Style::default().fg(self.hex_nonprint_fg)
    }

    pub fn sparkline(&self) -> Style {
        Style::default().fg(self.sparkline_fg)
    }

    pub fn help_key(&self) -> Style {
        Style::default()
            .fg(self.help_key_fg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn help_desc(&self) -> Style {
        Style::default().fg(self.help_desc_fg)
    }
}

// Preserve old Theme struct for compatibility during migration
pub struct Theme;

impl Theme {
    pub fn selected_row() -> Style {
        ThemeConfig::dark().selected_row()
    }

    pub fn zebra_row() -> Style {
        ThemeConfig::dark().zebra_row()
    }

    pub fn normal_row() -> Style {
        ThemeConfig::dark().normal_row()
    }

    pub fn warning_row() -> Style {
        ThemeConfig::dark().warning_row()
    }

    pub fn focused_border() -> Style {
        ThemeConfig::dark().focused_border()
    }

    pub fn unfocused_border() -> Style {
        ThemeConfig::dark().unfocused_border()
    }

    pub fn focused_title() -> Style {
        ThemeConfig::dark().focused_title()
    }

    pub fn unfocused_title() -> Style {
        ThemeConfig::dark().unfocused_title()
    }

    pub fn header() -> Style {
        ThemeConfig::dark().header()
    }

    pub fn status_bar() -> Style {
        ThemeConfig::dark().status_bar()
    }

    pub fn filter_bar() -> Style {
        ThemeConfig::dark().filter_bar()
    }

    pub fn filter_error() -> Style {
        ThemeConfig::dark().filter_error()
    }

    pub fn transport_color(kind: TransportKind) -> Color {
        ThemeConfig::dark().transport_color(kind)
    }

    pub fn direction_symbol(dir: prb_core::Direction) -> &'static str {
        ThemeConfig::direction_symbol(dir)
    }

    pub fn warning() -> Style {
        ThemeConfig::dark().warning()
    }

    pub fn tree_key() -> Style {
        ThemeConfig::dark().tree_key()
    }

    pub fn tree_value() -> Style {
        ThemeConfig::dark().tree_value()
    }

    pub fn hex_offset() -> Style {
        ThemeConfig::dark().hex_offset()
    }

    pub fn hex_byte() -> Style {
        ThemeConfig::dark().hex_byte()
    }

    pub fn hex_highlight() -> Style {
        ThemeConfig::dark().hex_highlight()
    }

    pub fn hex_search_match() -> Style {
        ThemeConfig::dark().hex_search_match()
    }

    pub fn hex_ascii() -> Style {
        ThemeConfig::dark().hex_ascii()
    }

    pub fn hex_nonprint() -> Style {
        ThemeConfig::dark().hex_nonprint()
    }

    pub fn sparkline() -> Style {
        ThemeConfig::dark().sparkline()
    }

    pub fn help_key() -> Style {
        ThemeConfig::dark().help_key()
    }

    pub fn help_desc() -> Style {
        ThemeConfig::dark().help_desc()
    }
}
