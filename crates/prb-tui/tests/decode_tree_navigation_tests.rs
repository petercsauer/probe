//! Tests for decode tree navigation, expansion, and value operations (S11).

use bytes::Bytes;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prb_core::{
    CorrelationKey, DebugEvent, Direction, EventId, EventSource, METADATA_KEY_GRPC_METHOD,
    NetworkAddr, Payload, Timestamp, TransportKind,
};
use prb_tui::app::AppState;
use prb_tui::event_store::EventStore;
use prb_tui::panes::decode_tree::DecodeTreePane;
use prb_tui::panes::{Action, PaneComponent};
use prb_tui::theme::ThemeConfig;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::collections::BTreeMap;

fn make_event_with_fields(
    id: u64,
    transport: TransportKind,
    metadata: BTreeMap<String, String>,
    payload: Payload,
) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(1_000_000_000),
        source: EventSource {
            adapter: "test".into(),
            origin: "test.pcap".into(),
            network: Some(NetworkAddr {
                src: "10.0.0.1:12345".to_string(),
                dst: "10.0.0.2:50051".to_string(),
            }),
        },
        transport,
        direction: Direction::Outbound,
        payload,
        metadata,
        correlation_keys: vec![
            CorrelationKey::StreamId { id: 1 },
            CorrelationKey::ConnectionId {
                id: "10.0.0.1:12345->10.0.0.2:50051".to_string(),
            },
        ],
        sequence: Some(5),
        warnings: vec![],
    }
}

fn make_app_state(events: Vec<DebugEvent>, selected_event: Option<usize>) -> AppState {
    let store = EventStore::new(events);
    AppState {
        filtered_indices: store.all_indices(),
        selected_event,
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
        conversations: None,
        store,
        visible_columns: Vec::new(),
    }
}

#[test]
fn test_expand_all_key() {
    let mut metadata = BTreeMap::new();
    metadata.insert(
        METADATA_KEY_GRPC_METHOD.to_string(),
        "/api.v1.Users/GetUser".to_string(),
    );
    metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let payload = Payload::Decoded {
        raw: Bytes::from(vec![1, 2, 3, 4, 5]),
        fields: serde_json::json!({
            "user_id": "abc-123",
            "request": {
                "fields": ["name", "email"]
            }
        }),
        schema_name: Some("api.v1.GetUserRequest".to_string()),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, metadata, payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = DecodeTreePane::new();

    // Press 'E' to expand all
    let action = pane.handle_key(
        KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT),
        &state,
    );

    assert!(matches!(action, Action::None));
    // Tree state should now have all nodes expanded (internal state changed)
}

#[test]
fn test_collapse_all_key() {
    let mut metadata = BTreeMap::new();
    metadata.insert("key".to_string(), "value".to_string());

    let payload = Payload::Raw {
        raw: Bytes::from(vec![1, 2, 3]),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, metadata, payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = DecodeTreePane::new();

    // Press 'C' to collapse all
    let action = pane.handle_key(
        KeyEvent::new(KeyCode::Char('C'), KeyModifiers::SHIFT),
        &state,
    );

    assert!(matches!(action, Action::None));
    // Tree state should now have all nodes collapsed
}

#[test]
fn test_copy_selected_value_key() {
    let mut metadata = BTreeMap::new();
    metadata.insert("key".to_string(), "value".to_string());

    let payload = Payload::Raw {
        raw: Bytes::from(vec![1, 2, 3]),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, metadata, payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = DecodeTreePane::new();

    // First render to build tree
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 30));
    pane.render(
        Rect::new(0, 0, 80, 30),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Navigate down to select a node
    pane.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &state);

    // Press 'y' to copy
    let action = pane.handle_key(
        KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
        &state,
    );

    assert!(matches!(action, Action::None));
    // OSC 52 would be sent to clipboard in real terminal
}

#[test]
fn test_mark_event_for_diff() {
    let payload = Payload::Raw {
        raw: Bytes::from(vec![1, 2, 3]),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, BTreeMap::new(), payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = DecodeTreePane::new();

    // Press 'm' to mark event
    let action = pane.handle_key(
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
        &state,
    );

    assert!(matches!(action, Action::None));
    // marked_event_idx should now be set internally
}

#[test]
fn test_show_diff_overlay() {
    let payload1 = Payload::Raw {
        raw: Bytes::from(vec![1, 2, 3]),
    };
    let payload2 = Payload::Raw {
        raw: Bytes::from(vec![4, 5, 6]),
    };

    let event1 = make_event_with_fields(1, TransportKind::Grpc, BTreeMap::new(), payload1);
    let event2 = make_event_with_fields(2, TransportKind::Grpc, BTreeMap::new(), payload2);

    let state = make_app_state(vec![event1, event2], Some(0));

    let mut pane = DecodeTreePane::new();

    // Mark first event
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
        &state,
    );

    // Select second event manually (simulate app state change)
    let mut state2 = state;
    state2.selected_event = Some(1);

    // Press 'D' to show diff
    let action = pane.handle_key(
        KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT),
        &state2,
    );

    assert!(matches!(action, Action::None));
    // show_diff should now be true internally
}

#[test]
fn test_diff_overlay_escape_closes() {
    let payload = Payload::Raw {
        raw: Bytes::from(vec![1, 2, 3]),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, BTreeMap::new(), payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = DecodeTreePane::new();

    // Mark event
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
        &state,
    );

    // Show diff
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT),
        &state,
    );

    // Press Escape to close diff
    let action = pane.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &state);

    assert!(matches!(action, Action::None));
    // show_diff should now be false
}

#[test]
fn test_diff_overlay_uppercase_d_key_closes() {
    let payload = Payload::Raw {
        raw: Bytes::from(vec![1, 2, 3]),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, BTreeMap::new(), payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = DecodeTreePane::new();

    // Mark event
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
        &state,
    );

    // Show diff
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT),
        &state,
    );

    // Press 'D' again to close diff
    let action = pane.handle_key(
        KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT),
        &state,
    );

    assert!(matches!(action, Action::None));
}

#[test]
fn test_highlight_bytes_action() {
    let payload = Payload::Raw {
        raw: Bytes::from(vec![1, 2, 3, 4, 5]),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, BTreeMap::new(), payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = DecodeTreePane::new();

    // Press 'h' to highlight payload in hex dump
    let action = pane.handle_key(
        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
        &state,
    );

    // Should return HighlightBytes action
    match action {
        Action::HighlightBytes { offset, len } => {
            assert_eq!(offset, 0);
            assert_eq!(len, 5);
        }
        _ => panic!("Expected HighlightBytes action"),
    }
}

#[test]
fn test_highlight_bytes_decoded_payload() {
    let payload = Payload::Decoded {
        raw: Bytes::from(vec![1, 2, 3, 4, 5, 6]),
        fields: serde_json::json!({"test": "data"}),
        schema_name: Some("Test".to_string()),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, BTreeMap::new(), payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = DecodeTreePane::new();

    // Press 'h' to highlight payload
    let action = pane.handle_key(
        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
        &state,
    );

    // Should return HighlightBytes action with raw bytes length
    match action {
        Action::HighlightBytes { offset, len } => {
            assert_eq!(offset, 0);
            assert_eq!(len, 6);
        }
        _ => panic!("Expected HighlightBytes action"),
    }
}

#[test]
fn test_highlight_bytes_empty_payload() {
    let payload = Payload::Raw {
        raw: Bytes::from(vec![]),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, BTreeMap::new(), payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = DecodeTreePane::new();

    // Press 'h' on empty payload
    let action = pane.handle_key(
        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
        &state,
    );

    // Should not return HighlightBytes (empty payload)
    assert!(matches!(action, Action::None));
}

#[test]
fn test_tree_navigation_up_down() {
    let payload = Payload::Raw {
        raw: Bytes::from(vec![1, 2, 3]),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, BTreeMap::new(), payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = DecodeTreePane::new();

    // Render to build tree
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 30));
    pane.render(
        Rect::new(0, 0, 80, 30),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Navigate down
    pane.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &state);
    pane.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &state);

    // Navigate up
    pane.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), &state);

    // Should not panic
}

#[test]
fn test_tree_navigation_vim_keys() {
    let payload = Payload::Raw {
        raw: Bytes::from(vec![1, 2, 3]),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, BTreeMap::new(), payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = DecodeTreePane::new();

    // Render to build tree
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 30));
    pane.render(
        Rect::new(0, 0, 80, 30),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Navigate with j/k
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        &state,
    );
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        &state,
    );
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
        &state,
    );

    // Should not panic
}

#[test]
fn test_tree_expand_collapse_right_left() {
    let mut metadata = BTreeMap::new();
    metadata.insert("key1".to_string(), "value1".to_string());
    metadata.insert("key2".to_string(), "value2".to_string());

    let payload = Payload::Raw {
        raw: Bytes::from(vec![1, 2, 3]),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, metadata, payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = DecodeTreePane::new();

    // Render to build tree
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 30));
    pane.render(
        Rect::new(0, 0, 80, 30),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Navigate to a node with children
    pane.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &state);

    // Expand with Right arrow
    pane.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &state);

    // Collapse with Left arrow
    pane.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), &state);

    // Should not panic
}

#[test]
fn test_tree_expand_with_enter() {
    let mut metadata = BTreeMap::new();
    metadata.insert("key".to_string(), "value".to_string());

    let payload = Payload::Raw {
        raw: Bytes::from(vec![1, 2, 3]),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, metadata, payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = DecodeTreePane::new();

    // Render to build tree
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 30));
    pane.render(
        Rect::new(0, 0, 80, 30),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Navigate to a node with children
    pane.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &state);

    // Toggle with Enter
    pane.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &state);

    // Should not panic
}

#[test]
fn test_tree_collapse_with_backspace() {
    let mut metadata = BTreeMap::new();
    metadata.insert("key".to_string(), "value".to_string());

    let payload = Payload::Raw {
        raw: Bytes::from(vec![1, 2, 3]),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, metadata, payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = DecodeTreePane::new();

    // Render to build tree
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 30));
    pane.render(
        Rect::new(0, 0, 80, 30),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Navigate to a node
    pane.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &state);

    // Collapse parent with Backspace
    pane.handle_key(
        KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        &state,
    );

    // Should not panic
}

#[test]
fn test_tree_toggle_with_space() {
    let mut metadata = BTreeMap::new();
    metadata.insert("key".to_string(), "value".to_string());

    let payload = Payload::Raw {
        raw: Bytes::from(vec![1, 2, 3]),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, metadata, payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = DecodeTreePane::new();

    // Render to build tree
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 30));
    pane.render(
        Rect::new(0, 0, 80, 30),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Navigate to a node
    pane.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &state);

    // Toggle with Space
    pane.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        &state,
    );

    // Should not panic
}

#[test]
fn test_decode_tree_no_event_selected() {
    let payload = Payload::Raw {
        raw: Bytes::from(vec![1, 2, 3]),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, BTreeMap::new(), payload);
    let state = make_app_state(vec![event], None); // No selection

    let mut pane = DecodeTreePane::new();

    // Key presses should not panic when no event is selected
    pane.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &state);
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT),
        &state,
    );
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('C'), KeyModifiers::SHIFT),
        &state,
    );
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
        &state,
    );
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
        &state,
    );
}

#[test]
fn test_decode_tree_render_with_marked_event() {
    let payload = Payload::Raw {
        raw: Bytes::from(vec![1, 2, 3]),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, BTreeMap::new(), payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = DecodeTreePane::new();

    // Mark event
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
        &state,
    );

    // Render with marked event
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 30));
    pane.render(
        Rect::new(0, 0, 80, 30),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Should render title with "(marked)" indicator
    // Just verify render succeeded without panic
}

#[test]
fn test_render_diff_overlay_with_two_events() {
    let payload1 = Payload::Raw {
        raw: Bytes::from(vec![1, 2, 3]),
    };
    let payload2 = Payload::Raw {
        raw: Bytes::from(vec![4, 5, 6]),
    };

    let mut metadata1 = BTreeMap::new();
    metadata1.insert("key".to_string(), "value1".to_string());

    let mut metadata2 = BTreeMap::new();
    metadata2.insert("key".to_string(), "value2".to_string());

    let event1 = make_event_with_fields(1, TransportKind::Grpc, metadata1, payload1);
    let event2 = make_event_with_fields(2, TransportKind::Grpc, metadata2, payload2);

    let state = make_app_state(vec![event1, event2], Some(0));

    let mut pane = DecodeTreePane::new();

    // Mark first event
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
        &state,
    );

    // Change selection to second event
    let mut state2 = state;
    state2.selected_event = Some(1);

    // Show diff
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT),
        &state2,
    );

    // Render diff overlay
    let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 40));
    pane.render(
        Rect::new(0, 0, 120, 40),
        &mut buffer,
        &state2,
        &ThemeConfig::dark(),
        true,
    );

    // Should render diff overlay without panicking
}

#[test]
fn test_diff_keys_ignored_when_not_in_diff_mode() {
    let payload = Payload::Raw {
        raw: Bytes::from(vec![1, 2, 3]),
    };

    let event = make_event_with_fields(1, TransportKind::Grpc, BTreeMap::new(), payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = DecodeTreePane::new();

    // Press 'j' when not in diff mode - should navigate normally
    let action = pane.handle_key(
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        &state,
    );

    assert!(matches!(action, Action::None));
}
