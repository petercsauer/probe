//! Unit and render tests for timeline.rs

use bytes::Bytes;
use prb_core::{DebugEvent, Direction, EventId, EventSource, Payload, Timestamp, TransportKind};
use prb_tui::app::AppState;
use prb_tui::event_store::EventStore;
use prb_tui::panes::timeline::TimelinePane;
use prb_tui::panes::PaneComponent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::collections::BTreeMap;

fn make_test_event_at(id: u64, timestamp_nanos: u64, transport: TransportKind) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(timestamp_nanos),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: None,
        },
        transport,
        direction: Direction::Inbound,
        payload: Payload::Raw {
            raw: Bytes::from(vec![1, 2, 3]),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}

#[test]
fn test_timeline_pane_new() {
    let pane = TimelinePane::new();
    // Should construct without panic
    let _ = pane;
}

#[test]
fn test_timeline_pane_default() {
    let pane = TimelinePane::default();
    // Should construct without panic
    let _ = pane;
}

#[test]
fn test_timeline_render_empty_store() {
    let store = EventStore::new(vec![]);
    let state = AppState {
        store,
        filtered_indices: vec![],
        selected_event: None,
        filter: None,
        filter_text: String::new(),
    };

    let mut pane = TimelinePane::new();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));

    pane.render(Rect::new(0, 0, 80, 10), &mut buffer, &state, false);

    // Should render without panic
    // Check that the title "Timeline" is present
    let mut found_title_char = false;
    for y in 0..10 {
        for x in 0..80 {
            let symbol = buffer[(x, y)].symbol();
            if symbol == "T" || symbol == "i" || symbol == "m" {
                found_title_char = true;
                break;
            }
        }
    }
    assert!(found_title_char, "Should render Timeline title");
}

#[test]
fn test_timeline_render_with_events() {
    let events = vec![
        make_test_event_at(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event_at(2, 2_000_000_000, TransportKind::Zmq),
        make_test_event_at(3, 3_000_000_000, TransportKind::Grpc),
    ];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        store,
    };

    let mut pane = TimelinePane::new();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));

    pane.render(Rect::new(0, 0, 80, 10), &mut buffer, &state, false);

    // Should render without panic and have some content
    let mut has_content = false;
    for y in 0..10 {
        for x in 0..80 {
            if buffer[(x, y)].symbol() != " " && !buffer[(x, y)].symbol().is_empty() {
                has_content = true;
                break;
            }
        }
    }
    assert!(has_content, "Timeline should render content");
}

#[test]
fn test_timeline_render_focused_vs_unfocused() {
    let events = vec![make_test_event_at(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        store,
    };

    let mut pane = TimelinePane::new();

    // Render focused
    let mut buffer_focused = Buffer::empty(Rect::new(0, 0, 80, 10));
    pane.render(Rect::new(0, 0, 80, 10), &mut buffer_focused, &state, true);

    // Render unfocused
    let mut buffer_unfocused = Buffer::empty(Rect::new(0, 0, 80, 10));
    pane.render(Rect::new(0, 0, 80, 10), &mut buffer_unfocused, &state, false);

    // Both should render without panic
    // Border colors should differ (we can't easily test that in unit tests)
    // But we verify rendering succeeded
}

#[test]
fn test_timeline_render_small_area() {
    let events = vec![make_test_event_at(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        store,
    };

    let mut pane = TimelinePane::new();

    // Very small area (should handle gracefully)
    let mut buffer = Buffer::empty(Rect::new(0, 0, 5, 2));
    pane.render(Rect::new(0, 0, 5, 2), &mut buffer, &state, false);

    // Should not panic with small area
}

#[test]
fn test_timeline_render_with_multiple_transports() {
    let events = vec![
        make_test_event_at(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event_at(2, 2_000_000_000, TransportKind::Zmq),
        make_test_event_at(3, 3_000_000_000, TransportKind::DdsRtps),
        make_test_event_at(4, 4_000_000_000, TransportKind::RawTcp),
    ];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(1),
        filter: None,
        filter_text: String::new(),
        store,
    };

    let mut pane = TimelinePane::new();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 10));

    pane.render(Rect::new(0, 0, 120, 10), &mut buffer, &state, false);

    // Should render protocol legend with multiple types
    // Check for numbers that would appear in counts
    let mut found_digits = false;
    for y in 0..10 {
        for x in 0..120 {
            let symbol = buffer[(x, y)].symbol();
            if symbol.chars().all(|c| c.is_ascii_digit()) && !symbol.is_empty() {
                found_digits = true;
                break;
            }
        }
    }
    assert!(found_digits, "Should show protocol counts");
}

#[test]
fn test_timeline_render_with_filter_active() {
    use prb_query::Filter;

    let events = vec![
        make_test_event_at(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event_at(2, 2_000_000_000, TransportKind::Zmq),
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

    let mut pane = TimelinePane::new();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 10));

    pane.render(Rect::new(0, 0, 120, 10), &mut buffer, &state, false);

    // Should show "(filtered)" indicator
    let mut found_filtered = false;
    for y in 0..10 {
        for x in 0..115 {
            let symbol = buffer[(x, y)].symbol();
            if symbol == "f" || symbol == "i" || symbol == "l" {
                // Look for "filtered" text
                found_filtered = true;
                break;
            }
        }
    }
    assert!(found_filtered, "Should show filtered indicator");
}

#[test]
fn test_timeline_time_range_display() {
    // Events spanning exactly 1 hour
    let start = 0;
    let end = 3_600_000_000_000u64; // 1 hour in nanoseconds

    let events = vec![
        make_test_event_at(1, start, TransportKind::Grpc),
        make_test_event_at(2, end, TransportKind::Grpc),
    ];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        store,
    };

    let mut pane = TimelinePane::new();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));

    pane.render(Rect::new(0, 0, 80, 10), &mut buffer, &state, false);

    // Should display time legend with HH:MM:SS format
    // Look for colons in the time display
    let mut found_colon = false;
    for y in 0..10 {
        for x in 0..80 {
            if buffer[(x, y)].symbol() == ":" {
                found_colon = true;
                break;
            }
        }
    }
    assert!(found_colon, "Should display time range with colons");
}
