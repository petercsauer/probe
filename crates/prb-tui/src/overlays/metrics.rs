//! Metrics overlay for aggregate conversation statistics.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Clear, Widget};
use ratatui::style::{Color, Style};

use crate::theme::ThemeConfig;
use prb_core::{compute_aggregate_metrics, conversation::Conversation, TransportKind};
use std::collections::HashMap;

/// Metrics overlay showing aggregate statistics.
pub struct MetricsOverlay;

impl MetricsOverlay {
    pub fn new() -> Self {
        Self
    }

    pub fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        conversations: &[Conversation],
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
            .title(" Conversation Metrics ")
            .border_style(theme.focused_border());

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        if inner.height < 3 {
            return;
        }

        // Compute aggregate metrics
        let conv_refs: Vec<&Conversation> = conversations.iter().collect();
        let metrics = compute_aggregate_metrics(&conv_refs);

        // Compute per-protocol breakdown
        let by_protocol = self.compute_protocol_breakdown(conversations);

        // Render content
        let mut y = inner.y;

        // Overall metrics
        y = self.render_overall_metrics(&metrics, inner.x, y, inner.width, buf, theme);

        // Separator
        if y < inner.y + inner.height {
            buf.set_string(
                inner.x,
                y,
                "─".repeat(inner.width as usize),
                theme.focused_border(),
            );
            y += 1;
        }

        // Per-protocol breakdown
        if y < inner.y + inner.height {
            buf.set_string(
                inner.x,
                y,
                "By Protocol:",
                Style::default().fg(Color::Cyan),
            );
            y += 1;
        }

        for (protocol, stats) in by_protocol {
            if y >= inner.y + inner.height {
                break;
            }

            let line = format!(
                "  {:8}  {} conv  p50={}  {} errors",
                format!("{}:", protocol),
                stats.count,
                format_duration_ms(stats.p50_ns / 1_000_000),
                stats.error_count
            );
            buf.set_string(inner.x, y, line, theme.normal_row());
            y += 1;
        }
    }

    fn render_overall_metrics(
        &self,
        metrics: &prb_core::AggregateMetrics,
        x: u16,
        y: u16,
        _width: u16,
        buf: &mut Buffer,
        theme: &ThemeConfig,
    ) -> u16 {
        let mut current_y = y;

        // Line 1: Conversation count and error rate
        let error_rate_pct = (metrics.error_rate * 100.0) as u32;
        let line1 = format!(
            "Conversations: {}    Error rate: {:.1}%",
            metrics.total_conversations, error_rate_pct as f64
        );
        buf.set_string(x, current_y, line1, theme.normal_row());
        current_y += 1;

        // Line 2: Latency percentiles
        let p50_ms = metrics.latency_p50_ns / 1_000_000;
        let p95_ms = metrics.latency_p95_ns / 1_000_000;
        let p99_ms = metrics.latency_p99_ns / 1_000_000;
        let line2 = format!(
            "Latency:  p50={}  p95={}  p99={}",
            format_duration_ms(p50_ms),
            format_duration_ms(p95_ms),
            format_duration_ms(p99_ms)
        );
        buf.set_string(x, current_y, line2, theme.normal_row());
        current_y += 1;

        // Line 3: Throughput
        let kb_per_s = metrics.total_bytes as f64 / 1024.0 / metrics.conversations_per_second.max(1.0);
        let line3 = format!(
            "Throughput: {:.1} conv/s  {:.1} KB/s",
            metrics.conversations_per_second, kb_per_s
        );
        buf.set_string(x, current_y, line3, theme.normal_row());
        current_y += 1;

        current_y
    }

    fn compute_protocol_breakdown(
        &self,
        conversations: &[Conversation],
    ) -> Vec<(TransportKind, ProtocolStats)> {
        let mut by_protocol: HashMap<TransportKind, Vec<&Conversation>> = HashMap::new();

        for conv in conversations {
            by_protocol.entry(conv.protocol).or_default().push(conv);
        }

        let mut results: Vec<(TransportKind, ProtocolStats)> = by_protocol
            .into_iter()
            .map(|(protocol, convs)| {
                let count = convs.len();
                let error_count = convs
                    .iter()
                    .filter(|c| c.state == prb_core::conversation::ConversationState::Error)
                    .count();

                let mut durations: Vec<u64> = convs.iter().map(|c| c.metrics.duration_ns).collect();
                durations.sort_unstable();

                let p50_ns = if !durations.is_empty() {
                    durations[durations.len() / 2]
                } else {
                    0
                };

                (
                    protocol,
                    ProtocolStats {
                        count,
                        error_count,
                        p50_ns,
                    },
                )
            })
            .collect();

        // Sort by count (descending)
        results.sort_by(|a, b| b.1.count.cmp(&a.1.count));

        results
    }
}

impl Default for MetricsOverlay {
    fn default() -> Self {
        Self::new()
    }
}

struct ProtocolStats {
    count: usize,
    error_count: usize,
    p50_ns: u64,
}

fn format_duration_ms(ms: u64) -> String {
    if ms == 0 {
        "0ms".to_string()
    } else if ms < 1000 {
        format!("{}ms", ms)
    } else {
        format!("{:.1}s", ms as f64 / 1000.0)
    }
}
