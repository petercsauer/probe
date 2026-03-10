use ratatui::style::{Color, Modifier, Style};

use prb_core::TransportKind;

pub struct Theme;

impl Theme {
    pub fn selected_row() -> Style {
        Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    }

    pub fn focused_border() -> Style {
        Style::default().fg(Color::Cyan)
    }

    pub fn unfocused_border() -> Style {
        Style::default().fg(Color::DarkGray)
    }

    pub fn header() -> Style {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    }

    pub fn status_bar() -> Style {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    }

    pub fn filter_bar() -> Style {
        Style::default().fg(Color::White).bg(Color::Black)
    }

    pub fn filter_error() -> Style {
        Style::default().fg(Color::Red)
    }

    pub fn transport_color(kind: TransportKind) -> Color {
        match kind {
            TransportKind::Grpc => Color::Green,
            TransportKind::Zmq => Color::Yellow,
            TransportKind::DdsRtps => Color::Magenta,
            TransportKind::RawTcp => Color::Blue,
            TransportKind::RawUdp => Color::Cyan,
            TransportKind::JsonFixture => Color::White,
        }
    }

    pub fn direction_symbol(dir: prb_core::Direction) -> &'static str {
        match dir {
            prb_core::Direction::Inbound => "←",
            prb_core::Direction::Outbound => "→",
            prb_core::Direction::Unknown => "?",
        }
    }

    pub fn warning() -> Style {
        Style::default().fg(Color::Red)
    }

    pub fn tree_key() -> Style {
        Style::default().fg(Color::Cyan)
    }

    pub fn tree_value() -> Style {
        Style::default().fg(Color::White)
    }

    pub fn hex_offset() -> Style {
        Style::default().fg(Color::DarkGray)
    }

    pub fn hex_byte() -> Style {
        Style::default().fg(Color::White)
    }

    pub fn hex_highlight() -> Style {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    }

    pub fn hex_ascii() -> Style {
        Style::default().fg(Color::Green)
    }

    pub fn hex_nonprint() -> Style {
        Style::default().fg(Color::DarkGray)
    }

    pub fn sparkline() -> Style {
        Style::default().fg(Color::Cyan)
    }

    pub fn help_key() -> Style {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    }

    pub fn help_desc() -> Style {
        Style::default().fg(Color::White)
    }
}
