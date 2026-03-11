//! Diff view overlay for comparing two capture files side-by-side.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Clear, Widget};
use ratatui::style::{Color, Style};

use crate::theme::ThemeConfig;
use prb_core::{compute_aggregate_metrics, conversation::Conversation, DebugEvent, AggregateMetrics};
use std::collections::{HashMap, HashSet};

/// Diff entry representing matched or unmatched events.
#[derive(Debug, Clone)]
pub enum DiffEntry {
    /// Event present in both files (matched by similarity).
    Both {
        event1_idx: usize,
        event2_idx: usize,
    },
    /// Event unique to first file.
    OnlyInFirst {
        event_idx: usize,
    },
    /// Event unique to second file.
    OnlyInSecond {
        event_idx: usize,
    },
}

/// Conversation diff entry.
#[derive(Debug, Clone)]
pub enum ConversationDiff {
    /// Conversation present in both files.
    Both {
        conv1_idx: usize,
        conv2_idx: usize,
        changed: bool, // true if status/duration differs significantly
    },
    /// Conversation only in first file.
    OnlyInFirst {
        conv_idx: usize,
    },
    /// Conversation only in second file.
    OnlyInSecond {
        conv_idx: usize,
    },
}

/// Regression analysis result.
#[derive(Debug, Clone)]
pub struct RegressionReport {
    pub metrics1: AggregateMetrics,
    pub metrics2: AggregateMetrics,
    pub latency_p50_change_pct: f64,
    pub latency_p95_change_pct: f64,
    pub error_rate_change_pct: f64,
    pub new_error_count: usize,
    pub fixed_error_count: usize,
}

impl RegressionReport {
    /// Compute regression report from two sets of conversations.
    pub fn compute(convs1: &[&Conversation], convs2: &[&Conversation]) -> Self {
        let metrics1 = compute_aggregate_metrics(convs1);
        let metrics2 = compute_aggregate_metrics(convs2);

        let latency_p50_change_pct = if metrics1.latency_p50_ns > 0 {
            ((metrics2.latency_p50_ns as f64 - metrics1.latency_p50_ns as f64) / metrics1.latency_p50_ns as f64) * 100.0
        } else {
            0.0
        };

        let latency_p95_change_pct = if metrics1.latency_p95_ns > 0 {
            ((metrics2.latency_p95_ns as f64 - metrics1.latency_p95_ns as f64) / metrics1.latency_p95_ns as f64) * 100.0
        } else {
            0.0
        };

        let error_rate_change_pct = (metrics2.error_rate - metrics1.error_rate) * 100.0;

        // Count error conversations
        let errors1: HashSet<_> = convs1
            .iter()
            .filter(|c| c.state == prb_core::conversation::ConversationState::Error)
            .map(|c| &c.id)
            .collect();

        let errors2: HashSet<_> = convs2
            .iter()
            .filter(|c| c.state == prb_core::conversation::ConversationState::Error)
            .map(|c| &c.id)
            .collect();

        let new_error_count = errors2.difference(&errors1).count();
        let fixed_error_count = errors1.difference(&errors2).count();

        Self {
            metrics1,
            metrics2,
            latency_p50_change_pct,
            latency_p95_change_pct,
            error_rate_change_pct,
            new_error_count,
            fixed_error_count,
        }
    }
}

/// Diff view overlay showing side-by-side comparison.
pub struct DiffViewOverlay {
    pub selected: usize,
    pub scroll_offset: usize,
    #[allow(dead_code)]
    diff_entries: Vec<DiffEntry>,
    conv_diffs: Vec<ConversationDiff>,
    regression_report: Option<RegressionReport>,
}

impl DiffViewOverlay {
    /// Create a new diff view from two event lists.
    pub fn new(
        events1: &[DebugEvent],
        events2: &[DebugEvent],
        convs1: Option<&[Conversation]>,
        convs2: Option<&[Conversation]>,
    ) -> Self {
        let diff_entries = Self::compute_event_diff(events1, events2);
        let (conv_diffs, regression_report) = if let (Some(c1), Some(c2)) = (convs1, convs2) {
            let diffs = Self::compute_conversation_diff(c1, c2);
            let conv_refs1: Vec<&Conversation> = c1.iter().collect();
            let conv_refs2: Vec<&Conversation> = c2.iter().collect();
            let report = RegressionReport::compute(&conv_refs1, &conv_refs2);
            (diffs, Some(report))
        } else {
            (Vec::new(), None)
        };

        Self {
            selected: 0,
            scroll_offset: 0,
            diff_entries,
            conv_diffs,
            regression_report,
        }
    }

    /// Compute event-level diff by matching events with similar properties.
    fn compute_event_diff(events1: &[DebugEvent], events2: &[DebugEvent]) -> Vec<DiffEntry> {
        let mut entries = Vec::new();
        let mut matched2 = HashSet::new();

        // For each event in file 1, try to find a match in file 2
        for (idx1, e1) in events1.iter().enumerate() {
            if let Some(idx2) = Self::find_matching_event(e1, events2, &matched2) {
                matched2.insert(idx2);
                entries.push(DiffEntry::Both {
                    event1_idx: idx1,
                    event2_idx: idx2,
                });
            } else {
                entries.push(DiffEntry::OnlyInFirst { event_idx: idx1 });
            }
        }

        // Add unmatched events from file 2
        for (idx2, _e2) in events2.iter().enumerate() {
            if !matched2.contains(&idx2) {
                entries.push(DiffEntry::OnlyInSecond { event_idx: idx2 });
            }
        }

        entries
    }

    /// Find a matching event in events2 based on method, timestamp proximity, and source/dest.
    fn find_matching_event(
        e1: &DebugEvent,
        events2: &[DebugEvent],
        already_matched: &HashSet<usize>,
    ) -> Option<usize> {
        let mut best_match: Option<(usize, i64)> = None;
        const MAX_TIME_DIFF_NS: i64 = 100_000_000; // 100ms tolerance

        for (idx2, e2) in events2.iter().enumerate() {
            if already_matched.contains(&idx2) {
                continue;
            }

            // Check if method matches (if available)
            let method_match = e1.metadata.get("method") == e2.metadata.get("method");
            if !method_match && e1.metadata.contains_key("method") && e2.metadata.contains_key("method") {
                continue;
            }

            // Check if source/dest match (if network info available)
            if let (Some(n1), Some(n2)) = (&e1.source.network, &e2.source.network) {
                if n1.src != n2.src || n1.dst != n2.dst {
                    continue;
                }
            } else {
                // If no network info, skip matching
                continue;
            }

            // Check timestamp proximity
            let t1 = e1.timestamp;
            let t2 = e2.timestamp;
            let diff = (t2.as_nanos() as i64 - t1.as_nanos() as i64).abs();
            if diff > MAX_TIME_DIFF_NS {
                continue;
            }

            // Track best match by smallest time difference
            if let Some((_, best_diff)) = best_match {
                if diff < best_diff {
                    best_match = Some((idx2, diff));
                }
            } else {
                best_match = Some((idx2, diff));
            }
        }

        best_match.map(|(idx, _)| idx)
    }

    /// Compute conversation-level diff.
    fn compute_conversation_diff(
        convs1: &[Conversation],
        convs2: &[Conversation],
    ) -> Vec<ConversationDiff> {
        let mut diffs = Vec::new();
        let mut matched2 = HashSet::new();

        // Build lookup by conversation ID
        let conv2_by_id: HashMap<String, usize> = convs2
            .iter()
            .enumerate()
            .map(|(idx, c)| (c.id.0.clone(), idx))
            .collect();

        for (idx1, c1) in convs1.iter().enumerate() {
            if let Some(&idx2) = conv2_by_id.get(&c1.id.0) {
                matched2.insert(idx2);
                let c2 = &convs2[idx2];

                // Check if conversation changed significantly
                let changed = c1.state != c2.state
                    || (c1.metrics.duration_ns as i64 - c2.metrics.duration_ns as i64).abs()
                        > (c1.metrics.duration_ns / 10) as i64; // 10% change

                diffs.push(ConversationDiff::Both {
                    conv1_idx: idx1,
                    conv2_idx: idx2,
                    changed,
                });
            } else {
                diffs.push(ConversationDiff::OnlyInFirst { conv_idx: idx1 });
            }
        }

        // Add unmatched conversations from file 2
        for (idx2, _c2) in convs2.iter().enumerate() {
            if !matched2.contains(&idx2) {
                diffs.push(ConversationDiff::OnlyInSecond { conv_idx: idx2 });
            }
        }

        diffs
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &ThemeConfig) {
        // Calculate overlay dimensions (centered, large)
        let width = 100u16.min(area.width.saturating_sub(4));
        let height = 30u16.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let overlay_area = Rect::new(x, y, width, height);

        // Clear the area
        Clear.render(overlay_area, buf);

        // Render block
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Diff View: File Comparison ")
            .border_style(theme.focused_border());

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        if inner.height < 3 {
            return;
        }

        let mut y = inner.y;

        // Render regression report if available
        if let Some(ref report) = self.regression_report {
            y = self.render_regression_report(report, inner.x, y, inner.width, buf, theme);

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
        }

        // Render conversation diffs
        if !self.conv_diffs.is_empty() && y < inner.y + inner.height {
            buf.set_string(
                inner.x,
                y,
                "Conversation Diff:",
                Style::default().fg(Color::Cyan),
            );
            y += 1;

            for diff in self.conv_diffs.iter().skip(self.scroll_offset) {
                if y >= inner.y + inner.height {
                    break;
                }

                let (marker, msg, color) = match diff {
                    ConversationDiff::Both { changed: true, .. } => {
                        ("~", "Changed conversation", Color::Yellow)
                    }
                    ConversationDiff::Both { changed: false, .. } => {
                        ("=", "Unchanged conversation", Color::White)
                    }
                    ConversationDiff::OnlyInFirst { .. } => {
                        ("-", "Removed conversation", Color::Red)
                    }
                    ConversationDiff::OnlyInSecond { .. } => {
                        ("+", "New conversation", Color::Green)
                    }
                };

                let line = format!("  {} {}", marker, msg);
                buf.set_string(inner.x, y, line, Style::default().fg(color));
                y += 1;
            }
        }
    }

    fn render_regression_report(
        &self,
        report: &RegressionReport,
        x: u16,
        y: u16,
        _width: u16,
        buf: &mut Buffer,
        _theme: &ThemeConfig,
    ) -> u16 {
        let mut current_y = y;

        // Title
        buf.set_string(
            x,
            current_y,
            "Regression Report:",
            Style::default().fg(Color::Cyan),
        );
        current_y += 1;

        // Latency p50
        let p50_indicator = if report.latency_p50_change_pct > 10.0 {
            " ▲"
        } else if report.latency_p50_change_pct < -10.0 {
            " ▼"
        } else {
            ""
        };
        let p50_color = if report.latency_p50_change_pct > 10.0 {
            Color::Red
        } else if report.latency_p50_change_pct < -10.0 {
            Color::Green
        } else {
            Color::White
        };

        let line1 = format!(
            "  Latency p50: {} → {} ({:+.1}%){}",
            format_duration_ns(report.metrics1.latency_p50_ns),
            format_duration_ns(report.metrics2.latency_p50_ns),
            report.latency_p50_change_pct,
            p50_indicator
        );
        buf.set_string(x, current_y, line1, Style::default().fg(p50_color));
        current_y += 1;

        // Latency p95
        let p95_indicator = if report.latency_p95_change_pct > 10.0 {
            " ▲"
        } else if report.latency_p95_change_pct < -10.0 {
            " ▼"
        } else {
            ""
        };
        let p95_color = if report.latency_p95_change_pct > 10.0 {
            Color::Red
        } else if report.latency_p95_change_pct < -10.0 {
            Color::Green
        } else {
            Color::White
        };

        let line2 = format!(
            "  Latency p95: {} → {} ({:+.1}%){}",
            format_duration_ns(report.metrics1.latency_p95_ns),
            format_duration_ns(report.metrics2.latency_p95_ns),
            report.latency_p95_change_pct,
            p95_indicator
        );
        buf.set_string(x, current_y, line2, Style::default().fg(p95_color));
        current_y += 1;

        // Error rate
        let err_indicator = if report.error_rate_change_pct > 1.0 {
            " ▲"
        } else if report.error_rate_change_pct < -1.0 {
            " ▼"
        } else {
            ""
        };
        let err_color = if report.error_rate_change_pct > 1.0 {
            Color::Red
        } else if report.error_rate_change_pct < -1.0 {
            Color::Green
        } else {
            Color::White
        };

        let line3 = format!(
            "  Error rate: {:.1}% → {:.1}% ({:+.1}%){}",
            report.metrics1.error_rate * 100.0,
            report.metrics2.error_rate * 100.0,
            report.error_rate_change_pct,
            err_indicator
        );
        buf.set_string(x, current_y, line3, Style::default().fg(err_color));
        current_y += 1;

        // New/fixed errors
        if report.new_error_count > 0 {
            let line4 = format!("  New errors: {} conversations", report.new_error_count);
            buf.set_string(x, current_y, line4, Style::default().fg(Color::Red));
            current_y += 1;
        }

        if report.fixed_error_count > 0 {
            let line5 = format!("  Fixed errors: {} conversations", report.fixed_error_count);
            buf.set_string(x, current_y, line5, Style::default().fg(Color::Green));
            current_y += 1;
        }

        current_y
    }

    pub fn handle_scroll(&mut self, up: bool) {
        if up {
            self.scroll_offset = self.scroll_offset.saturating_sub(1);
        } else {
            self.scroll_offset = self.scroll_offset.saturating_add(1);
        }
    }
}

fn format_duration_ns(ns: u64) -> String {
    if ns == 0 {
        "0ms".to_string()
    } else if ns < 1_000 {
        format!("{}ns", ns)
    } else if ns < 1_000_000 {
        let us = ns as f64 / 1_000.0;
        if us < 10.0 {
            format!("{:.1}us", us)
        } else {
            format!("{}us", us as u64)
        }
    } else if ns < 1_000_000_000 {
        let ms = ns as f64 / 1_000_000.0;
        if ms < 10.0 {
            format!("{:.1}ms", ms)
        } else {
            format!("{}ms", ms as u64)
        }
    } else {
        format!("{:.1}s", ns as f64 / 1_000_000_000.0)
    }
}
