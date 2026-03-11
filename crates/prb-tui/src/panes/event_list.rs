use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget,
};
use unicode_width::UnicodeWidthStr;

use crate::app::AppState;
use crate::panes::{Action, PaneComponent};
use crate::theme::ThemeConfig;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Id,
    Time,
    Source,
    Dest,
    Protocol,
    Dir,
}

/// Cached view of sorted/filtered indices and protocol counts.
/// Only recomputes when filter or sort changes.
#[derive(Debug, Clone)]
struct CachedView {
    filter_hash: u64,
    sort_column: SortColumn,
    sort_reversed: bool,
    sorted_indices: Vec<usize>,
    #[allow(dead_code)]
    protocol_counts: Vec<(prb_core::TransportKind, usize)>,
}

impl SortColumn {
    pub fn next(self) -> Self {
        match self {
            SortColumn::Id => SortColumn::Time,
            SortColumn::Time => SortColumn::Source,
            SortColumn::Source => SortColumn::Dest,
            SortColumn::Dest => SortColumn::Protocol,
            SortColumn::Protocol => SortColumn::Dir,
            SortColumn::Dir => SortColumn::Id,
        }
    }
}

pub struct EventListPane {
    pub selected: usize,
    pub scroll_offset: usize,
    pub sort_column: SortColumn,
    pub sort_reversed: bool,
    cached_view: Option<CachedView>,
}

#[derive(Debug, Clone, Copy)]
struct ColumnWidths {
    id: u16,
    time: u16,
    src: u16,
    dst: u16,
    proto: u16,
    dir: u16,
    summary: u16,
}

impl Default for EventListPane {
    fn default() -> Self {
        Self::new()
    }
}

/// Format source address or fallback to origin name.
fn format_source(event: &prb_core::DebugEvent) -> String {
    event
        .source
        .network
        .as_ref()
        .map(|n| n.src.clone())
        .unwrap_or_else(|| event.source.origin.clone())
}

/// Format destination address or fallback to "-".
fn format_dest(event: &prb_core::DebugEvent) -> String {
    event
        .source
        .network
        .as_ref()
        .map(|n| n.dst.clone())
        .unwrap_or_else(|| String::from("-"))
}

/// Compute adaptive column widths based on visible events.
fn compute_column_widths(
    events: &[&prb_core::DebugEvent],
    total_width: u16,
) -> ColumnWidths {
    let has_network = events.iter().any(|e| e.source.network.is_some());

    if has_network {
        // Full layout: #(6) Time(13) Src(19) Dst(19) Proto(11) Dir(4) Summary(fill)
        let fixed = 6 + 13 + 19 + 19 + 11 + 4;
        let summary = (total_width as usize).saturating_sub(fixed) as u16;
        ColumnWidths {
            id: 6,
            time: 13,
            src: 19,
            dst: 19,
            proto: 11,
            dir: 4,
            summary,
        }
    } else {
        // Collapsed: #(6) Time(13) Origin(20) Proto(11) Dir(4) Summary(fill)
        let fixed = 6 + 13 + 20 + 11 + 4;
        let summary = (total_width as usize).saturating_sub(fixed) as u16;
        ColumnWidths {
            id: 6,
            time: 13,
            src: 20,
            dst: 0,
            proto: 11,
            dir: 4,
            summary,
        }
    }
}

impl EventListPane {
    pub fn new() -> Self {
        EventListPane {
            selected: 0,
            scroll_offset: 0,
            sort_column: SortColumn::Time,
            sort_reversed: false,
            cached_view: None,
        }
    }

    pub fn ensure_visible(&mut self, position: usize, context_lines: usize) {
        // Ensure the position is visible with some context lines around it
        if position < self.scroll_offset + context_lines {
            self.scroll_offset = position.saturating_sub(context_lines);
        }
        // Note: We don't adjust for the bottom because we don't know the visible height here
        // The caller should handle that if needed
    }

    fn total_items(state: &AppState) -> usize {
        state.filtered_indices.len()
    }

    fn sorted_indices(&mut self, state: &AppState) -> &[usize] {
        // Compute hash of current filter state
        let mut hasher = DefaultHasher::new();
        state.filtered_indices.hash(&mut hasher);
        let filter_hash = hasher.finish();

        // Check if we can use cached view
        let needs_recompute = self.cached_view.as_ref().is_none_or(|cache| {
            cache.filter_hash != filter_hash
                || cache.sort_column != self.sort_column
                || cache.sort_reversed != self.sort_reversed
        });

        if needs_recompute {
            let mut indices = state.filtered_indices.clone();

            indices.sort_by(|&a, &b| {
                let event_a = state.store.get(a);
                let event_b = state.store.get(b);

                if event_a.is_none() || event_b.is_none() {
                    return std::cmp::Ordering::Equal;
                }

                let event_a = event_a.unwrap();
                let event_b = event_b.unwrap();

                let cmp = match self.sort_column {
                    SortColumn::Id => event_a.id.as_u64().cmp(&event_b.id.as_u64()),
                    SortColumn::Time => event_a.timestamp.cmp(&event_b.timestamp),
                    SortColumn::Source => {
                        let src_a = event_a.source.network.as_ref().map(|n| n.src.as_str()).unwrap_or("");
                        let src_b = event_b.source.network.as_ref().map(|n| n.src.as_str()).unwrap_or("");
                        src_a.cmp(src_b)
                    }
                    SortColumn::Dest => {
                        let dst_a = event_a.source.network.as_ref().map(|n| n.dst.as_str()).unwrap_or("");
                        let dst_b = event_b.source.network.as_ref().map(|n| n.dst.as_str()).unwrap_or("");
                        dst_a.cmp(dst_b)
                    }
                    SortColumn::Protocol => event_a.transport.cmp(&event_b.transport),
                    SortColumn::Dir => event_a.direction.cmp(&event_b.direction),
                };

                if self.sort_reversed {
                    cmp.reverse()
                } else {
                    cmp
                }
            });

            // Compute protocol counts
            let protocol_counts = state.store.protocol_counts(&indices);

            self.cached_view = Some(CachedView {
                filter_hash,
                sort_column: self.sort_column,
                sort_reversed: self.sort_reversed,
                sorted_indices: indices,
                protocol_counts,
            });
        }

        &self.cached_view.as_ref().unwrap().sorted_indices
    }
}

impl PaneComponent for EventListPane {
    fn handle_key(&mut self, key: KeyEvent, state: &AppState) -> Action {
        let total = Self::total_items(state);
        if total == 0 {
            return Action::None;
        }
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < total {
                    self.selected += 1;
                }
                Action::SelectEvent(self.selected)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                Action::SelectEvent(self.selected)
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.selected = 0;
                self.scroll_offset = 0;
                Action::SelectEvent(self.selected)
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.selected = total.saturating_sub(1);
                Action::SelectEvent(self.selected)
            }
            KeyCode::PageDown => {
                self.selected = (self.selected + 20).min(total.saturating_sub(1));
                Action::SelectEvent(self.selected)
            }
            KeyCode::PageUp => {
                self.selected = self.selected.saturating_sub(20);
                Action::SelectEvent(self.selected)
            }
            KeyCode::Enter => Action::SelectEvent(self.selected),
            KeyCode::Char('s') => {
                self.sort_column = self.sort_column.next();
                Action::None
            }
            KeyCode::Char('S') => {
                self.sort_reversed = !self.sort_reversed;
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, state: &AppState, theme: &ThemeConfig, focused: bool) {
        use ratatui::widgets::BorderType;

        let block = if focused {
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.focused_border())
                .title(" Events [*] ")
                .title_style(theme.focused_title())
        } else {
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(theme.unfocused_border())
                .title(" Events ")
                .title_style(theme.unfocused_title())
        };

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 2 || inner.width < 10 {
            return;
        }

        let vis_height = inner.height.saturating_sub(1) as usize;

        // Copy values we need before borrowing self.sorted_indices
        let scroll_offset = self.scroll_offset;
        let selected = self.selected;
        let sort_column = self.sort_column;
        let sort_reversed = self.sort_reversed;

        let sorted = self.sorted_indices(state);
        let total = sorted.len();

        // Collect visible events for column width computation
        let scroll_offset_start = {
            let mut off = scroll_offset;
            if selected < off {
                off = selected;
            } else if selected >= off + vis_height {
                off = selected.saturating_sub(vis_height - 1);
            }
            off
        };

        let visible_events: Vec<_> = (scroll_offset_start..total.min(scroll_offset_start + vis_height))
            .filter_map(|idx| {
                let event_idx = sorted[idx];
                state.store.get(event_idx)
            })
            .collect();

        let col_widths = compute_column_widths(&visible_events, inner.width);

        // Header row
        let header = format_header(col_widths, sort_column, sort_reversed, theme);
        buf.set_line(inner.x, inner.y, &header, inner.width);

        for i in 0..vis_height {
            let idx = scroll_offset_start + i;
            if idx >= total {
                break;
            }
            let event_idx = sorted[idx];
            if let Some(event) = state.store.get(event_idx) {
                let y = inner.y + 1 + i as u16;
                let is_selected = idx == selected;

                let row_style = if is_selected {
                    theme.selected_row()
                } else if !event.warnings.is_empty() {
                    theme.warning_row()
                } else if idx % 2 == 1 {
                    theme.zebra_row()
                } else {
                    theme.normal_row()
                };

                let transport_color = theme.transport_color(event.transport);
                let dir_sym = ThemeConfig::direction_symbol(event.direction);

                let ts_ns = event.timestamp.as_nanos();
                let secs = ts_ns / 1_000_000_000;
                let millis = (ts_ns % 1_000_000_000) / 1_000_000;
                let h = (secs / 3600) % 24;
                let m = (secs % 3600) / 60;
                let s = secs % 60;

                let src = format_source(event);
                let dst = format_dest(event);

                let summary = event
                    .metadata
                    .values()
                    .next()
                    .cloned()
                    .unwrap_or_default();

                let transport_style = if is_selected {
                    row_style
                } else {
                    row_style.fg(transport_color)
                };

                // Add warning indicator prefix for events with warnings
                let warning_indicator = if !event.warnings.is_empty() { "!" } else { " " };
                let warning_style = if !event.warnings.is_empty() && !is_selected {
                    theme.warning()
                } else {
                    row_style
                };

                let mut spans = vec![
                    Span::styled(warning_indicator, warning_style),
                    Span::styled(format!("{:>4} ", event.id.as_u64()), row_style),
                    Span::styled(
                        format!("{:02}:{:02}:{:02}.{:03} ", h, m, s, millis),
                        row_style,
                    ),
                    Span::styled(pad_to_width(&src, col_widths.src as usize), row_style),
                ];

                if col_widths.dst > 0 {
                    spans.push(Span::styled(
                        pad_to_width(&dst, col_widths.dst as usize),
                        row_style,
                    ));
                }

                spans.extend(vec![
                    Span::styled(
                        pad_to_width(&format!("{}", event.transport), col_widths.proto as usize),
                        transport_style,
                    ),
                    Span::styled(pad_to_width(dir_sym, col_widths.dir as usize), row_style),
                    Span::styled(
                        truncate_str(&summary, col_widths.summary as usize),
                        row_style,
                    ),
                ]);

                let line = Line::from(spans);
                buf.set_line(inner.x, y, &line, inner.width);
            }
        }

        // Scrollbar
        if total > vis_height {
            let mut scrollbar_state = ScrollbarState::new(total)
                .position(scroll_offset_start)
                .viewport_content_length(vis_height);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
            scrollbar.render(inner, buf, &mut scrollbar_state);
        }
    }
}

fn format_header(col_widths: ColumnWidths, sort_column: SortColumn, reversed: bool, theme: &ThemeConfig) -> Line<'static> {
    let style = theme.header();
    let sort_indicator = if reversed { "v" } else { "^" };

    let id_text = if sort_column == SortColumn::Id {
        pad_to_width(&format!("#{}", sort_indicator), col_widths.id as usize)
    } else {
        pad_to_width("#", col_widths.id as usize)
    };

    let time_text = if sort_column == SortColumn::Time {
        pad_to_width(&format!("Time{}", sort_indicator), col_widths.time as usize)
    } else {
        pad_to_width("Time", col_widths.time as usize)
    };

    let src_label = if col_widths.dst == 0 { "Origin" } else { "Source" };
    let src_text = if sort_column == SortColumn::Source {
        pad_to_width(&format!("{}{}", src_label, sort_indicator), col_widths.src as usize)
    } else {
        pad_to_width(src_label, col_widths.src as usize)
    };

    let proto_text = if sort_column == SortColumn::Protocol {
        pad_to_width(&format!("Protocol{}", sort_indicator), col_widths.proto as usize)
    } else {
        pad_to_width("Protocol", col_widths.proto as usize)
    };

    let dir_text = if sort_column == SortColumn::Dir {
        pad_to_width(&format!("Dir{}", sort_indicator), col_widths.dir as usize)
    } else {
        pad_to_width("Dir", col_widths.dir as usize)
    };

    let mut spans = vec![
        Span::styled(id_text, style),
        Span::styled(time_text, style),
        Span::styled(src_text, style),
    ];

    if col_widths.dst > 0 {
        let dst_text = if sort_column == SortColumn::Dest {
            pad_to_width(&format!("Destination{}", sort_indicator), col_widths.dst as usize)
        } else {
            pad_to_width("Destination", col_widths.dst as usize)
        };
        spans.push(Span::styled(dst_text, style));
    }

    spans.extend(vec![
        Span::styled(proto_text, style),
        Span::styled(dir_text, style),
        Span::styled(pad_to_width("Summary", col_widths.summary as usize), style),
    ]);

    Line::from(spans)
}

fn truncate_str(s: &str, max_width: usize) -> String {
    if UnicodeWidthStr::width(s) <= max_width {
        return s.to_string();
    }
    if max_width <= 3 {
        return ".".repeat(max_width);
    }
    let target = max_width - 3;
    let mut result = String::new();
    let mut width = 0;
    for c in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if width + cw > target {
            break;
        }
        result.push(c);
        width += cw;
    }
    result.push_str("...");
    result
}

/// Pad a string to exactly `width` display cells, truncating or right-padding as needed.
fn pad_to_width(s: &str, width: usize) -> String {
    let display_w = UnicodeWidthStr::width(s);
    if display_w >= width {
        truncate_str(s, width)
    } else {
        format!("{}{}", s, " ".repeat(width - display_w))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_store::EventStore;
    use bytes::Bytes;
    use prb_core::{
        DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind,
    };
    use std::collections::BTreeMap;

    fn make_event(
        id: u64,
        ts_ns: u64,
        transport: TransportKind,
        direction: Direction,
        src: &str,
        dst: &str,
    ) -> DebugEvent {
        DebugEvent {
            id: EventId::from_raw(id),
            timestamp: Timestamp::from_nanos(ts_ns),
            source: EventSource {
                adapter: "test".into(),
                origin: "test".into(),
                network: Some(NetworkAddr {
                    src: src.to_string(),
                    dst: dst.to_string(),
                }),
            },
            transport,
            direction,
            payload: Payload::Raw {
                raw: Bytes::new(),
            },
            metadata: BTreeMap::new(),
            correlation_keys: vec![],
            sequence: None,
            warnings: vec![],
        }
    }

    fn make_app_state(events: Vec<DebugEvent>) -> AppState {
        let store = EventStore::new(events);
        let filtered_indices = store.all_indices();
        AppState {
            filtered_indices,
            selected_event: if store.is_empty() { None } else { Some(0) },
            filter: None,
            filter_text: String::new(),
            store,
            schema_registry: None,
            conversations: None,
            visible_columns: Vec::new(),
        }
    }

    #[test]
    fn test_virtual_scroll_windowing() {
        let events: Vec<_> = (0..100)
            .map(|i| {
                make_event(
                    i,
                    1000 * i,
                    TransportKind::Grpc,
                    Direction::Inbound,
                    "10.0.0.1:8080",
                    "10.0.0.2:9090",
                )
            })
            .collect();

        let state = make_app_state(events);
        let mut pane = EventListPane::new();

        // Initially at top
        assert_eq!(pane.scroll_offset, 0);
        assert_eq!(pane.selected, 0);

        // Test that sorted_indices returns the correct count
        let sorted = pane.sorted_indices(&state);
        assert_eq!(sorted.len(), 100);

        // Test that all indices are valid
        for idx in sorted {
            assert!(*idx < 100);
            assert!(state.store.get(*idx).is_some());
        }
    }

    #[test]
    fn test_sort_by_time() {
        let events = vec![
            make_event(
                2,
                3000,
                TransportKind::Grpc,
                Direction::Inbound,
                "10.0.0.1:8080",
                "10.0.0.2:9090",
            ),
            make_event(
                1,
                1000,
                TransportKind::Grpc,
                Direction::Inbound,
                "10.0.0.1:8080",
                "10.0.0.2:9090",
            ),
            make_event(
                3,
                2000,
                TransportKind::Grpc,
                Direction::Inbound,
                "10.0.0.1:8080",
                "10.0.0.2:9090",
            ),
        ];

        let state = make_app_state(events);
        let mut pane = EventListPane::new();
        pane.sort_column = SortColumn::Time;
        pane.sort_reversed = false;

        let sorted = pane.sorted_indices(&state);
        assert_eq!(sorted.len(), 3);

        // Should be sorted by timestamp: 1000, 2000, 3000
        assert_eq!(state.store.get(sorted[0]).unwrap().timestamp.as_nanos(), 1000);
        assert_eq!(state.store.get(sorted[1]).unwrap().timestamp.as_nanos(), 2000);
        assert_eq!(state.store.get(sorted[2]).unwrap().timestamp.as_nanos(), 3000);
    }

    #[test]
    fn test_sort_reversed() {
        let events = vec![
            make_event(
                1,
                1000,
                TransportKind::Grpc,
                Direction::Inbound,
                "10.0.0.1:8080",
                "10.0.0.2:9090",
            ),
            make_event(
                2,
                2000,
                TransportKind::Grpc,
                Direction::Inbound,
                "10.0.0.1:8080",
                "10.0.0.2:9090",
            ),
            make_event(
                3,
                3000,
                TransportKind::Grpc,
                Direction::Inbound,
                "10.0.0.1:8080",
                "10.0.0.2:9090",
            ),
        ];

        let state = make_app_state(events);
        let mut pane = EventListPane::new();
        pane.sort_column = SortColumn::Time;
        pane.sort_reversed = true;

        let sorted = pane.sorted_indices(&state);

        // Should be sorted by timestamp descending: 3000, 2000, 1000
        assert_eq!(state.store.get(sorted[0]).unwrap().timestamp.as_nanos(), 3000);
        assert_eq!(state.store.get(sorted[1]).unwrap().timestamp.as_nanos(), 2000);
        assert_eq!(state.store.get(sorted[2]).unwrap().timestamp.as_nanos(), 1000);
    }

    #[test]
    fn test_sort_by_protocol() {
        let events = vec![
            make_event(
                1,
                1000,
                TransportKind::Zmq,
                Direction::Inbound,
                "10.0.0.1:8080",
                "10.0.0.2:9090",
            ),
            make_event(
                2,
                2000,
                TransportKind::Grpc,
                Direction::Inbound,
                "10.0.0.1:8080",
                "10.0.0.2:9090",
            ),
            make_event(
                3,
                3000,
                TransportKind::DdsRtps,
                Direction::Inbound,
                "10.0.0.1:8080",
                "10.0.0.2:9090",
            ),
        ];

        let state = make_app_state(events);
        let mut pane = EventListPane::new();
        pane.sort_column = SortColumn::Protocol;
        pane.sort_reversed = false;

        let sorted = pane.sorted_indices(&state);

        // Transport enum order: Grpc < Zmq < DdsRtps (declaration order in enum)
        assert_eq!(state.store.get(sorted[0]).unwrap().transport, TransportKind::Grpc);
        assert_eq!(state.store.get(sorted[1]).unwrap().transport, TransportKind::Zmq);
        assert_eq!(state.store.get(sorted[2]).unwrap().transport, TransportKind::DdsRtps);
    }

    #[test]
    fn test_sort_by_source() {
        let events = vec![
            make_event(
                1,
                1000,
                TransportKind::Grpc,
                Direction::Inbound,
                "192.168.1.3:8080",
                "10.0.0.2:9090",
            ),
            make_event(
                2,
                2000,
                TransportKind::Grpc,
                Direction::Inbound,
                "192.168.1.1:8080",
                "10.0.0.2:9090",
            ),
            make_event(
                3,
                3000,
                TransportKind::Grpc,
                Direction::Inbound,
                "192.168.1.2:8080",
                "10.0.0.2:9090",
            ),
        ];

        let state = make_app_state(events);
        let mut pane = EventListPane::new();
        pane.sort_column = SortColumn::Source;
        pane.sort_reversed = false;

        let sorted = pane.sorted_indices(&state);

        // Should be sorted lexicographically
        let src0 = state.store.get(sorted[0]).unwrap().source.network.as_ref().unwrap().src.as_str();
        let src1 = state.store.get(sorted[1]).unwrap().source.network.as_ref().unwrap().src.as_str();
        let src2 = state.store.get(sorted[2]).unwrap().source.network.as_ref().unwrap().src.as_str();

        assert_eq!(src0, "192.168.1.1:8080");
        assert_eq!(src1, "192.168.1.2:8080");
        assert_eq!(src2, "192.168.1.3:8080");
    }

    #[test]
    fn test_sort_by_direction() {
        let events = vec![
            make_event(
                1,
                1000,
                TransportKind::Grpc,
                Direction::Outbound,
                "10.0.0.1:8080",
                "10.0.0.2:9090",
            ),
            make_event(
                2,
                2000,
                TransportKind::Grpc,
                Direction::Inbound,
                "10.0.0.1:8080",
                "10.0.0.2:9090",
            ),
            make_event(
                3,
                3000,
                TransportKind::Grpc,
                Direction::Outbound,
                "10.0.0.1:8080",
                "10.0.0.2:9090",
            ),
        ];

        let state = make_app_state(events);
        let mut pane = EventListPane::new();
        pane.sort_column = SortColumn::Dir;
        pane.sort_reversed = false;

        let sorted = pane.sorted_indices(&state);

        // Inbound should come first
        assert_eq!(state.store.get(sorted[0]).unwrap().direction, Direction::Inbound);
        assert_eq!(state.store.get(sorted[1]).unwrap().direction, Direction::Outbound);
        assert_eq!(state.store.get(sorted[2]).unwrap().direction, Direction::Outbound);
    }

    #[test]
    fn test_sort_cycle() {
        let pane = EventListPane::new();
        assert_eq!(pane.sort_column, SortColumn::Time);

        // Simulate 's' key press through cycle
        let mut col = pane.sort_column;
        col = col.next();
        assert_eq!(col, SortColumn::Source);

        col = col.next();
        assert_eq!(col, SortColumn::Dest);

        col = col.next();
        assert_eq!(col, SortColumn::Protocol);

        col = col.next();
        assert_eq!(col, SortColumn::Dir);

        col = col.next();
        assert_eq!(col, SortColumn::Id);

        col = col.next();
        assert_eq!(col, SortColumn::Time);
    }

    #[test]
    fn test_navigation_keys() {
        let events: Vec<_> = (0..10)
            .map(|i| {
                make_event(
                    i,
                    1000 * i,
                    TransportKind::Grpc,
                    Direction::Inbound,
                    "10.0.0.1:8080",
                    "10.0.0.2:9090",
                )
            })
            .collect();

        let _state = make_app_state(events.clone());
        let state = make_app_state(events);
        let mut pane = EventListPane::new();

        // Test 'j' (down)
        let key = KeyEvent::new(KeyCode::Char('j'), crossterm::event::KeyModifiers::NONE);
        pane.handle_key(key, &state);
        assert_eq!(pane.selected, 1);

        // Test 'k' (up)
        let key = KeyEvent::new(KeyCode::Char('k'), crossterm::event::KeyModifiers::NONE);
        pane.handle_key(key, &state);
        assert_eq!(pane.selected, 0);

        // Test 'G' (end)
        let key = KeyEvent::new(KeyCode::Char('G'), crossterm::event::KeyModifiers::NONE);
        pane.handle_key(key, &state);
        assert_eq!(pane.selected, 9);

        // Test 'g' (home)
        let key = KeyEvent::new(KeyCode::Char('g'), crossterm::event::KeyModifiers::NONE);
        pane.handle_key(key, &state);
        assert_eq!(pane.selected, 0);
    }

    #[test]
    fn test_filter_application() {
        use prb_query::Filter;

        let events = vec![
            make_event(
                1,
                1000,
                TransportKind::Grpc,
                Direction::Inbound,
                "10.0.0.1:8080",
                "10.0.0.2:9090",
            ),
            make_event(
                2,
                2000,
                TransportKind::Zmq,
                Direction::Inbound,
                "10.0.0.1:8080",
                "10.0.0.2:9090",
            ),
            make_event(
                3,
                3000,
                TransportKind::Grpc,
                Direction::Outbound,
                "10.0.0.1:8080",
                "10.0.0.2:9090",
            ),
        ];

        let store = EventStore::new(events);
        let filter = Filter::parse(r#"transport == "gRPC""#).unwrap();
        let filtered_indices = store.filter_indices(&filter);

        let state = AppState {
            filtered_indices,
            selected_event: Some(0),
            filter: Some(filter),
            filter_text: r#"transport == "gRPC""#.to_string(),
            store,
            schema_registry: None,
            conversations: None,
            visible_columns: Vec::new(),
        };

        let mut pane = EventListPane::new();
        let sorted = pane.sorted_indices(&state);

        // Should only have 2 gRPC events
        assert_eq!(sorted.len(), 2);
        assert_eq!(state.store.get(sorted[0]).unwrap().transport, TransportKind::Grpc);
        assert_eq!(state.store.get(sorted[1]).unwrap().transport, TransportKind::Grpc);
    }

    #[test]
    fn test_large_dataset_performance() {
        // Create 1000+ events for performance test
        let events: Vec<_> = (0..1500)
            .map(|i| {
                make_event(
                    i,
                    1000 * i,
                    if i % 3 == 0 {
                        TransportKind::Grpc
                    } else if i % 3 == 1 {
                        TransportKind::Zmq
                    } else {
                        TransportKind::DdsRtps
                    },
                    if i % 2 == 0 {
                        Direction::Inbound
                    } else {
                        Direction::Outbound
                    },
                    &format!("192.168.1.{}:8080", i % 256),
                    &format!("10.0.0.{}:9090", i % 256),
                )
            })
            .collect();

        let state = make_app_state(events);
        let mut pane = EventListPane::new();

        // Test sorting doesn't panic and completes
        pane.sort_column = SortColumn::Protocol;
        let sorted = pane.sorted_indices(&state);
        assert_eq!(sorted.len(), 1500);

        // Test that all indices are valid
        for idx in sorted.iter().take(100) {
            assert!(*idx < 1500);
            assert!(state.store.get(*idx).is_some());
        }
    }
}
