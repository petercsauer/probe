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

        let total_lines = (payload_bytes.len() + 15) / 16;
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
