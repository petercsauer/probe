use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Widget};

use prb_core::Payload;

use crate::app::AppState;
use crate::panes::{Action, PaneComponent};
use crate::theme::Theme;

pub struct HexDumpPane {
    pub scroll_offset: usize,
    pub highlight: Option<(usize, usize)>,
}

impl Default for HexDumpPane {
    fn default() -> Self {
        Self::new()
    }
}

impl HexDumpPane {
    pub fn new() -> Self {
        HexDumpPane {
            scroll_offset: 0,
            highlight: None,
        }
    }

    pub fn set_highlight(&mut self, offset: usize, len: usize) {
        self.highlight = Some((offset, len));
        self.scroll_offset = offset / 16;
    }

    pub fn clear_highlight(&mut self) {
        self.highlight = None;
    }
}

impl PaneComponent for HexDumpPane {
    fn handle_key(&mut self, key: KeyEvent, _state: &AppState) -> Action {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_offset += 1;
                Action::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                Action::None
            }
            KeyCode::PageDown => {
                self.scroll_offset += 16;
                Action::None
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(16);
                Action::None
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.scroll_offset = 0;
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, state: &AppState, focused: bool) {
        let border_style = if focused {
            Theme::focused_border()
        } else {
            Theme::unfocused_border()
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Hex Dump ");

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 1 || inner.width < 20 {
            return;
        }

        let Some(sel_idx) = state.selected_event else {
            let msg = Text::raw("  No event selected");
            Widget::render(msg, inner, buf);
            return;
        };
        let Some(event_idx) = state.filtered_indices.get(sel_idx) else {
            return;
        };
        let Some(event) = state.store.get(*event_idx) else {
            return;
        };

        let payload_bytes = match &event.payload {
            Payload::Raw { raw } => raw.as_ref(),
            Payload::Decoded { raw, .. } => raw.as_ref(),
        };

        if payload_bytes.is_empty() {
            let msg = Text::raw("  Empty payload");
            Widget::render(msg, inner, buf);
            return;
        }

        let total_lines = payload_bytes.len().div_ceil(16);
        let scroll = self.scroll_offset.min(total_lines.saturating_sub(1));
        let vis_lines = inner.height as usize;

        for i in 0..vis_lines {
            let line_idx = scroll + i;
            if line_idx >= total_lines {
                break;
            }

            let byte_offset = line_idx * 16;
            let line_bytes = &payload_bytes
                [byte_offset..payload_bytes.len().min(byte_offset + 16)];

            let hex_line = render_hex_line(byte_offset, line_bytes, self.highlight);
            let y = inner.y + i as u16;
            buf.set_line(inner.x, y, &hex_line, inner.width);
        }
    }
}

fn render_hex_line(
    offset: usize,
    bytes: &[u8],
    highlight: Option<(usize, usize)>,
) -> Line<'static> {
    let mut spans = Vec::new();

    // Offset column
    spans.push(Span::styled(
        format!("{:08x}  ", offset),
        Theme::hex_offset(),
    ));

    // Hex bytes
    for i in 0..16 {
        if i == 8 {
            spans.push(Span::raw(" "));
        }
        if i < bytes.len() {
            let byte_pos = offset + i;
            let is_highlighted = highlight
                .is_some_and(|(start, len)| byte_pos >= start && byte_pos < start + len);
            let style = if is_highlighted {
                Theme::hex_highlight()
            } else {
                Theme::hex_byte()
            };
            spans.push(Span::styled(format!("{:02x} ", bytes[i]), style));
        } else {
            spans.push(Span::raw("   "));
        }
    }

    spans.push(Span::raw(" "));

    // ASCII column
    for i in 0..16 {
        if i < bytes.len() {
            let ch = bytes[i];
            let byte_pos = offset + i;
            let is_highlighted = highlight
                .is_some_and(|(start, len)| byte_pos >= start && byte_pos < start + len);

            let (c, base_style) = if ch.is_ascii_graphic() || ch == b' ' {
                (ch as char, Theme::hex_ascii())
            } else {
                ('.', Theme::hex_nonprint())
            };
            let style = if is_highlighted {
                Theme::hex_highlight()
            } else {
                base_style
            };
            spans.push(Span::styled(String::from(c), style));
        } else {
            spans.push(Span::raw(" "));
        }
    }

    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_line_formatting() {
        let bytes = b"Hello, World!";
        let line = render_hex_line(0, bytes, None);

        // Should have offset + hex bytes + ASCII
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

        // Check offset is present
        assert!(text.starts_with("00000000"));

        // Check hex bytes are present
        assert!(text.contains("48")); // 'H'
        assert!(text.contains("65")); // 'e'

        // Check ASCII is present
        assert!(text.contains("Hello"));
    }

    #[test]
    fn test_hex_line_with_highlight() {
        let bytes = b"ABCDEFGH";
        // Highlight bytes 2-4 (CDE)
        let line = render_hex_line(0, bytes, Some((2, 3)));

        // Verify highlighting spans exist
        let highlighted_count = line.spans.iter()
            .filter(|s| s.style == Theme::hex_highlight())
            .count();

        // Should have highlighted hex bytes (3) + ASCII chars (3) = 6 total
        assert!(highlighted_count >= 6, "Expected at least 6 highlighted spans");
    }

    #[test]
    fn test_hex_line_partial_row() {
        let bytes = b"ABC"; // Less than 16 bytes
        let line = render_hex_line(0, bytes, None);

        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

        // Should still have proper offset
        assert!(text.starts_with("00000000"));

        // Should have ASCII representation
        assert!(text.contains("ABC"));
    }

    #[test]
    fn test_hex_line_non_printable() {
        let bytes = b"\x00\x01\x02\x03";
        let line = render_hex_line(0, bytes, None);

        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

        // Should have hex values
        assert!(text.contains("00"));
        assert!(text.contains("01"));

        // Non-printable should be shown as dots
        let dots = text.chars().filter(|&c| c == '.').count();
        assert_eq!(dots, 4, "Non-printable bytes should be shown as dots");
    }

    #[test]
    fn test_set_highlight_auto_scroll() {
        let mut pane = HexDumpPane::new();

        // Highlight at byte offset 256 (line 16)
        pane.set_highlight(256, 10);

        assert_eq!(pane.highlight, Some((256, 10)));
        assert_eq!(pane.scroll_offset, 16, "Should auto-scroll to highlighted line");
    }

    #[test]
    fn test_clear_highlight() {
        let mut pane = HexDumpPane::new();

        pane.set_highlight(100, 20);
        assert!(pane.highlight.is_some());

        pane.clear_highlight();
        assert!(pane.highlight.is_none());
    }

    #[test]
    fn test_scroll_bounds() {
        let mut pane = HexDumpPane::new();
        pane.scroll_offset = 10;

        // Simulate up key beyond bounds
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        use crate::app::AppState;
        use crate::event_store::EventStore;

        let store = EventStore::new(vec![]);
        let state = AppState {
            store,
            filtered_indices: vec![],
            selected_event: None,
            filter: None,
            filter_text: String::new(),
        };

        // Press 'k' (up) many times
        for _ in 0..20 {
            pane.handle_key(
                KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
                &state
            );
        }

        // Should stop at 0
        assert_eq!(pane.scroll_offset, 0);
    }

    #[test]
    fn test_highlight_range_calculation() {
        // Test that highlight range correctly identifies bytes
        let bytes = b"0123456789ABCDEF0123456789ABCDEF";

        // Highlight bytes 8-16 (second half of first line + first half of second)
        let line1 = render_hex_line(0, &bytes[0..16], Some((8, 8)));
        let line2 = render_hex_line(16, &bytes[16..32], Some((8, 8)));

        // Line 1 should have some highlighted spans (bytes 8-15)
        let hl1 = line1.spans.iter().filter(|s| s.style == Theme::hex_highlight()).count();
        assert!(hl1 > 0, "Line 1 should have highlighted spans");

        // Line 2 should have no highlighted spans (highlight ends at byte 16)
        let hl2 = line2.spans.iter().filter(|s| s.style == Theme::hex_highlight()).count();
        assert_eq!(hl2, 0, "Line 2 should have no highlighted spans");
    }
}
