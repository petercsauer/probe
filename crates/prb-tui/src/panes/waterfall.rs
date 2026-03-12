//! Waterfall pane showing request/response timing as horizontal bars.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget};

use crate::app::AppState;
use crate::panes::{Action, PaneComponent};
use crate::theme::ThemeConfig;
use prb_core::conversation::{Conversation, ConversationState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Duration,
    StartTime,
    Status,
}

impl SortMode {
    fn next(self) -> Self {
        match self {
            SortMode::Duration => SortMode::StartTime,
            SortMode::StartTime => SortMode::Status,
            SortMode::Status => SortMode::Duration,
        }
    }

    fn label(self) -> &'static str {
        match self {
            SortMode::Duration => "Duration",
            SortMode::StartTime => "Start Time",
            SortMode::Status => "Status",
        }
    }
}

pub struct WaterfallPane {
    pub selected: usize,
    pub scroll_offset: usize,
    pub sort_mode: SortMode,
    pub sort_ascending: bool,
}

impl Default for WaterfallPane {
    fn default() -> Self {
        Self::new()
    }
}

impl WaterfallPane {
    pub fn new() -> Self {
        WaterfallPane {
            selected: 0,
            scroll_offset: 0,
            sort_mode: SortMode::StartTime,
            sort_ascending: true,
        }
    }

    /// Get method/endpoint label for a conversation
    fn get_method_label(conv: &Conversation) -> String {
        conv.metadata
            .get("grpc.method")
            .or_else(|| conv.metadata.get("zmq.topic"))
            .or_else(|| conv.metadata.get("dds.topic_name"))
            .cloned()
            .unwrap_or_else(|| conv.summary.clone())
    }

    /// Format duration in human-readable form
    fn format_duration(ns: u64) -> String {
        let ms = ns / 1_000_000;
        if ms < 1000 {
            format!("{}ms", ms)
        } else {
            format!("{:.1}s", ms as f64 / 1000.0)
        }
    }

    /// Filter conversations to only include those with events in the filtered set
    fn filter_conversations(&self, conversations: &[Conversation], state: &AppState) -> Vec<usize> {
        let filtered_event_ids: std::collections::HashSet<_> = state
            .filtered_indices
            .iter()
            .filter_map(|&idx| state.store.get(idx).map(|e| e.id))
            .collect();

        conversations
            .iter()
            .enumerate()
            .filter_map(|(idx, conv)| {
                // Include conversation if any of its events are in the filtered set
                let has_filtered_event = conv
                    .event_ids
                    .iter()
                    .any(|event_id| filtered_event_ids.contains(event_id));
                if has_filtered_event {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Sort conversations according to current sort mode
    fn sort_conversations(&self, conversations: &[Conversation], filtered_indices: &[usize]) -> Vec<usize> {
        let mut indices = filtered_indices.to_vec();

        indices.sort_by(|&a, &b| {
            let conv_a = &conversations[a];
            let conv_b = &conversations[b];

            let ordering = match self.sort_mode {
                SortMode::Duration => conv_a.metrics.duration_ns.cmp(&conv_b.metrics.duration_ns),
                SortMode::StartTime => {
                    match (conv_a.metrics.start_time, conv_b.metrics.start_time) {
                        (Some(t_a), Some(t_b)) => t_a.cmp(&t_b),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    }
                }
                SortMode::Status => conv_a.state.cmp(&conv_b.state),
            };

            if self.sort_ascending {
                ordering
            } else {
                ordering.reverse()
            }
        });

        indices
    }

    /// Compute time range from conversations at given indices
    fn compute_time_range_filtered(conversations: &[Conversation], indices: &[usize]) -> Option<(u64, u64)> {
        if indices.is_empty() {
            return None;
        }

        let mut min_time = u64::MAX;
        let mut max_time = 0u64;

        for &idx in indices {
            if let Some(conv) = conversations.get(idx) {
                if let Some(start) = conv.metrics.start_time {
                    let start_ns = start.as_nanos();
                    if start_ns < min_time {
                        min_time = start_ns;
                    }
                }
                if let Some(end) = conv.metrics.end_time {
                    let end_ns = end.as_nanos();
                    if end_ns > max_time {
                        max_time = end_ns;
                    }
                }
            }
        }

        if min_time == u64::MAX || max_time == 0 {
            None
        } else {
            Some((min_time, max_time))
        }
    }

    /// Render a horizontal timing bar for a conversation
    fn render_bar(
        &self,
        conv: &Conversation,
        min_time: u64,
        time_range: u64,
        bar_width: u16,
        is_selected: bool,
        _theme: &ThemeConfig,
    ) -> (String, Style) {
        let start_time = conv.metrics.start_time.map(|t| t.as_nanos()).unwrap_or(min_time);
        let end_time = conv.metrics.end_time.map(|t| t.as_nanos()).unwrap_or(start_time);

        // Calculate positions
        let rel_start = start_time.saturating_sub(min_time);
        let rel_end = end_time.saturating_sub(min_time);

        let start_col = if time_range > 0 {
            ((rel_start as f64 / time_range as f64) * bar_width as f64) as u16
        } else {
            0
        };

        let end_col = if time_range > 0 {
            ((rel_end as f64 / time_range as f64) * bar_width as f64) as u16
        } else {
            0
        };

        let bar_len = end_col.saturating_sub(start_col).max(1);

        // Build bar string with Unicode blocks
        let mut bar = String::new();

        // Leading spaces
        for _ in 0..start_col {
            bar.push(' ');
        }

        // Bar content - split between request and response phases
        let request_phase = if let Some(ttfr) = conv.metrics.time_to_first_response_ns {
            let request_frac = ttfr as f64 / conv.metrics.duration_ns.max(1) as f64;
            (bar_len as f64 * request_frac).ceil() as u16
        } else {
            bar_len
        };

        // Solid blocks for request phase
        for _ in 0..request_phase.min(bar_len) {
            bar.push('█');
        }

        // Lighter blocks for response wait
        for _ in request_phase..bar_len {
            bar.push('░');
        }

        // Trailing spaces to fill width
        while bar.len() < bar_width as usize {
            bar.push(' ');
        }

        // Determine style based on conversation state
        let style = if conv.state == ConversationState::Error {
            Style::default().fg(Color::Red).add_modifier(if is_selected {
                Modifier::REVERSED
            } else {
                Modifier::empty()
            })
        } else {
            let proto_color = match conv.protocol {
                prb_core::TransportKind::Grpc => Color::Cyan,
                prb_core::TransportKind::Zmq => Color::Yellow,
                prb_core::TransportKind::DdsRtps => Color::Magenta,
                _ => Color::Gray,
            };
            Style::default().fg(proto_color).add_modifier(if is_selected {
                Modifier::REVERSED
            } else {
                Modifier::empty()
            })
        };

        (bar, style)
    }

    /// Render time axis at the bottom
    fn render_time_axis(
        &self,
        area: Rect,
        buf: &mut Buffer,
        time_range_ns: u64,
        theme: &ThemeConfig,
    ) {
        if area.width < 10 {
            return;
        }

        // Determine scale unit
        let (scale_ms, unit) = if time_range_ns < 1_000_000_000 {
            (time_range_ns / 1_000_000, "ms")
        } else {
            (time_range_ns / 1_000_000_000, "s")
        };

        // Draw axis line
        let axis_style = theme.normal_row();
        for x in area.x..area.x + area.width {
            buf.set_string(x, area.y, "─", axis_style);
        }

        // Draw tick marks at 0%, 50%, and 100%
        buf.set_string(area.x, area.y, "|", axis_style);
        buf.set_string(area.x, area.y + 1, "0", axis_style);

        let mid_x = area.x + area.width / 2;
        buf.set_string(mid_x, area.y, "|", axis_style);
        buf.set_string(
            mid_x.saturating_sub(2),
            area.y + 1,
            format!("{}{}", scale_ms / 2, unit),
            axis_style,
        );

        let end_x = area.x + area.width - 1;
        buf.set_string(end_x, area.y, "|", axis_style);
        let end_label = format!("{}{}", scale_ms, unit);
        buf.set_string(
            end_x.saturating_sub(end_label.len() as u16),
            area.y + 1,
            &end_label,
            axis_style,
        );
    }

    /// Render latency breakdown for selected conversation
    fn render_latency_breakdown(
        &self,
        area: Rect,
        buf: &mut Buffer,
        conv: &Conversation,
        theme: &ThemeConfig,
    ) {
        if area.height < 3 {
            return;
        }

        let style = theme.normal_row();
        let y = area.y;

        // Line 1: Duration and breakdown
        let duration_str = format!(
            "Duration: {}",
            Self::format_duration(conv.metrics.duration_ns)
        );
        buf.set_string(area.x, y, &duration_str, style);

        // Line 2: TTFR and breakdown percentages
        if let Some(ttfr) = conv.metrics.time_to_first_response_ns {
            let ttfr_str = format!("TTFR: {}", Self::format_duration(ttfr));
            let ttfr_pct = if conv.metrics.duration_ns > 0 {
                (ttfr as f64 / conv.metrics.duration_ns as f64 * 100.0) as u32
            } else {
                0
            };
            let response_pct = 100u32.saturating_sub(ttfr_pct);

            let breakdown = format!(
                "{}  |  Request: {}% (█)  Response: {}% (░)",
                ttfr_str, ttfr_pct, response_pct
            );
            buf.set_string(area.x, y + 1, &breakdown, style);
        } else {
            buf.set_string(area.x, y + 1, "TTFR: N/A  |  No timing breakdown available", style);
        }

        // Line 3: Request/Response counts, bytes, and status
        let status_str = match conv.state {
            ConversationState::Complete => "✓ Complete",
            ConversationState::Error => "✗ Error",
            ConversationState::Timeout => "⏱ Timeout",
            ConversationState::Active => "⟳ Active",
            ConversationState::Incomplete => "◐ Incomplete",
        };

        let metrics_str = format!(
            "Req: {}  Resp: {}  Bytes: {}  Status: {}",
            conv.metrics.request_count,
            conv.metrics.response_count,
            conv.metrics.total_bytes,
            status_str
        );
        buf.set_string(area.x, y + 2, &metrics_str, style);
    }
}

impl PaneComponent for WaterfallPane {
    fn handle_key(&mut self, key: KeyEvent, state: &AppState) -> Action {
        let conversations = match &state.conversations {
            Some(conv_set) => &conv_set.conversations,
            None => return Action::None,
        };

        if conversations.is_empty() {
            return Action::None;
        }

        // Get filtered and sorted indices
        let filtered_indices = self.filter_conversations(conversations, state);
        if filtered_indices.is_empty() {
            return Action::None;
        }

        let max_idx = filtered_indices.len().saturating_sub(1);

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
                // Jump to first event of selected conversation
                let sorted_indices = self.sort_conversations(conversations, &filtered_indices);
                if self.selected < sorted_indices.len() {
                    let conv_idx = sorted_indices[self.selected];
                    let conv = &conversations[conv_idx];
                    if let Some(&first_event_id) = conv.event_ids.first() {
                        // Find the event index in the filtered indices
                        if let Some(pos) = state.filtered_indices.iter().position(|&idx| {
                            state.store.get(idx).map(|e| e.id == first_event_id).unwrap_or(false)
                        }) {
                            return Action::SelectEvent(pos);
                        }
                    }
                }
                Action::None
            }
            KeyCode::Char('s') => {
                // Cycle sort mode
                self.sort_mode = self.sort_mode.next();
                self.selected = 0;
                self.scroll_offset = 0;
                Action::None
            }
            KeyCode::Char('r') => {
                // Reverse sort order
                self.sort_ascending = !self.sort_ascending;
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
        let sort_indicator = if self.sort_ascending { "↑" } else { "↓" };
        let title = format!(" Waterfall [Sort: {} {}] ", self.sort_mode.label(), sort_indicator);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(if focused {
                theme.focused_border()
            } else {
                theme.unfocused_border()
            });

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 5 {
            return;
        }

        let conversations = match &state.conversations {
            Some(conv_set) => &conv_set.conversations,
            None => {
                buf.set_string(
                    inner.x,
                    inner.y,
                    "No conversations available",
                    theme.normal_row(),
                );
                return;
            }
        };

        if conversations.is_empty() {
            buf.set_string(
                inner.x,
                inner.y,
                "No conversations to display",
                theme.normal_row(),
            );
            return;
        }

        // Filter conversations based on event filter
        let filtered_indices = self.filter_conversations(conversations, state);
        if filtered_indices.is_empty() {
            buf.set_string(
                inner.x,
                inner.y,
                "No conversations match current filter",
                theme.normal_row(),
            );
            return;
        }

        // Compute time range from filtered conversations
        let Some((min_time, max_time)) = Self::compute_time_range_filtered(conversations, &filtered_indices) else {
            buf.set_string(
                inner.x,
                inner.y,
                "No timing data available",
                theme.normal_row(),
            );
            return;
        };

        let time_range = max_time.saturating_sub(min_time);

        // Reserve space for time axis (2 lines) and latency breakdown (3 lines)
        let axis_height = 2;
        let breakdown_height = 3;
        let reserved_height = axis_height + breakdown_height + 1; // +1 for separator

        if inner.height <= reserved_height {
            return;
        }

        let visible_height = (inner.height - reserved_height) as usize;

        // Calculate bar width (leave room for label)
        let label_width = 25u16;
        let duration_width = 8u16;
        let error_label_width = 5u16;
        let bar_width = inner.width.saturating_sub(label_width + duration_width + error_label_width + 3);

        // Get sorted filtered indices
        let sorted_indices = self.sort_conversations(conversations, &filtered_indices);

        // Clamp selection to valid range
        if self.selected >= sorted_indices.len() {
            self.selected = sorted_indices.len().saturating_sub(1);
        }

        // Adjust scroll to keep selection visible
        if self.selected >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected.saturating_sub(visible_height - 1);
        } else if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }

        // Render conversations
        let mut y = inner.y;
        for (display_idx, &conv_idx) in sorted_indices
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(visible_height)
        {
            let is_selected = display_idx == self.selected;
            let conv = &conversations[conv_idx];

            // Render method label (left-aligned, truncated)
            let method = Self::get_method_label(conv);
            let method_display = if method.len() > label_width as usize {
                format!("{}...", &method[..label_width as usize - 3])
            } else {
                format!("{:<width$}", method, width = label_width as usize)
            };

            let label_style = if is_selected {
                theme.selected_row()
            } else {
                theme.normal_row()
            };
            buf.set_string(inner.x, y, &method_display, label_style);

            // Render timing bar
            let (bar, bar_style) = self.render_bar(
                conv,
                min_time,
                time_range,
                bar_width,
                is_selected,
                theme,
            );
            buf.set_string(inner.x + label_width + 1, y, &bar, bar_style);

            // Render duration (right side)
            let duration_str = Self::format_duration(conv.metrics.duration_ns);
            buf.set_string(
                inner.x + label_width + bar_width + 2,
                y,
                format!("{:>width$}", duration_str, width = duration_width as usize),
                label_style,
            );

            // Render error indicator if needed
            if conv.state == ConversationState::Error {
                buf.set_string(
                    inner.x + label_width + bar_width + duration_width + 3,
                    y,
                    " ERR",
                    Style::default().fg(Color::Red),
                );
            }

            y += 1;
        }

        // Render time axis
        let axis_y = inner.y + visible_height as u16;
        let axis_area = Rect {
            x: inner.x + label_width + 1,
            y: axis_y,
            width: bar_width,
            height: axis_height,
        };
        self.render_time_axis(axis_area, buf, time_range, theme);

        // Render latency breakdown for selected conversation
        if self.selected < sorted_indices.len() {
            let breakdown_y = axis_y + axis_height;
            let breakdown_area = Rect {
                x: inner.x,
                y: breakdown_y,
                width: inner.width,
                height: breakdown_height,
            };
            let selected_conv_idx = sorted_indices[self.selected];
            self.render_latency_breakdown(
                breakdown_area,
                buf,
                &conversations[selected_conv_idx],
                theme,
            );
        }

        // Render scrollbar if needed
        if sorted_indices.len() > visible_height {
            let mut scrollbar_state =
                ScrollbarState::new(sorted_indices.len()).position(self.scroll_offset);

            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
                .render(
                    Rect {
                        x: inner.x + inner.width - 1,
                        y: inner.y,
                        width: 1,
                        height: visible_height as u16,
                    },
                    buf,
                    &mut scrollbar_state,
                );
        }
    }
}
