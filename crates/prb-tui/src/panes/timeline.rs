use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Widget};

use crate::app::AppState;
use crate::panes::{Action, PaneComponent};
use crate::theme::ThemeConfig;
use prb_core::{Timestamp, TransportKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimelineMode {
    /// Show stacked per-protocol sparklines
    MultiProtocol,
    /// Show latency heatmap (when conversations available)
    LatencyHeatmap,
}

pub struct TimelinePane {
    /// Current cursor position (bucket index)
    cursor: Option<usize>,
    /// Number of buckets in the current render
    bucket_count: usize,
    /// Time range selection (start_bucket, end_bucket)
    selection: Option<(usize, usize)>,
    /// Current display mode
    mode: TimelineMode,
    /// Zoom level (0.25 = 25% detail, 1.0 = normal, 4.0 = 4x detail)
    zoom_level: f64,
}

impl Default for TimelinePane {
    fn default() -> Self {
        Self::new()
    }
}

impl TimelinePane {
    pub fn new() -> Self {
        TimelinePane {
            cursor: None,
            bucket_count: 0,
            selection: None,
            mode: TimelineMode::MultiProtocol,
            zoom_level: 1.0,
        }
    }

    fn bucket_time_range(&self, state: &AppState, bucket_idx: usize) -> Option<(Timestamp, Timestamp)> {
        let (start, end) = state.store.time_range()?;
        if self.bucket_count == 0 {
            return None;
        }

        let range = end.as_nanos().saturating_sub(start.as_nanos());
        let bucket_width = range / self.bucket_count as u64;

        let bucket_start = start.as_nanos() + (bucket_idx as u64 * bucket_width);
        let bucket_end = bucket_start + bucket_width;

        Some((Timestamp::from_nanos(bucket_start), Timestamp::from_nanos(bucket_end)))
    }

    fn events_in_bucket(&self, state: &AppState, bucket_idx: usize) -> Vec<usize> {
        let Some((bucket_start, bucket_end)) = self.bucket_time_range(state, bucket_idx) else {
            return Vec::new();
        };

        state.filtered_indices
            .iter()
            .filter_map(|&idx| {
                let event = state.store.get(idx)?;
                let ts = event.timestamp.as_nanos();
                if ts >= bucket_start.as_nanos() && ts < bucket_end.as_nanos() {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect()
    }
}

impl PaneComponent for TimelinePane {
    fn handle_key(&mut self, key: KeyEvent, state: &AppState) -> Action {
        match key.code {
            KeyCode::Left => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    // Time range selection
                    if let Some(cursor) = self.cursor {
                        if let Some((start, end)) = self.selection {
                            if cursor < start {
                                self.selection = Some((cursor, end));
                            } else {
                                self.selection = Some((start, cursor));
                            }
                        } else {
                            self.selection = Some((cursor, cursor));
                        }
                    }
                } else {
                    // Move cursor left
                    if self.bucket_count > 0 {
                        self.cursor = match self.cursor {
                            Some(pos) if pos > 0 => Some(pos - 1),
                            Some(_) => Some(0),
                            None => Some(self.bucket_count.saturating_sub(1)),
                        };
                    }
                }
                Action::None
            }
            KeyCode::Right => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    // Time range selection
                    if let Some(cursor) = self.cursor {
                        if let Some((start, end)) = self.selection {
                            if cursor > end {
                                self.selection = Some((start, cursor));
                            } else {
                                self.selection = Some((cursor, end));
                            }
                        } else {
                            self.selection = Some((cursor, cursor));
                        }
                    }
                } else {
                    // Move cursor right
                    if self.bucket_count > 0 {
                        self.cursor = match self.cursor {
                            Some(pos) if pos < self.bucket_count - 1 => Some(pos + 1),
                            Some(_) => Some(self.bucket_count - 1),
                            None => Some(0),
                        };
                    }
                }
                Action::None
            }
            KeyCode::Enter => {
                // Jump to first event in cursor bucket
                if let Some(cursor_pos) = self.cursor {
                    let events = self.events_in_bucket(state, cursor_pos);
                    if let Some(&first_event) = events.first() {
                        return Action::SelectEvent(first_event);
                    }
                }
                Action::None
            }
            KeyCode::Char('h') | KeyCode::Char('H') => {
                // Toggle between multi-protocol and latency heatmap
                if state.conversations.is_some() {
                    self.mode = match self.mode {
                        TimelineMode::MultiProtocol => TimelineMode::LatencyHeatmap,
                        TimelineMode::LatencyHeatmap => TimelineMode::MultiProtocol,
                    };
                }
                Action::None
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                // Zoom in (increase detail)
                self.zoom_level = (self.zoom_level * 1.5).min(4.0);
                Action::None
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                // Zoom out (decrease detail)
                self.zoom_level = (self.zoom_level / 1.5).max(0.25);
                Action::None
            }
            KeyCode::Esc => {
                // Clear selection
                self.selection = None;
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, state: &AppState, theme: &ThemeConfig, focused: bool) {
        let border_style = if focused {
            theme.focused_border()
        } else {
            theme.unfocused_border()
        };

        let title = match self.mode {
            TimelineMode::MultiProtocol => " Timeline ",
            TimelineMode::LatencyHeatmap => " Timeline (Latency Heatmap) ",
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title);

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 1 || inner.width < 10 {
            return;
        }

        // Show hint if no events loaded
        if state.store.is_empty() {
            let msg = Text::styled(
                "  Load a capture to see time distribution",
                Style::default().fg(Color::DarkGray),
            );
            Widget::render(msg, inner, buf);
            return;
        }

        self.bucket_count = ((inner.width as f64) * self.zoom_level) as usize;
        self.bucket_count = self.bucket_count.max(1);

        match self.mode {
            TimelineMode::MultiProtocol => {
                self.render_multi_protocol(inner, buf, state, theme, focused);
            }
            TimelineMode::LatencyHeatmap => {
                if state.conversations.is_some() {
                    self.render_latency_heatmap(inner, buf, state, theme, focused);
                } else {
                    // Fallback to multi-protocol if no conversations
                    self.render_multi_protocol(inner, buf, state, theme, focused);
                }
            }
        }
    }
}

impl TimelinePane {
    fn render_multi_protocol(&mut self, area: Rect, buf: &mut Buffer, state: &AppState, theme: &ThemeConfig, focused: bool) {
        // Get protocol counts for display
        let protocol_counts = state.store.protocol_counts(&state.filtered_indices);
        let active_protocols: Vec<_> = protocol_counts.iter().map(|(p, _)| *p).collect();

        if active_protocols.is_empty() {
            return;
        }

        // Calculate sparkline area (leave room for time legend at bottom)
        let sparkline_height = area.height.saturating_sub(1).max(1);
        let rows_per_protocol = if active_protocols.len() == 1 {
            sparkline_height as usize
        } else {
            (sparkline_height as usize) / active_protocols.len()
        };

        if rows_per_protocol == 0 {
            return;
        }

        // Build per-protocol buckets
        let mut protocol_buckets: Vec<(TransportKind, Vec<u64>)> = Vec::new();

        for &protocol in &active_protocols {
            let protocol_indices: Vec<usize> = state.filtered_indices
                .iter()
                .filter(|&&idx| {
                    state.store.get(idx).map(|e| e.transport == protocol).unwrap_or(false)
                })
                .copied()
                .collect();

            let buckets = state.store.time_buckets(&protocol_indices, self.bucket_count);
            protocol_buckets.push((protocol, buckets));
        }

        // Render each protocol's sparkline
        for (protocol_idx, (protocol, buckets)) in protocol_buckets.iter().enumerate() {
            let y_start = area.y + (protocol_idx * rows_per_protocol) as u16;
            let row_height = rows_per_protocol.min((sparkline_height as usize) - (protocol_idx * rows_per_protocol));

            if row_height == 0 {
                break;
            }

            let protocol_area = Rect::new(area.x, y_start, area.width, row_height as u16);
            self.render_sparkline(protocol_area, buf, buckets, *protocol, theme, focused);
        }

        // Render cursor and selection highlights
        if focused {
            self.render_cursor_and_selection(area, buf, theme, sparkline_height);
        }

        // Time range + protocol legend on the bottom line
        if area.height >= 2 {
            let y = area.y + area.height - 1;
            let time_line = format_time_legend(state, area.width, theme, &self.cursor, self);
            buf.set_line(area.x, y, &time_line, area.width);
        }
    }

    fn render_sparkline(&self, area: Rect, buf: &mut Buffer, buckets: &[u64], protocol: TransportKind, theme: &ThemeConfig, _focused: bool) {
        if buckets.is_empty() || area.height == 0 {
            return;
        }

        let max_val = buckets.iter().max().copied().unwrap_or(1).max(1);
        let color = theme.transport_color(protocol);

        // Use block characters to render sparkline
        let chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

        for (i, &count) in buckets.iter().enumerate() {
            if i >= area.width as usize {
                break;
            }

            let normalized = if max_val > 0 {
                ((count as f64 / max_val as f64) * (chars.len() - 1) as f64).round() as usize
            } else {
                0
            };

            let ch = if count == 0 {
                ' '
            } else {
                chars[normalized.min(chars.len() - 1)]
            };

            let x = area.x + i as u16;
            let y = area.y + area.height - 1;

            if x < area.x + area.width && y < area.y + area.height {
                buf[(x, y)]
                    .set_char(ch)
                    .set_style(Style::default().fg(color));
            }
        }

        // Show protocol name on the left if there's room
        if area.width > 10 && area.height > 0 {
            let label = format!("{}", protocol);
            let label_x = area.x;
            let label_y = area.y;
            for (i, ch) in label.chars().enumerate() {
                if i >= 4 || label_x + i as u16 >= area.x + area.width {
                    break;
                }
                buf[(label_x + i as u16, label_y)]
                    .set_char(ch)
                    .set_style(Style::default().fg(Color::DarkGray));
            }
        }
    }

    fn render_cursor_and_selection(&self, area: Rect, buf: &mut Buffer, theme: &ThemeConfig, sparkline_height: u16) {
        // Render selection range
        if let Some((start, end)) = self.selection {
            let actual_start = start.min(end);
            let actual_end = start.max(end);

            for bucket in actual_start..=actual_end {
                if bucket >= self.bucket_count {
                    break;
                }
                let x = area.x + bucket as u16;
                for dy in 0..sparkline_height {
                    let y = area.y + dy;
                    if x < area.x + area.width && y < area.y + area.height {
                        buf[(x, y)]
                            .set_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::DIM));
                    }
                }
            }
        }

        // Render cursor
        if let Some(cursor_pos) = self.cursor
            && cursor_pos < self.bucket_count
        {
            let x = area.x + cursor_pos as u16;
            for dy in 0..sparkline_height {
                let y = area.y + dy;
                if x < area.x + area.width && y < area.y + area.height {
                    buf[(x, y)]
                        .set_style(theme.selected_row().add_modifier(Modifier::BOLD));
                }
            }
        }
    }

    fn render_latency_heatmap(&mut self, area: Rect, buf: &mut Buffer, state: &AppState, theme: &ThemeConfig, focused: bool) {
        let Some(ref conversations) = state.conversations else {
            return;
        };

        // Latency buckets: 0-10ms, 10-50ms, 50-100ms, >100ms
        const LATENCY_BUCKETS: &[(u64, &str)] = &[
            (10_000_000, "0-10ms"),
            (50_000_000, "10-50ms"),
            (100_000_000, "50-100ms"),
            (u64::MAX, ">100ms"),
        ];

        let heatmap_height = area.height.saturating_sub(1).max(1);
        let num_latency_buckets = LATENCY_BUCKETS.len().min(heatmap_height as usize);

        if num_latency_buckets == 0 {
            return;
        }

        // Build time range
        let Some((start_ts, end_ts)) = state.store.time_range() else {
            return;
        };

        let time_range = end_ts.as_nanos().saturating_sub(start_ts.as_nanos());
        if time_range == 0 {
            return;
        }

        // Initialize heatmap grid: [time_bucket][latency_bucket] = count
        let mut grid = vec![vec![0u64; num_latency_buckets]; self.bucket_count];

        // Fill grid with conversation data
        for conv in &conversations.conversations {
            let Some(ttfr) = conv.metrics.time_to_first_response_ns else {
                continue;
            };

            // Determine latency bucket
            let latency_bucket = LATENCY_BUCKETS
                .iter()
                .position(|(threshold, _)| ttfr <= *threshold)
                .unwrap_or(num_latency_buckets - 1);

            // Determine time bucket based on conversation start time
            if let Some(start_time) = conv.metrics.start_time {
                let offset = start_time.as_nanos().saturating_sub(start_ts.as_nanos());
                let bucket_width = time_range / self.bucket_count as u64;
                let time_bucket = if bucket_width > 0 {
                    ((offset / bucket_width) as usize).min(self.bucket_count - 1)
                } else {
                    0
                };

                grid[time_bucket][latency_bucket] += 1;
            }
        }

        // Find max count for normalization
        let max_count = grid
            .iter()
            .flat_map(|row| row.iter())
            .max()
            .copied()
            .unwrap_or(1)
            .max(1);

        // Render heatmap
        let chars = [' ', '░', '▒', '▓', '█'];
        let colors = [
            Color::Green,      // 0-10ms - good
            Color::Yellow,     // 10-50ms - ok
            Color::LightRed,   // 50-100ms - slow
            Color::Red,        // >100ms - bad
        ];

        for latency_idx in 0..num_latency_buckets {
            let y = area.y + latency_idx as u16;

            // Draw label
            let label = LATENCY_BUCKETS[latency_idx].1;
            for (i, ch) in label.chars().enumerate() {
                if i >= 8 {
                    break;
                }
                buf[(area.x + i as u16, y)]
                    .set_char(ch)
                    .set_style(Style::default().fg(Color::DarkGray));
            }

            // Draw heatmap cells
            let heatmap_start_x = area.x + 9;
            for (time_idx, bucket_data) in grid.iter().enumerate().take(self.bucket_count) {
                let x = heatmap_start_x + time_idx as u16;
                if x >= area.x + area.width {
                    break;
                }

                let count = bucket_data[latency_idx];
                let intensity = if max_count > 0 {
                    ((count as f64 / max_count as f64) * (chars.len() - 1) as f64).round() as usize
                } else {
                    0
                };

                let ch = chars[intensity.min(chars.len() - 1)];
                let color = colors[latency_idx];

                buf[(x, y)]
                    .set_char(ch)
                    .set_style(Style::default().fg(color));
            }
        }

        // Render cursor
        if focused
            && let Some(cursor_pos) = self.cursor
            && cursor_pos < self.bucket_count
        {
            let x = area.x + 9 + cursor_pos as u16;
            for latency_idx in 0..num_latency_buckets {
                let y = area.y + latency_idx as u16;
                if x < area.x + area.width && y < area.y + area.height {
                    buf[(x, y)]
                        .set_style(theme.selected_row().add_modifier(Modifier::BOLD));
                }
            }
        }

        // Time range legend at bottom
        if area.height >= 2 {
            let y = area.y + area.height - 1;
            let time_line = format_time_legend(state, area.width, theme, &self.cursor, self);
            buf.set_line(area.x, y, &time_line, area.width);
        }
    }
}

fn format_time_legend(state: &AppState, _width: u16, theme: &ThemeConfig, cursor: &Option<usize>, timeline: &TimelinePane) -> Line<'static> {
    let mut spans = Vec::new();

    if let Some((start, end)) = state.store.time_range() {
        let start_str = format_timestamp_short(start.as_nanos());
        let end_str = format_timestamp_short(end.as_nanos());
        spans.push(Span::styled(
            format!(" {} --- {} ", start_str, end_str),
            theme.hex_offset(),
        ));
    }

    // Show cursor info if active
    if let Some(cursor_pos) = cursor
        && let Some((bucket_start, bucket_end)) = timeline.bucket_time_range(state, *cursor_pos)
    {
        let events_in_bucket = timeline.events_in_bucket(state, *cursor_pos).len();
        let start_str = format_timestamp_short(bucket_start.as_nanos());
        let end_str = format_timestamp_short(bucket_end.as_nanos());
        spans.push(Span::styled(
            format!(" ▶ {}-{} ({} events) ", start_str, end_str, events_in_bucket),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ));
    }

    // Show protocol counts (only in multi-protocol mode)
    if matches!(timeline.mode, TimelineMode::MultiProtocol) {
        let counts = state.store.protocol_counts(&state.filtered_indices);
        for (kind, count) in counts.iter().take(3) {
            let color = theme.transport_color(*kind);
            spans.push(Span::styled(
                format!(" {}: {} ", kind, count),
                Style::default().fg(color),
            ));
        }
    }

    // Show (filtered) indicator if a filter is active
    if state.filter.is_some() {
        spans.push(Span::styled(
            " (filtered) ",
            Style::default().fg(Color::Yellow),
        ));
    }

    // Show selection info
    if let Some((start, end)) = timeline.selection {
        let actual_start = start.min(end);
        let actual_end = start.max(end);
        let width = actual_end - actual_start + 1;
        spans.push(Span::styled(
            format!(" [sel: {}] ", width),
            Style::default().fg(Color::Magenta),
        ));
    }

    // Show zoom level if not default
    if (timeline.zoom_level - 1.0).abs() > 0.01 {
        spans.push(Span::styled(
            format!(" [zoom: {:.1}x] ", timeline.zoom_level),
            Style::default().fg(Color::Green),
        ));
    }

    Line::from(spans)
}

fn format_timestamp_short(ns: u64) -> String {
    let secs = ns / 1_000_000_000;
    let millis = (ns % 1_000_000_000) / 1_000_000;
    let h = (secs / 3600) % 24;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, millis)
}

// Test helpers
#[doc(hidden)]
pub fn test_format_time_legend(state: &AppState, width: u16) -> Line<'static> {
    let theme = ThemeConfig::dark();
    let timeline = TimelinePane::new();
    format_time_legend(state, width, &theme, &None, &timeline)
}

#[doc(hidden)]
pub fn test_format_timestamp_short(ns: u64) -> String {
    format_timestamp_short(ns)
}
