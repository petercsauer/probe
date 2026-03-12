//! Conversation list pane for viewing grouped conversations.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{
    Block, Borders, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::AppState;
use crate::panes::{Action, PaneComponent};
use crate::theme::ThemeConfig;
use prb_core::{DebugEvent, conversation::Conversation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvSortColumn {
    Id,
    Protocol,
    Source,
    Dest,
    Requests,
    Duration,
    Error,
}

impl ConvSortColumn {
    pub fn next(self) -> Self {
        match self {
            ConvSortColumn::Id => ConvSortColumn::Protocol,
            ConvSortColumn::Protocol => ConvSortColumn::Source,
            ConvSortColumn::Source => ConvSortColumn::Dest,
            ConvSortColumn::Dest => ConvSortColumn::Requests,
            ConvSortColumn::Requests => ConvSortColumn::Duration,
            ConvSortColumn::Duration => ConvSortColumn::Error,
            ConvSortColumn::Error => ConvSortColumn::Id,
        }
    }
}

pub struct ConversationListPane {
    pub selected: usize,
    pub scroll_offset: usize,
    pub sort_column: ConvSortColumn,
    pub sort_reversed: bool,
}

#[derive(Debug, Clone, Copy)]
struct ColumnWidths {
    id: u16,
    protocol: u16,
    src: u16,
    dst: u16,
    reqs: u16,
    duration: u16,
    status: u16,
    method: u16,
}

impl Default for ConversationListPane {
    fn default() -> Self {
        Self::new()
    }
}

impl ConversationListPane {
    pub fn new() -> Self {
        ConversationListPane {
            selected: 0,
            scroll_offset: 0,
            sort_column: ConvSortColumn::Id,
            sort_reversed: false,
        }
    }

    /// Sort conversations according to current sort column and direction.
    fn sort_conversations<'a>(
        &self,
        conversations: &'a [Conversation],
        events: &[DebugEvent],
    ) -> Vec<(usize, &'a Conversation)> {
        let mut indexed: Vec<(usize, &'a Conversation)> =
            conversations.iter().enumerate().collect();

        indexed.sort_by(|(_, a), (_, b)| {
            let cmp = match self.sort_column {
                ConvSortColumn::Id => a.id.0.cmp(&b.id.0),
                ConvSortColumn::Protocol => a.protocol.cmp(&b.protocol),
                ConvSortColumn::Source => {
                    let src_a = self.get_first_event_source(a, events);
                    let src_b = self.get_first_event_source(b, events);
                    src_a.cmp(&src_b)
                }
                ConvSortColumn::Dest => {
                    let dst_a = self.get_first_event_dest(a, events);
                    let dst_b = self.get_first_event_dest(b, events);
                    dst_a.cmp(&dst_b)
                }
                ConvSortColumn::Requests => a.metrics.request_count.cmp(&b.metrics.request_count),
                ConvSortColumn::Duration => a.metrics.duration_ns.cmp(&b.metrics.duration_ns),
                ConvSortColumn::Error => {
                    let status_a = Self::get_status_str(a);
                    let status_b = Self::get_status_str(b);
                    status_a.cmp(status_b)
                }
            };

            if self.sort_reversed {
                cmp.reverse()
            } else {
                cmp
            }
        });

        indexed
    }

    fn get_first_event_source(&self, conv: &Conversation, events: &[DebugEvent]) -> String {
        conv.event_ids
            .first()
            .and_then(|&event_id| events.iter().find(|e| e.id == event_id))
            .map(|e| {
                e.source
                    .network
                    .as_ref()
                    .map(|n| n.src.clone())
                    .unwrap_or_else(|| e.source.origin.clone())
            })
            .unwrap_or_else(|| "-".to_string())
    }

    fn get_first_event_dest(&self, conv: &Conversation, events: &[DebugEvent]) -> String {
        conv.event_ids
            .first()
            .and_then(|&event_id| events.iter().find(|e| e.id == event_id))
            .and_then(|e| e.source.network.as_ref())
            .map(|n| n.dst.clone())
            .unwrap_or_else(|| "-".to_string())
    }

    fn get_status_str(conv: &Conversation) -> &str {
        match conv.state {
            prb_core::conversation::ConversationState::Complete => "[OK] OK",
            prb_core::conversation::ConversationState::Error => "[X] ERROR",
            prb_core::conversation::ConversationState::Timeout => "⏱ TIMEOUT",
            prb_core::conversation::ConversationState::Active => "→ ACTIVE",
            prb_core::conversation::ConversationState::Incomplete => "[!] INCOMPL",
        }
    }

    fn get_method_str(conv: &Conversation) -> String {
        conv.metadata
            .get("grpc.method")
            .or_else(|| conv.metadata.get("zmq.topic"))
            .or_else(|| conv.metadata.get("dds.topic_name"))
            .cloned()
            .unwrap_or_else(|| "-".to_string())
    }

    fn format_duration(ns: u64) -> String {
        let ms = ns / 1_000_000;
        if ms < 1000 {
            format!("{}ms", ms)
        } else {
            format!("{:.1}s", ms as f64 / 1000.0)
        }
    }

    fn compute_column_widths(total_width: u16) -> ColumnWidths {
        // Fixed widths: #(6) Protocol(10) Src(18) Dst(18) Reqs(5) Duration(10) Status(11)
        let fixed = 6 + 10 + 18 + 18 + 5 + 10 + 11;
        let method = (total_width as usize).saturating_sub(fixed) as u16;
        ColumnWidths {
            id: 6,
            protocol: 10,
            src: 18,
            dst: 18,
            reqs: 5,
            duration: 10,
            status: 11,
            method,
        }
    }

    fn render_header(
        &self,
        area: Rect,
        buf: &mut Buffer,
        widths: &ColumnWidths,
        theme: &ThemeConfig,
    ) {
        let header_style = theme.header();
        let mut x = area.x;

        let columns = [
            ("#", widths.id),
            ("Protocol", widths.protocol),
            ("Source", widths.src),
            ("Dest", widths.dst),
            ("Reqs", widths.reqs),
            ("Duration", widths.duration),
            ("Status", widths.status),
            ("Method", widths.method),
        ];

        for (col_name, col_width) in &columns {
            if *col_width > 0 {
                buf.set_string(
                    x,
                    area.y,
                    truncate(col_name, *col_width as usize),
                    header_style,
                );
                x += col_width;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_row(
        &self,
        conv_idx: usize,
        conv: &Conversation,
        events: &[DebugEvent],
        y: u16,
        area: Rect,
        buf: &mut Buffer,
        widths: &ColumnWidths,
        style: Style,
    ) {
        let mut x = area.x;

        // # column
        let id_str = format!("{}", conv_idx);
        buf.set_string(x, y, truncate(&id_str, widths.id as usize), style);
        x += widths.id;

        // Protocol column
        let proto_str = format!("{}", conv.protocol);
        buf.set_string(x, y, truncate(&proto_str, widths.protocol as usize), style);
        x += widths.protocol;

        // Source column
        let src_str = self.get_first_event_source(conv, events);
        buf.set_string(x, y, truncate(&src_str, widths.src as usize), style);
        x += widths.src;

        // Dest column
        let dst_str = self.get_first_event_dest(conv, events);
        buf.set_string(x, y, truncate(&dst_str, widths.dst as usize), style);
        x += widths.dst;

        // Requests column
        let reqs_str = format!("{}", conv.metrics.request_count);
        buf.set_string(x, y, truncate(&reqs_str, widths.reqs as usize), style);
        x += widths.reqs;

        // Duration column
        let duration_str = Self::format_duration(conv.metrics.duration_ns);
        buf.set_string(
            x,
            y,
            truncate(&duration_str, widths.duration as usize),
            style,
        );
        x += widths.duration;

        // Status column
        let status_str = Self::get_status_str(conv);
        buf.set_string(x, y, truncate(status_str, widths.status as usize), style);
        x += widths.status;

        // Method column (fill remaining)
        if widths.method > 0 {
            let method_str = Self::get_method_str(conv);
            buf.set_string(x, y, truncate(&method_str, widths.method as usize), style);
        }
    }
}

impl PaneComponent for ConversationListPane {
    fn handle_key(&mut self, key: KeyEvent, state: &AppState) -> Action {
        let conversations = match &state.conversations {
            Some(conv_set) => &conv_set.conversations,
            None => return Action::None,
        };

        let sorted = self.sort_conversations(conversations, state.store.events());
        let max_idx = sorted.len().saturating_sub(1);

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected < max_idx {
                    self.selected += 1;
                }
                Action::None
            }
            KeyCode::PageUp => {
                self.selected = self.selected.saturating_sub(10);
                Action::None
            }
            KeyCode::PageDown => {
                self.selected = (self.selected + 10).min(max_idx);
                Action::None
            }
            KeyCode::Home => {
                self.selected = 0;
                Action::None
            }
            KeyCode::End => {
                self.selected = max_idx;
                Action::None
            }
            KeyCode::Enter => {
                // Select this conversation (filter events to this conversation)
                if self.selected < sorted.len() {
                    let (_, conv) = sorted[self.selected];
                    if let Some(&first_event_id) = conv.event_ids.first() {
                        // Find the event index in the filtered indices
                        if let Some(pos) = state.filtered_indices.iter().position(|&idx| {
                            state
                                .store
                                .get(idx)
                                .map(|e| e.id == first_event_id)
                                .unwrap_or(false)
                        }) {
                            return Action::SelectEvent(pos);
                        }
                    }
                }
                Action::None
            }
            KeyCode::Char('s') => {
                self.sort_column = self.sort_column.next();
                Action::None
            }
            KeyCode::Char('r') => {
                self.sort_reversed = !self.sort_reversed;
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        state: &AppState,
        theme: &ThemeConfig,
        focused: bool,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Conversations ")
            .border_style(if focused {
                theme.focused_border()
            } else {
                theme.unfocused_border()
            });

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 2 {
            return;
        }

        let conversations = match &state.conversations {
            Some(conv_set) => &conv_set.conversations,
            None => {
                // No conversations available
                buf.set_string(
                    inner.x,
                    inner.y,
                    "No conversations available",
                    theme.normal_row(),
                );
                return;
            }
        };

        let sorted = self.sort_conversations(conversations, state.store.events());
        let widths = Self::compute_column_widths(inner.width);

        // Render header
        self.render_header(inner, buf, &widths, theme);

        // Adjust scroll to keep selection visible
        let visible_height = inner.height.saturating_sub(1) as usize; // Subtract 1 for header
        if self.selected >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected.saturating_sub(visible_height - 1);
        } else if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }

        // Render rows
        let mut y = inner.y + 1; // Start after header
        for (list_idx, (conv_idx, conv)) in sorted
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(visible_height)
        {
            let style = if list_idx == self.selected {
                theme.selected_row()
            } else {
                theme.normal_row()
            };

            self.render_row(
                *conv_idx,
                conv,
                state.store.events(),
                y,
                inner,
                buf,
                &widths,
                style,
            );
            y += 1;
        }

        // Render scrollbar if needed
        if sorted.len() > visible_height {
            let mut scrollbar_state =
                ScrollbarState::new(sorted.len()).position(self.scroll_offset);

            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
                .render(
                    Rect {
                        x: inner.x + inner.width - 1,
                        y: inner.y + 1, // After header
                        width: 1,
                        height: inner.height.saturating_sub(1),
                    },
                    buf,
                    &mut scrollbar_state,
                );
        }
    }
}

/// Truncate string to width, adding ellipsis if needed.
fn truncate(s: &str, width: usize) -> String {
    if s.width() <= width {
        format!("{:width$}", s, width = width)
    } else if width > 3 {
        let mut result = String::new();
        let mut w = 0;
        for ch in s.chars() {
            let ch_width = ch.width().unwrap_or(0);
            if w + ch_width + 3 > width {
                result.push_str("...");
                break;
            }
            result.push(ch);
            w += ch_width;
        }
        format!("{:width$}", result, width = width)
    } else {
        s.chars().take(width).collect()
    }
}
