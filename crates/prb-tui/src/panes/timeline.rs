use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Sparkline, Widget};

use crate::app::AppState;
use crate::panes::{Action, PaneComponent};
use crate::theme::Theme;

pub struct TimelinePane;

impl Default for TimelinePane {
    fn default() -> Self {
        Self::new()
    }
}

impl TimelinePane {
    pub fn new() -> Self {
        TimelinePane
    }
}

impl PaneComponent for TimelinePane {
    fn handle_key(&mut self, _key: KeyEvent, _state: &AppState) -> Action {
        Action::None
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
            .title(" Timeline ");

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 1 || inner.width < 10 {
            return;
        }

        let bucket_count = inner.width as usize;
        let buckets = state.store.time_buckets(&state.filtered_indices, bucket_count);

        // Sparkline takes the top line(s)
        let sparkline_height = inner.height.saturating_sub(1).max(1);
        let sparkline_area = Rect::new(inner.x, inner.y, inner.width, sparkline_height);

        let sparkline = Sparkline::default()
            .data(&buckets)
            .style(Theme::sparkline());

        sparkline.render(sparkline_area, buf);

        // Highlight the bucket corresponding to the selected event
        if let Some(selected_idx) = state.selected_event
            && let Some(bucket_idx) = calculate_selected_bucket(state, selected_idx, bucket_count)
        {
            let x = inner.x + bucket_idx as u16;
            let y = inner.y + sparkline_height - 1;
            if x < inner.x + inner.width && y < inner.y + inner.height {
                buf[(x, y)].set_style(Theme::selected_row());
            }
        }

        // Time range + protocol legend on the bottom line
        if inner.height >= 2 {
            let y = inner.y + inner.height - 1;
            let time_line = format_time_legend(state, inner.width);
            buf.set_line(inner.x, y, &time_line, inner.width);
        }
    }
}

fn calculate_selected_bucket(state: &AppState, selected_idx: usize, bucket_count: usize) -> Option<usize> {
    if bucket_count == 0 {
        return None;
    }

    let (start, end) = state.store.time_range()?;

    let selected_event = state.store.get(selected_idx)?;
    let range = end.as_nanos().saturating_sub(start.as_nanos());

    if range == 0 {
        return Some(0);
    }

    let bucket_width = range / bucket_count as u64;
    let offset = selected_event.timestamp.as_nanos().saturating_sub(start.as_nanos());
    let bucket_idx = if bucket_width > 0 {
        (offset / bucket_width).min(bucket_count as u64 - 1) as usize
    } else {
        0
    };

    Some(bucket_idx)
}

fn format_time_legend(state: &AppState, _width: u16) -> Line<'static> {
    let mut spans = Vec::new();

    if let Some((start, end)) = state.store.time_range() {
        let start_str = format_timestamp_short(start.as_nanos());
        let end_str = format_timestamp_short(end.as_nanos());
        spans.push(Span::styled(
            format!(" {} --- {} ", start_str, end_str),
            Theme::hex_offset(),
        ));
    }

    let counts = state.store.protocol_counts(&state.filtered_indices);
    for (kind, count) in counts.iter().take(4) {
        let color = Theme::transport_color(*kind);
        spans.push(Span::styled(
            format!(" {}: {} ", kind, count),
            ratatui::style::Style::default().fg(color),
        ));
    }

    // Show (filtered) indicator if a filter is active
    if state.filter.is_some() {
        spans.push(Span::styled(
            " (filtered) ",
            ratatui::style::Style::default().fg(ratatui::style::Color::Yellow),
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
