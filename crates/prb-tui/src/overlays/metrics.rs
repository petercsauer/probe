//! Metrics overlay for aggregate event statistics.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Clear, Widget};
use ratatui::style::{Color, Style};

use crate::theme::ThemeConfig;
use crate::event_store::EventStore;
use prb_core::DebugEvent;

/// Metrics overlay showing aggregate statistics.
pub struct MetricsOverlay;

impl MetricsOverlay {
    pub fn new() -> Self {
        Self
    }

    /// Render metrics overlay using EventStore data.
    pub fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        store: &EventStore,
        filtered_indices: &[usize],
        theme: &ThemeConfig,
    ) {
        // Calculate overlay dimensions (centered, reasonable size)
        let width = 70u16.min(area.width.saturating_sub(4));
        let height = 20u16.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let overlay_area = Rect::new(x, y, width, height);

        // Clear the area
        Clear.render(overlay_area, buf);

        // Render block
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Metrics ")
            .border_style(theme.focused_border());

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        if inner.height < 3 {
            return;
        }

        // Calculate metrics from EventStore
        let total_events = store.len();
        let filtered_events = filtered_indices.len();
        let filter_pct = if total_events > 0 {
            (filtered_events as f64 / total_events as f64) * 100.0
        } else {
            0.0
        };

        // Calculate protocol distribution
        let protocol_counts = store.protocol_counts(filtered_indices);

        // Calculate total bytes
        let total_bytes: usize = filtered_indices
            .iter()
            .filter_map(|&idx| store.get(idx))
            .map(|event| self.get_event_size(event))
            .sum();

        // Calculate error count (events with warnings)
        let error_count: usize = filtered_indices
            .iter()
            .filter_map(|&idx| store.get(idx))
            .filter(|event| !event.warnings.is_empty())
            .count();

        let error_rate = if filtered_events > 0 {
            (error_count as f64 / filtered_events as f64) * 100.0
        } else {
            0.0
        };

        // Calculate throughput based on time range
        let (events_per_sec, bytes_per_sec) = if let Some((start, end)) = store.time_range() {
            let duration_ns = end.as_nanos().saturating_sub(start.as_nanos());
            if duration_ns > 0 {
                let duration_sec = duration_ns as f64 / 1_000_000_000.0;
                let eps = filtered_events as f64 / duration_sec;
                let bps = total_bytes as f64 / duration_sec;
                (eps, bps)
            } else {
                (0.0, 0.0)
            }
        } else {
            (0.0, 0.0)
        };

        // Render content
        let mut current_y = inner.y;

        // Line 1: Total and filtered events
        let line1 = format!(
            "Total Events:     {}",
            format_count(total_events)
        );
        buf.set_string(inner.x, current_y, line1, theme.normal_row());
        current_y += 1;

        let line2 = format!(
            "Filtered:         {}  ({:.0}%)",
            format_count(filtered_events),
            filter_pct
        );
        buf.set_string(inner.x, current_y, line2, theme.normal_row());
        current_y += 1;

        // Blank line
        current_y += 1;

        // Throughput
        buf.set_string(inner.x, current_y, "Throughput:", Style::default().fg(Color::Cyan));
        current_y += 1;

        let line3 = format!("  Events/sec:     {:.1}", events_per_sec);
        buf.set_string(inner.x, current_y, line3, theme.normal_row());
        current_y += 1;

        let line4 = format!("  Bytes/sec:      {}", format_bytes(bytes_per_sec as usize));
        buf.set_string(inner.x, current_y, line4, theme.normal_row());
        current_y += 1;

        // Blank line
        current_y += 1;

        // Protocol distribution
        if current_y < inner.y + inner.height {
            buf.set_string(
                inner.x,
                current_y,
                "Protocols:",
                Style::default().fg(Color::Cyan),
            );
            current_y += 1;
        }

        for (protocol, count) in protocol_counts.iter().take(5) {
            if current_y >= inner.y + inner.height {
                break;
            }

            let pct = if filtered_events > 0 {
                (*count as f64 / filtered_events as f64) * 100.0
            } else {
                0.0
            };

            let line = format!(
                "  {:10}  {}  ({:.0}%)",
                format!("{}:", protocol),
                format_count(*count),
                pct
            );
            buf.set_string(inner.x, current_y, line, theme.normal_row());
            current_y += 1;
        }

        // Blank line
        if current_y < inner.y + inner.height {
            current_y += 1;
        }

        // Errors
        if current_y < inner.y + inner.height {
            let error_line = format!(
                "Errors:           {}  ({:.1}%)",
                format_count(error_count),
                error_rate
            );
            buf.set_string(inner.x, current_y, error_line, theme.normal_row());
            current_y += 1;
        }

        // Latency section (placeholder - requires conversation tracking)
        if current_y < inner.y + inner.height {
            current_y += 1;
            buf.set_string(
                inner.x,
                current_y,
                "Latency:          (requires conversation tracking)",
                Style::default().fg(Color::DarkGray),
            );
        }
    }

    /// Get the size of an event in bytes.
    fn get_event_size(&self, event: &DebugEvent) -> usize {
        match &event.payload {
            prb_core::Payload::Raw { raw } => raw.len(),
            prb_core::Payload::Decoded { raw, .. } => raw.len(),
        }
    }
}

impl Default for MetricsOverlay {
    fn default() -> Self {
        Self::new()
    }
}

/// Format count with commas.
fn format_count(n: usize) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut result = String::new();

    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(b as char);
    }

    result
}

/// Format bytes with units (B, KB, MB, GB).
fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B/s", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB/s", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB/s", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB/s", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
