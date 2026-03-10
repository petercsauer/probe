//! Additional tests for decode_tree.rs to improve coverage

use bytes::Bytes;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prb_core::{DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind};
use prb_tui::app::AppState;
use prb_tui::event_store::EventStore;
use prb_tui::panes::decode_tree::DecodeTreePane;
use prb_tui::panes::PaneComponent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::collections::BTreeMap;

fn make_event_with_metadata(id: u64, transport: TransportKind, metadata: BTreeMap<String, String>) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(1_000_000_000),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: Some(NetworkAddr {
                src: "10.0.0.1:1234".to_string(),
                dst: "10.0.0.2:5678".to_string(),
            }),
        },
        transport,
        direction: Direction::Inbound,
        payload: Payload::Raw {
            raw: Bytes::from(vec![1, 2, 3, 4, 5]),
        },
        metadata,
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}

#[test]
fn test_decode_tree_render_no_selection() {
    let events = vec![make_event_with_metadata(1, TransportKind::Grpc, BTreeMap::new())];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: None, // No selection
        filter: None,
        filter_text: String::new(),
        store,
    };

    let mut pane = DecodeTreePane::new();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 20));

    pane.render(Rect::new(0, 0, 80, 20), &mut buffer, &state, false);

    // Should render "No event selected" message
    let mut found_message = false;
    for y in 0..20 {
        for x in 0..80 {
            if buffer[(x, y)].symbol() == "N" || buffer[(x, y)].symbol() == "o" {
                found_message = true;
                break;
            }
        }
    }
    assert!(found_message || buffer[(0, 0)].symbol() != " ", "Should show message or border");
}

#[test]
fn test_decode_tree_render_with_warnings() {
    let mut metadata = BTreeMap::new();
    metadata.insert("key".to_string(), "value".to_string());

    let mut event = make_event_with_metadata(1, TransportKind::Grpc, metadata);
    event.warnings.push("Test warning 1".to_string());
    event.warnings.push("Test warning 2".to_string());

    let events = vec![event];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        store,
    };

    let mut pane = DecodeTreePane::new();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 30));

    pane.render(Rect::new(0, 0, 80, 30), &mut buffer, &state, false);

    // Should render warnings section
}

#[test]
fn test_decode_tree_render_all_transport_types() {
    let transports = vec![
        TransportKind::Grpc,
        TransportKind::Zmq,
        TransportKind::DdsRtps,
        TransportKind::RawTcp,
        TransportKind::RawUdp,
        TransportKind::JsonFixture,
    ];

    for transport in transports {
        let events = vec![make_event_with_metadata(1, transport, BTreeMap::new())];
        let store = EventStore::new(events);
        let state = AppState {
            filtered_indices: store.all_indices(),
            selected_event: Some(0),
            filter: None,
            filter_text: String::new(),
            store,
        };

        let mut pane = DecodeTreePane::new();
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 30));

        // Should render without panic for each transport type
        pane.render(Rect::new(0, 0, 80, 30), &mut buffer, &state, false);
    }
}

#[test]
fn test_decode_tree_handle_key_space() {
    let events = vec![make_event_with_metadata(1, TransportKind::Grpc, BTreeMap::new())];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        store,
    };

    let mut pane = DecodeTreePane::new();

    // Press Space to toggle node
    let key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
    pane.handle_key(key, &state);

    // Should not panic
}

#[test]
fn test_decode_tree_handle_key_arrows() {
    let events = vec![make_event_with_metadata(1, TransportKind::Grpc, BTreeMap::new())];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        store,
    };

    let mut pane = DecodeTreePane::new();

    // Test arrow keys
    let keys = vec![
        KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
    ];

    for key in keys {
        pane.handle_key(key, &state);
    }

    // Should not panic
}

#[test]
fn test_decode_tree_render_small_area() {
    let events = vec![make_event_with_metadata(1, TransportKind::Grpc, BTreeMap::new())];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        store,
    };

    let mut pane = DecodeTreePane::new();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 5));

    // Should handle very small area gracefully
    pane.render(Rect::new(0, 0, 20, 5), &mut buffer, &state, false);
}

#[test]
fn test_decode_tree_render_focused_vs_unfocused() {
    let events = vec![make_event_with_metadata(1, TransportKind::Grpc, BTreeMap::new())];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        store,
    };

    let mut pane = DecodeTreePane::new();

    // Render focused
    let mut buffer_focused = Buffer::empty(Rect::new(0, 0, 80, 20));
    pane.render(Rect::new(0, 0, 80, 20), &mut buffer_focused, &state, true);

    // Render unfocused
    let mut buffer_unfocused = Buffer::empty(Rect::new(0, 0, 80, 20));
    pane.render(Rect::new(0, 0, 80, 20), &mut buffer_unfocused, &state, false);

    // Both should render without panic
}

#[test]
fn test_decode_tree_with_correlation_keys() {
    let mut event = make_event_with_metadata(1, TransportKind::Grpc, BTreeMap::new());
    event.correlation_keys = vec![
        prb_core::CorrelationKey::StreamId { id: 123 },
        prb_core::CorrelationKey::Topic { name: "test-topic".to_string() },
    ];

    let events = vec![event];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        store,
    };

    let mut pane = DecodeTreePane::new();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 30));

    pane.render(Rect::new(0, 0, 80, 30), &mut buffer, &state, false);

    // Should render correlation keys section
}

#[test]
fn test_decode_tree_with_sequence() {
    let mut event = make_event_with_metadata(1, TransportKind::Grpc, BTreeMap::new());
    event.sequence = Some(42);

    let events = vec![event];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        store,
    };

    let mut pane = DecodeTreePane::new();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 30));

    pane.render(Rect::new(0, 0, 80, 30), &mut buffer, &state, false);

    // Should render sequence number
}

#[test]
fn test_decode_tree_handle_key_enter() {
    let events = vec![make_event_with_metadata(1, TransportKind::Grpc, BTreeMap::new())];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        store,
    };

    let mut pane = DecodeTreePane::new();

    // Press Enter to expand/collapse
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    pane.handle_key(key, &state);

    // Should not panic
}

#[test]
fn test_decode_tree_handle_key_backspace() {
    let events = vec![make_event_with_metadata(1, TransportKind::Grpc, BTreeMap::new())];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        store,
    };

    let mut pane = DecodeTreePane::new();

    // Press Backspace to collapse
    let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
    pane.handle_key(key, &state);

    // Should not panic
}

#[test]
fn test_decode_tree_with_many_metadata_keys() {
    let mut metadata = BTreeMap::new();
    for i in 0..50 {
        metadata.insert(format!("key{}", i), format!("value{}", i));
    }

    let events = vec![make_event_with_metadata(1, TransportKind::Grpc, metadata)];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        store,
    };

    let mut pane = DecodeTreePane::new();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 30));

    // Should handle many metadata entries without panic
    pane.render(Rect::new(0, 0, 80, 30), &mut buffer, &state, false);
}

#[test]
fn test_decode_tree_with_large_payload() {
    let mut event = make_event_with_metadata(1, TransportKind::Grpc, BTreeMap::new());
    event.payload = Payload::Raw {
        raw: Bytes::from(vec![0x42; 10000]), // 10KB payload
    };

    let events = vec![event];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        store,
    };

    let mut pane = DecodeTreePane::new();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 30));

    // Should handle large payload without panic
    pane.render(Rect::new(0, 0, 80, 30), &mut buffer, &state, false);
}
