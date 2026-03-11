//! Follow stream overlay for visualizing conversation flow.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget,
};
use ratatui::style::{Color, Style};

use crate::theme::ThemeConfig;
use prb_core::{conversation::Conversation, DebugEvent, Direction};

/// Follow stream overlay showing detailed conversation flow.
pub struct FollowStreamOverlay {
    pub conversation_idx: usize,
    pub scroll_offset: usize,
}

impl FollowStreamOverlay {
    pub fn new(conversation_idx: usize) -> Self {
        Self {
            conversation_idx,
            scroll_offset: 0,
        }
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self, max: usize) {
        if self.scroll_offset + 1 < max {
            self.scroll_offset += 1;
        }
    }

    pub fn page_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(10);
    }

    pub fn page_down(&mut self, max: usize) {
        self.scroll_offset = (self.scroll_offset + 10).min(max.saturating_sub(1));
    }

    pub fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        conv: &Conversation,
        events: &[DebugEvent],
        theme: &ThemeConfig,
    ) {
        // Calculate overlay dimensions (80% of screen, minimum size)
        let width = (area.width * 80 / 100).max(60).min(area.width.saturating_sub(4));
        let height = (area.height * 80 / 100).max(20).min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let overlay_area = Rect::new(x, y, width, height);

        // Clear the area
        Clear.render(overlay_area, buf);

        // Render block with title
        let title = format!(
            " Follow Stream: {} ",
            conv.metadata
                .get("grpc.method")
                .or_else(|| conv.metadata.get("zmq.topic"))
                .or_else(|| conv.metadata.get("dds.topic_name"))
                .cloned()
                .unwrap_or_else(|| conv.summary.clone())
        );

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(theme.focused_border());

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        if inner.height < 3 {
            return;
        }

        // Get events for this conversation
        let conv_events: Vec<&DebugEvent> = conv
            .event_ids
            .iter()
            .filter_map(|&event_id| events.iter().find(|e| e.id == event_id))
            .collect();

        // Render event flow
        self.render_flow(&conv_events, inner, buf, theme);

        // Render footer with metrics
        self.render_footer(conv, inner, buf, theme);

        // Render scrollbar if needed
        let content_lines = self.count_content_lines(&conv_events);
        let visible_lines = inner.height.saturating_sub(3) as usize; // Reserve space for footer
        if content_lines > visible_lines {
            let mut scrollbar_state = ScrollbarState::new(content_lines)
                .position(self.scroll_offset);

            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
                .render(
                    Rect {
                        x: inner.x + inner.width - 1,
                        y: inner.y,
                        width: 1,
                        height: inner.height.saturating_sub(3),
                    },
                    buf,
                    &mut scrollbar_state,
                );
        }
    }

    fn render_flow(
        &self,
        events: &[&DebugEvent],
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
    ) {
        let mut y = area.y;
        let visible_height = area.height.saturating_sub(3); // Reserve for footer
        let mut line_idx = 0;

        for (idx, event) in events.iter().enumerate() {
            // Skip lines before scroll offset
            let event_lines = 3; // Each event takes ~3 lines
            if line_idx + event_lines <= self.scroll_offset {
                line_idx += event_lines;
                continue;
            }

            if y >= area.y + visible_height {
                break;
            }

            // Render event
            let lines_rendered = self.render_event(event, idx, area.x, y, area.width, buf, theme);
            y += lines_rendered;
            line_idx += lines_rendered as usize;
        }
    }

    fn render_event(
        &self,
        event: &DebugEvent,
        idx: usize,
        x: u16,
        y: u16,
        width: u16,
        buf: &mut Buffer,
        theme: &ThemeConfig,
    ) -> u16 {
        let mut current_y = y;

        // Determine direction and style
        let (arrow, style) = match event.direction {
            Direction::Outbound => (
                "→",
                Style::default().fg(Color::Green),
            ),
            Direction::Inbound => (
                "←",
                Style::default().fg(Color::Blue),
            ),
            Direction::Unknown => (
                "?",
                Style::default().fg(Color::Gray),
            ),
        };

        // Line 1: Direction arrow and addresses
        let src = event
            .source
            .network
            .as_ref()
            .map(|n| n.src.as_str())
            .unwrap_or(&event.source.origin);
        let dst = event
            .source
            .network
            .as_ref()
            .map(|n| n.dst.as_str())
            .unwrap_or("-");

        // Calculate time offset from start
        let time_offset = if idx > 0 {
            format!("(+{}ms)", event.timestamp.as_nanos() / 1_000_000)
        } else {
            String::new()
        };

        let line1 = format!("  {} {} → {}  {}", arrow, src, dst, time_offset);
        buf.set_string(x, current_y, truncate(&line1, width as usize), style);
        current_y += 1;

        // Line 2: Event type/method
        let event_type = match event.direction {
            Direction::Outbound => {
                if let Some(method) = event.metadata.get("grpc.method") {
                    format!("    gRPC Request: {}", method)
                } else if let Some(topic) = event.metadata.get("zmq.topic") {
                    format!("    ZMQ Publish: topic={}", topic)
                } else if let Some(topic) = event.metadata.get("dds.topic_name") {
                    format!("    DDS Write: Topic={}", topic)
                } else {
                    format!("    {} Request", event.transport)
                }
            }
            Direction::Inbound => {
                let status = event
                    .metadata
                    .get("grpc.status")
                    .and_then(|s| {
                        match s.as_str() {
                            "0" => Some("OK"),
                            _ => Some("ERROR"),
                        }
                    })
                    .unwrap_or("Response");
                format!("    {} Response ({})", event.transport, status)
            }
            Direction::Unknown => {
                format!("    {} Event (direction unknown)", event.transport)
            }
        };
        buf.set_string(x, current_y, truncate(&event_type, width as usize), Style::default());
        current_y += 1;

        // Line 3: Payload preview (first 60 chars)
        let payload_preview = self.format_payload_preview(event);
        if !payload_preview.is_empty() {
            let preview_line = format!("    {}", payload_preview);
            buf.set_string(
                x,
                current_y,
                truncate(&preview_line, width as usize),
                Style::default().fg(Color::Gray),
            );
            current_y += 1;
        }

        // Blank line between events
        current_y += 1;

        current_y - y
    }

    fn format_payload_preview(&self, event: &DebugEvent) -> String {
        // Try to extract a meaningful preview from the payload
        match &event.payload {
            prb_core::Payload::Decoded { decoded, .. } => {
                // Try to format decoded data compactly
                if let Some(obj) = decoded.as_object() {
                    let preview: Vec<String> = obj
                        .iter()
                        .take(3)
                        .map(|(k, v)| format!("{}: {}", k, format_json_value(v)))
                        .collect();
                    format!("{{ {} }}", preview.join(", "))
                } else {
                    format_json_value(decoded)
                }
            }
            prb_core::Payload::Raw { raw } => {
                // Show first 40 bytes as hex
                if raw.is_empty() {
                    String::new()
                } else {
                    let preview_len = raw.len().min(20);
                    let hex: Vec<String> = raw[..preview_len]
                        .iter()
                        .map(|b| format!("{:02x}", b))
                        .collect();
                    format!("[{}{}]", hex.join(" "), if raw.len() > 20 { " ..." } else { "" })
                }
            }
        }
    }

    fn render_footer(
        &self,
        conv: &Conversation,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
    ) {
        let footer_y = area.y + area.height - 2;

        // Separator line
        buf.set_string(
            area.x,
            footer_y,
            "─".repeat(area.width as usize),
            theme.unfocused_border(),
        );

        // Footer content
        let duration_ms = conv.metrics.duration_ns / 1_000_000;
        let event_count = conv.event_ids.len();
        let status = match conv.state {
            prb_core::conversation::ConversationState::Complete => "OK",
            prb_core::conversation::ConversationState::Error => "ERROR",
            _ => &format!("{}", conv.state),
        };

        let footer = format!(
            "  Duration: {}ms  │  {} events  │  Status: {}",
            duration_ms, event_count, status
        );

        buf.set_string(area.x, footer_y + 1, footer, Style::default());
    }

    fn count_content_lines(&self, events: &[&DebugEvent]) -> usize {
        // Each event takes ~4 lines (address, type, payload, blank)
        events.len() * 4
    }
}

/// Truncate string to width, adding ellipsis if needed.
fn truncate(s: &str, width: usize) -> String {
    if s.len() <= width {
        s.to_string()
    } else if width > 3 {
        format!("{}...", &s[..width.saturating_sub(3)])
    } else {
        s.chars().take(width).collect()
    }
}

/// Format JSON value compactly.
fn format_json_value(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("\"{}\"", truncate(s, 30)),
        serde_json::Value::Array(arr) => format!("[{} items]", arr.len()),
        serde_json::Value::Object(obj) => format!("{{{} fields}}", obj.len()),
    }
}
