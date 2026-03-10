use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget,
};

use crate::app::AppState;
use crate::panes::{Action, PaneComponent};
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Id,
    Time,
    Source,
    Dest,
    Protocol,
    Dir,
}

impl SortColumn {
    fn next(self) -> Self {
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
}

impl Default for EventListPane {
    fn default() -> Self {
        Self::new()
    }
}

impl EventListPane {
    pub fn new() -> Self {
        EventListPane {
            selected: 0,
            scroll_offset: 0,
            sort_column: SortColumn::Time,
            sort_reversed: false,
        }
    }

    #[allow(dead_code)]
    #[cfg(test)]
    pub fn ensure_visible(&mut self, area_height: u16) {
        let vis = area_height.saturating_sub(3) as usize;
        if vis == 0 {
            return;
        }
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + vis {
            self.scroll_offset = self.selected.saturating_sub(vis - 1);
        }
    }

    fn total_items(state: &AppState) -> usize {
        state.filtered_indices.len()
    }

    fn sorted_indices(&self, state: &AppState) -> Vec<usize> {
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

        indices
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

    fn render(&mut self, area: Rect, buf: &mut Buffer, state: &AppState, focused: bool) {
        let border_style = if focused {
            Theme::focused_border()
        } else {
            Theme::unfocused_border()
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Events ");

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 2 || inner.width < 10 {
            return;
        }

        // Header row
        let header = format_header(inner.width, self.sort_column, self.sort_reversed);
        buf.set_line(inner.x, inner.y, &header, inner.width);

        let vis_height = inner.height.saturating_sub(1) as usize;
        let sorted = self.sorted_indices(state);
        let total = sorted.len();

        // Clone to allow scrolling adjustment
        let scroll_offset = {
            let mut off = self.scroll_offset;
            if self.selected < off {
                off = self.selected;
            } else if self.selected >= off + vis_height {
                off = self.selected.saturating_sub(vis_height - 1);
            }
            off
        };

        for i in 0..vis_height {
            let idx = scroll_offset + i;
            if idx >= total {
                break;
            }
            let event_idx = sorted[idx];
            if let Some(event) = state.store.get(event_idx) {
                let y = inner.y + 1 + i as u16;
                let is_selected = idx == self.selected;

                let row_style = if is_selected {
                    Theme::selected_row()
                } else {
                    Style::default()
                };

                let transport_color = Theme::transport_color(event.transport);
                let dir_sym = Theme::direction_symbol(event.direction);

                let ts_ns = event.timestamp.as_nanos();
                let secs = ts_ns / 1_000_000_000;
                let millis = (ts_ns % 1_000_000_000) / 1_000_000;
                let h = (secs / 3600) % 24;
                let m = (secs % 3600) / 60;
                let s = secs % 60;

                let src = event
                    .source
                    .network
                    .as_ref()
                    .map(|n| n.src.as_str())
                    .unwrap_or("-");
                let dst = event
                    .source
                    .network
                    .as_ref()
                    .map(|n| n.dst.as_str())
                    .unwrap_or("-");

                let summary = event
                    .metadata
                    .values()
                    .next()
                    .cloned()
                    .unwrap_or_default();

                let w = inner.width as usize;
                let fixed_cols = 6 + 1 + 13 + 1 + 18 + 1 + 18 + 1 + 10 + 1 + 3 + 1;
                let summary_w = w.saturating_sub(fixed_cols);

                let truncated_src = truncate_str(src, 18);
                let truncated_dst = truncate_str(dst, 18);
                let truncated_summary = truncate_str(&summary, summary_w);

                let line = Line::from(vec![
                    Span::styled(format!("{:>5} ", event.id.as_u64()), row_style),
                    Span::styled(
                        format!("{:02}:{:02}:{:02}.{:03} ", h, m, s, millis),
                        row_style,
                    ),
                    Span::styled(format!("{:<18} ", truncated_src), row_style),
                    Span::styled(format!("{:<18} ", truncated_dst), row_style),
                    Span::styled(
                        format!("{:<10} ", event.transport),
                        row_style.fg(transport_color),
                    ),
                    Span::styled(format!("{:<3} ", dir_sym), row_style),
                    Span::styled(truncated_summary, row_style),
                ]);

                buf.set_line(inner.x, y, &line, inner.width);
            }
        }

        // Scrollbar
        if total > vis_height {
            let mut scrollbar_state = ScrollbarState::new(total)
                .position(scroll_offset)
                .viewport_content_length(vis_height);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
            scrollbar.render(inner, buf, &mut scrollbar_state);
        }
    }
}

fn format_header(width: u16, sort_column: SortColumn, reversed: bool) -> Line<'static> {
    let style = Theme::header();
    let w = width as usize;
    let summary_w = w.saturating_sub(6 + 1 + 13 + 1 + 18 + 1 + 18 + 1 + 10 + 1 + 3 + 1);

    let sort_indicator = if reversed { "↓" } else { "↑" };

    let id_text = if sort_column == SortColumn::Id {
        format!("{:>4}{} ", "#", sort_indicator)
    } else {
        format!("{:>5} ", "#")
    };

    let time_text = if sort_column == SortColumn::Time {
        format!("Time{:<9}", sort_indicator)
    } else {
        format!("{:<13}", "Time")
    };

    let src_text = if sort_column == SortColumn::Source {
        format!("Source{:<12} ", sort_indicator)
    } else {
        format!("{:<18} ", "Source")
    };

    let dst_text = if sort_column == SortColumn::Dest {
        format!("Destination{:<7} ", sort_indicator)
    } else {
        format!("{:<18} ", "Destination")
    };

    let proto_text = if sort_column == SortColumn::Protocol {
        format!("Protocol{:<2} ", sort_indicator)
    } else {
        format!("{:<10} ", "Protocol")
    };

    let dir_text = if sort_column == SortColumn::Dir {
        format!("Dir{} ", sort_indicator)
    } else {
        format!("{:<3} ", "Dir")
    };

    Line::from(vec![
        Span::styled(id_text, style),
        Span::styled(time_text, style),
        Span::styled(src_text, style),
        Span::styled(dst_text, style),
        Span::styled(proto_text, style),
        Span::styled(dir_text, style),
        Span::styled(
            format!("{:<width$}", "Summary", width = summary_w),
            style.add_modifier(Modifier::BOLD),
        ),
    ])
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max > 3 {
        format!("{}...", &s[..max - 3])
    } else {
        s[..max].to_string()
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
        let pane = EventListPane::new();

        // Initially at top
        assert_eq!(pane.scroll_offset, 0);
        assert_eq!(pane.selected, 0);

        // Test that sorted_indices returns the correct count
        let sorted = pane.sorted_indices(&state);
        assert_eq!(sorted.len(), 100);

        // Test that all indices are valid
        for idx in sorted {
            assert!(idx < 100);
            assert!(state.store.get(idx).is_some());
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
        };

        let pane = EventListPane::new();
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

        // Test virtual scrolling with large dataset
        pane.selected = 750;
        pane.ensure_visible(50);
        assert!(pane.scroll_offset <= 750);
        assert!(pane.scroll_offset + 50 > 750);
    }
}
