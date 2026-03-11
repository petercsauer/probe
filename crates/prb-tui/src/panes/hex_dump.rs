use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Widget};

use prb_core::Payload;

use crate::app::AppState;
use crate::panes::{Action, PaneComponent};
use crate::theme::ThemeConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteGrouping {
    One,
    Two,
    Four,
}

impl ByteGrouping {
    pub fn cycle(&self) -> Self {
        match self {
            ByteGrouping::One => ByteGrouping::Two,
            ByteGrouping::Two => ByteGrouping::Four,
            ByteGrouping::Four => ByteGrouping::One,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InputMode {
    None,
    Search,
    JumpToOffset,
}

pub struct HexDumpPane {
    pub scroll_offset: usize,
    pub highlight: Option<(usize, usize)>,
    pub byte_grouping: ByteGrouping,
    pub cursor_offset: usize,

    // Search state
    input_mode: InputMode,
    input_buffer: String,
    search_matches: Vec<usize>,
    current_match_index: Option<usize>,
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
            byte_grouping: ByteGrouping::One,
            cursor_offset: 0,
            input_mode: InputMode::None,
            input_buffer: String::new(),
            search_matches: Vec::new(),
            current_match_index: None,
        }
    }

    pub fn set_highlight(&mut self, offset: usize, len: usize) {
        self.highlight = Some((offset, len));
        self.scroll_offset = offset / 16;
        self.cursor_offset = offset;
    }

    pub fn clear_highlight(&mut self) {
        self.highlight = None;
    }

    fn perform_search(&mut self, payload: &[u8]) {
        self.search_matches.clear();
        self.current_match_index = None;

        if self.input_buffer.is_empty() {
            return;
        }

        let query = self.input_buffer.trim();

        // Try hex search first (e.g., "DE AD BE EF" or "DEADBEEF")
        if let Some(pattern) = parse_hex_pattern(query) {
            for i in 0..=payload.len().saturating_sub(pattern.len()) {
                if payload[i..].starts_with(&pattern) {
                    self.search_matches.push(i);
                }
            }
        }
        // Try ASCII search (e.g., "Hello" or quoted "Hello")
        else {
            let needle = query.trim_matches('"').as_bytes();
            for i in 0..=payload.len().saturating_sub(needle.len()) {
                if payload[i..].starts_with(needle) {
                    self.search_matches.push(i);
                }
            }
        }

        if !self.search_matches.is_empty() {
            self.current_match_index = Some(0);
            self.jump_to_match(0);
        }
    }

    fn jump_to_match(&mut self, index: usize) {
        if let Some(&offset) = self.search_matches.get(index) {
            self.cursor_offset = offset;
            self.scroll_offset = offset / 16;
            self.current_match_index = Some(index);
        }
    }

    fn next_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        let next_idx = self.current_match_index
            .map(|idx| (idx + 1) % self.search_matches.len())
            .unwrap_or(0);
        self.jump_to_match(next_idx);
    }

    fn prev_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        let prev_idx = self.current_match_index
            .map(|idx| {
                if idx == 0 {
                    self.search_matches.len() - 1
                } else {
                    idx - 1
                }
            })
            .unwrap_or(0);
        self.jump_to_match(prev_idx);
    }

    fn jump_to_offset(&mut self, offset: usize) {
        self.cursor_offset = offset;
        self.scroll_offset = offset / 16;
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }
}

impl PaneComponent for HexDumpPane {
    fn handle_key(&mut self, key: KeyEvent, state: &AppState) -> Action {
        // Handle input modes first
        match self.input_mode {
            InputMode::Search | InputMode::JumpToOffset => {
                match key.code {
                    KeyCode::Enter => {
                        if self.input_mode == InputMode::Search {
                            // Perform search
                            if let Some(sel_idx) = state.selected_event
                                && let Some(event_idx) = state.filtered_indices.get(sel_idx)
                                && let Some(event) = state.store.get(*event_idx)
                            {
                                let payload_bytes = match &event.payload {
                                    Payload::Raw { raw } => raw.as_ref(),
                                    Payload::Decoded { raw, .. } => raw.as_ref(),
                                };
                                self.perform_search(payload_bytes);
                            }
                        } else {
                            // Jump to offset
                            if let Ok(offset) = parse_hex_offset(&self.input_buffer) {
                                self.jump_to_offset(offset);
                            }
                        }
                        self.input_mode = InputMode::None;
                        self.input_buffer.clear();
                        Action::None
                    }
                    KeyCode::Esc => {
                        self.input_mode = InputMode::None;
                        self.input_buffer.clear();
                        Action::None
                    }
                    KeyCode::Char(c) => {
                        self.input_buffer.push(c);
                        Action::None
                    }
                    KeyCode::Backspace => {
                        self.input_buffer.pop();
                        Action::None
                    }
                    _ => Action::None,
                }
            }
            InputMode::None => {
                // Normal mode keybindings
                match key.code {
                    KeyCode::Char('/') => {
                        self.input_mode = InputMode::Search;
                        self.input_buffer.clear();
                        Action::None
                    }
                    KeyCode::Char('n') => {
                        self.next_match();
                        Action::None
                    }
                    KeyCode::Char('N') => {
                        self.prev_match();
                        Action::None
                    }
                    KeyCode::Char('b') => {
                        self.byte_grouping = self.byte_grouping.cycle();
                        Action::None
                    }
                    KeyCode::Char('g') if key.modifiers.is_empty() => {
                        self.input_mode = InputMode::JumpToOffset;
                        self.input_buffer.clear();
                        Action::None
                    }
                    KeyCode::Char('G') => {
                        // Jump to end
                        self.scroll_offset = usize::MAX; // Will be clamped in render
                        Action::None
                    }
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
                    KeyCode::Home => {
                        self.scroll_offset = 0;
                        Action::None
                    }
                    _ => Action::None,
                }
            }
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, state: &AppState, theme: &ThemeConfig, focused: bool) {
        use ratatui::widgets::BorderType;

        // Build title with search status
        let title = if focused {
            if !self.search_matches.is_empty() {
                format!(" Hex Dump [*] ({}/{} matches) ",
                    self.current_match_index.map(|i| i + 1).unwrap_or(0),
                    self.search_matches.len())
            } else {
                " Hex Dump [*] ".to_string()
            }
        } else {
            " Hex Dump ".to_string()
        };

        let block = if focused {
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.focused_border())
                .title(title)
                .title_style(theme.focused_title())
        } else {
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(theme.unfocused_border())
                .title(title)
                .title_style(theme.unfocused_title())
        };

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 1 || inner.width < 20 {
            return;
        }

        let Some(sel_idx) = state.selected_event else {
            let msg = Text::styled(
                "  Select an event to view raw bytes",
                Style::default().fg(Color::DarkGray),
            );
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

        // Reserve bottom lines for input prompt and value inspector
        let mut content_height = inner.height as usize;
        if self.input_mode != InputMode::None {
            content_height = content_height.saturating_sub(1);
        }
        if focused {
            content_height = content_height.saturating_sub(1); // Value inspector
        }

        let total_lines = payload_bytes.len().div_ceil(16);
        let scroll = self.scroll_offset.min(total_lines.saturating_sub(1));
        let vis_lines = content_height;

        // Render hex lines
        for i in 0..vis_lines {
            let line_idx = scroll + i;
            if line_idx >= total_lines {
                break;
            }

            let byte_offset = line_idx * 16;
            let line_bytes = &payload_bytes
                [byte_offset..payload_bytes.len().min(byte_offset + 16)];

            let hex_line = render_hex_line(
                byte_offset,
                line_bytes,
                self.highlight,
                self.byte_grouping,
                &self.search_matches,
                theme,
            );
            let y = inner.y + i as u16;
            buf.set_line(inner.x, y, &hex_line, inner.width);
        }

        // Render value inspector (if focused)
        if focused && content_height < inner.height as usize {
            let inspector_y = inner.y + content_height as u16;
            let inspector_line = render_value_inspector(self.cursor_offset, payload_bytes, theme);
            buf.set_line(inner.x, inspector_y, &inspector_line, inner.width);
        }

        // Render input prompt (if in input mode)
        if self.input_mode != InputMode::None {
            let prompt_y = inner.y + inner.height.saturating_sub(1);
            let prompt = match self.input_mode {
                InputMode::Search => format!("Search: {}", self.input_buffer),
                InputMode::JumpToOffset => format!("Jump to: {}", self.input_buffer),
                InputMode::None => String::new(),
            };
            let prompt_line = Line::from(vec![
                Span::styled(prompt, theme.filter_bar()),
            ]);
            buf.set_line(inner.x, prompt_y, &prompt_line, inner.width);
        }
    }
}

fn render_hex_line(
    offset: usize,
    bytes: &[u8],
    highlight: Option<(usize, usize)>,
    byte_grouping: ByteGrouping,
    search_matches: &[usize],
    theme: &ThemeConfig,
) -> Line<'static> {
    let mut spans = Vec::new();

    // Offset column
    spans.push(Span::styled(
        format!("{:08x}  ", offset),
        theme.hex_offset(),
    ));

    // Hex bytes with grouping
    match byte_grouping {
        ByteGrouping::One => {
            for i in 0..16 {
                if i == 8 {
                    spans.push(Span::raw(" "));
                }
                if i < bytes.len() {
                    let byte_pos = offset + i;
                    let is_highlighted = highlight
                        .is_some_and(|(start, len)| byte_pos >= start && byte_pos < start + len);
                    let is_search_match = search_matches.contains(&byte_pos);
                    let style = if is_search_match {
                        theme.hex_search_match()
                    } else if is_highlighted {
                        theme.hex_highlight()
                    } else {
                        theme.hex_byte()
                    };
                    spans.push(Span::styled(format!("{:02x} ", bytes[i]), style));
                } else {
                    spans.push(Span::raw("   "));
                }
            }
        }
        ByteGrouping::Two => {
            for i in (0..16).step_by(2) {
                if i == 8 {
                    spans.push(Span::raw(" "));
                }
                if i < bytes.len() {
                    let byte_pos = offset + i;
                    let is_highlighted = highlight
                        .is_some_and(|(start, len)| byte_pos >= start && byte_pos < start + len);
                    let is_search_match = search_matches.contains(&byte_pos);
                    let style = if is_search_match {
                        theme.hex_search_match()
                    } else if is_highlighted {
                        theme.hex_highlight()
                    } else {
                        theme.hex_byte()
                    };
                    if i + 1 < bytes.len() {
                        spans.push(Span::styled(
                            format!("{:02x}{:02x} ", bytes[i], bytes[i + 1]),
                            style,
                        ));
                    } else {
                        spans.push(Span::styled(format!("{:02x}   ", bytes[i]), style));
                    }
                } else {
                    spans.push(Span::raw("     "));
                }
            }
        }
        ByteGrouping::Four => {
            for i in (0..16).step_by(4) {
                if i == 8 {
                    spans.push(Span::raw(" "));
                }
                if i < bytes.len() {
                    let byte_pos = offset + i;
                    let is_highlighted = highlight
                        .is_some_and(|(start, len)| byte_pos >= start && byte_pos < start + len);
                    let is_search_match = search_matches.contains(&byte_pos);
                    let style = if is_search_match {
                        theme.hex_search_match()
                    } else if is_highlighted {
                        theme.hex_highlight()
                    } else {
                        theme.hex_byte()
                    };
                    let mut hex = String::new();
                    for j in 0..4 {
                        if i + j < bytes.len() {
                            hex.push_str(&format!("{:02x}", bytes[i + j]));
                        }
                    }
                    spans.push(Span::styled(format!("{:8} ", hex), style));
                } else {
                    spans.push(Span::raw("         "));
                }
            }
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
            let is_search_match = search_matches.contains(&byte_pos);

            let (c, base_style) = if ch.is_ascii_graphic() || ch == b' ' {
                (ch as char, theme.hex_ascii())
            } else {
                ('.', theme.hex_nonprint())
            };
            let style = if is_search_match {
                theme.hex_search_match()
            } else if is_highlighted {
                theme.hex_highlight()
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

fn render_value_inspector(offset: usize, payload: &[u8], theme: &ThemeConfig) -> Line<'static> {
    if offset >= payload.len() {
        return Line::from(vec![Span::styled(
            format!("Offset: 0x{:04x} (beyond end)", offset),
            theme.hex_offset(),
        )]);
    }

    let mut parts = vec![
        Span::styled(format!("Offset: 0x{:04x} | ", offset), theme.hex_offset()),
    ];

    // u8
    let u8_val = payload[offset];
    parts.push(Span::styled(
        format!("u8: {} | ", u8_val),
        theme.hex_byte(),
    ));

    // u16le
    if offset + 1 < payload.len() {
        let u16le_val = u16::from_le_bytes([payload[offset], payload[offset + 1]]);
        parts.push(Span::styled(
            format!("u16le: {} | ", u16le_val),
            theme.hex_byte(),
        ));
    }

    // u16be
    if offset + 1 < payload.len() {
        let u16be_val = u16::from_be_bytes([payload[offset], payload[offset + 1]]);
        parts.push(Span::styled(
            format!("u16be: {} | ", u16be_val),
            theme.hex_byte(),
        ));
    }

    // ASCII
    let ascii_char = if u8_val.is_ascii_graphic() || u8_val == b' ' {
        u8_val as char
    } else {
        '.'
    };
    parts.push(Span::styled(
        format!("ASCII: '{}'", ascii_char),
        theme.hex_ascii(),
    ));

    Line::from(parts)
}

fn parse_hex_pattern(query: &str) -> Option<Vec<u8>> {
    let cleaned = query.replace(' ', "");
    if cleaned.is_empty() {
        return None;
    }

    let mut bytes = Vec::new();
    let mut chars = cleaned.chars().peekable();

    while chars.peek().is_some() {
        let hex_str: String = chars.by_ref().take(2).collect();
        if hex_str.len() != 2 {
            return None;
        }
        let byte = u8::from_str_radix(&hex_str, 16).ok()?;
        bytes.push(byte);
    }

    if bytes.is_empty() {
        None
    } else {
        Some(bytes)
    }
}

fn parse_hex_offset(input: &str) -> Result<usize, std::num::ParseIntError> {
    let cleaned = input.trim().trim_start_matches("0x").trim_start_matches("0X");
    usize::from_str_radix(cleaned, 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_line_formatting() {
        let bytes = b"Hello, World!";
        let line = render_hex_line(0, bytes, None, ByteGrouping::One, &[], &ThemeConfig::dark());

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
        let theme = ThemeConfig::dark();
        let line = render_hex_line(0, bytes, Some((2, 3)), ByteGrouping::One, &[], &theme);

        // Verify highlighting spans exist
        let highlighted_count = line.spans.iter()
            .filter(|s| s.style == theme.hex_highlight())
            .count();

        // Should have highlighted hex bytes (3) + ASCII chars (3) = 6 total
        assert!(highlighted_count >= 6, "Expected at least 6 highlighted spans");
    }

    #[test]
    fn test_hex_line_partial_row() {
        let bytes = b"ABC"; // Less than 16 bytes
        let theme = ThemeConfig::dark();
        let line = render_hex_line(0, bytes, None, ByteGrouping::One, &[], &theme);

        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

        // Should still have proper offset
        assert!(text.starts_with("00000000"));

        // Should have ASCII representation
        assert!(text.contains("ABC"));
    }

    #[test]
    fn test_hex_line_non_printable() {
        let bytes = b"\x00\x01\x02\x03";
        let line = render_hex_line(0, bytes, None, ByteGrouping::One, &[], &ThemeConfig::dark());

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
            schema_registry: None,
            conversations: None,
            visible_columns: vec![],
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
        let theme = ThemeConfig::dark();
        let line1 = render_hex_line(0, &bytes[0..16], Some((8, 8)), ByteGrouping::One, &[], &theme);
        let line2 = render_hex_line(16, &bytes[16..32], Some((8, 8)), ByteGrouping::One, &[], &theme);

        // Line 1 should have some highlighted spans (bytes 8-15)
        let hl1 = line1.spans.iter().filter(|s| s.style == theme.hex_highlight()).count();
        assert!(hl1 > 0, "Line 1 should have highlighted spans");

        // Line 2 should have no highlighted spans (highlight ends at byte 16)
        let hl2 = line2.spans.iter().filter(|s| s.style == theme.hex_highlight()).count();
        assert_eq!(hl2, 0, "Line 2 should have no highlighted spans");
    }
}
