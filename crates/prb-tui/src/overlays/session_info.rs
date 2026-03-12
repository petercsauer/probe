//! Session information overlay showing MCAP metadata and file statistics.

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

use prb_storage::SessionMetadata;

/// Session information to display.
pub struct SessionInfo {
    pub file_path: String,
    pub file_size: u64,
    pub event_count: usize,
    pub time_range: Option<(u64, u64)>, // (start_us, end_us)
    pub metadata: Option<SessionMetadata>,
    pub channel_info: Option<Vec<ChannelDisplay>>,
}

/// Channel information for display.
pub struct ChannelDisplay {
    pub topic: String,
    pub message_count: u64,
}

/// Session info overlay showing file and MCAP metadata.
pub struct SessionInfoOverlay;

impl SessionInfoOverlay {
    /// Render the session info overlay.
    pub fn render(area: Rect, buf: &mut Buffer, session_info: &SessionInfo) {
        let width = 70u16.min(area.width.saturating_sub(4));
        let height = 20u16.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let overlay_area = Rect::new(x, y, width, height);

        Clear.render(overlay_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Session Info ");

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        let mut lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "File: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&session_info.file_path),
            ]),
        ];

        // File size
        lines.push(Line::from(vec![
            Span::styled(
                "Size: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format_file_size(session_info.file_size)),
        ]));

        // Event count
        lines.push(Line::from(vec![
            Span::styled(
                "Events: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("{}", session_info.event_count)),
        ]));

        // Time range
        if let Some((start_us, end_us)) = session_info.time_range {
            let duration_us = end_us.saturating_sub(start_us);
            lines.push(Line::from(vec![
                Span::styled(
                    "Duration: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format_duration(duration_us)),
            ]));

            // Format timestamp
            let start_time = format_timestamp(start_us);
            lines.push(Line::from(vec![
                Span::styled(
                    "Captured: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(start_time),
            ]));
        }

        // MCAP-specific metadata
        if let Some(ref metadata) = session_info.metadata {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "MCAP Metadata:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));

            if let Some(ref tool) = metadata.capture_tool {
                lines.push(Line::from(vec![
                    Span::raw("  Tool: "),
                    Span::styled(tool, Style::default().fg(Color::Green)),
                ]));
            }

            lines.push(Line::from(vec![
                Span::raw("  Version: "),
                Span::styled(&metadata.tool_version, Style::default().fg(Color::Green)),
            ]));

            if let Some(ref source) = metadata.source_file {
                lines.push(Line::from(vec![Span::raw("  Source: "), Span::raw(source)]));
            }
        }

        // Channel information
        if let Some(ref channels) = session_info.channel_info
            && !channels.is_empty()
        {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(
                    "Channels: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}", channels.len()),
                    Style::default().fg(Color::Yellow),
                ),
            ]));

            for channel in channels.iter().take(5) {
                lines.push(Line::from(vec![
                    Span::raw("  • "),
                    Span::styled(&channel.topic, Style::default().fg(Color::Green)),
                    Span::raw(format!(" ({} messages)", channel.message_count)),
                ]));
            }

            if channels.len() > 5 {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("... and {} more", channels.len() - 5),
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press ESC or 'i' to close",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )));

        let paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .alignment(Alignment::Left);

        paragraph.render(inner, buf);
    }
}

fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

fn format_duration(duration_us: u64) -> String {
    let seconds = duration_us / 1_000_000;
    let minutes = seconds / 60;
    let hours = minutes / 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes % 60, seconds % 60)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds % 60)
    } else {
        format!("{}.{:03}s", seconds, (duration_us % 1_000_000) / 1000)
    }
}

fn format_timestamp(timestamp_us: u64) -> String {
    // Convert microseconds to a human-readable format
    // Format: YYYY-MM-DD HH:MM:SS (approximation from Unix timestamp)
    let secs = timestamp_us / 1_000_000;

    // Simple date formatting from Unix timestamp
    // This is a basic implementation; for production use a proper time library
    let days_since_epoch = secs / 86400;
    let remaining_secs = secs % 86400;
    let hours = remaining_secs / 3600;
    let minutes = (remaining_secs % 3600) / 60;
    let seconds = remaining_secs % 60;

    // Approximate year calculation (simplified, not accounting for leap years properly)
    let years_since_epoch = days_since_epoch / 365;
    let year = 1970 + years_since_epoch;

    format!("{} {:02}:{:02}:{:02} UTC", year, hours, minutes, seconds)
}
