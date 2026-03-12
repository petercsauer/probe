//! Welcome screen overlay shown when launching with no data.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

/// Welcome overlay shown on first launch or when no file is loaded.
pub struct WelcomeOverlay;

impl WelcomeOverlay {
    pub fn render(area: Rect, buf: &mut Buffer) {
        let width = 60u16.min(area.width.saturating_sub(4));
        let height = 16u16.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let overlay_area = Rect::new(x, y, width, height);

        Clear.render(overlay_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Welcome to prb ");

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Universal Message Debugger",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::raw("Usage: "),
                Span::styled("prb tui <file>", Style::default().fg(Color::Yellow)),
            ]),
            Line::from(""),
            Line::from("Supported formats:"),
            Line::from("  • JSON fixtures (.json)"),
            Line::from("  • Packet captures (.pcap, .pcapng)"),
            Line::from("  • MCAP recordings (.mcap)"),
            Line::from(""),
            Line::from(vec![
                Span::raw("Try demo mode: "),
                Span::styled("prb tui --demo", Style::default().fg(Color::Green)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Press any key to continue",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )),
        ];

        let paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .style(Style::default());

        paragraph.render(inner, buf);
    }
}
