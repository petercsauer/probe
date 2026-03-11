//! Additional tests for event_list.rs to improve coverage

use bytes::Bytes;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prb_core::{DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind};
use prb_tui::app::AppState;
use prb_tui::event_store::EventStore;
use prb_tui::panes::event_list::{EventListPane, SortColumn};
use prb_tui::panes::{Action, PaneComponent};
use prb_tui::theme::ThemeConfig;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::collections::BTreeMap;

fn make_test_event_with_network(
    id: u64,
    timestamp_nanos: u64,
    transport: TransportKind,
    src: &str,
    dst: &str,
    direction: Direction,
) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(timestamp_nanos),
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
            raw: Bytes::from(vec![1, 2, 3]),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}

#[test]
fn test_sort_column_next_cycle() {
    assert_eq!(SortColumn::Id.next(), SortColumn::Time);
    assert_eq!(SortColumn::Time.next(), SortColumn::Source);
    assert_eq!(SortColumn::Source.next(), SortColumn::Dest);
    assert_eq!(SortColumn::Dest.next(), SortColumn::Protocol);
    assert_eq!(SortColumn::Protocol.next(), SortColumn::Dir);
    assert_eq!(SortColumn::Dir.next(), SortColumn::Id);
}

#[test]
fn test_event_list_pane_new() {
    let pane = EventListPane::new();
    assert_eq!(pane.selected, 0);
    assert_eq!(pane.scroll_offset, 0);
    assert_eq!(pane.sort_column, SortColumn::Time);
    assert!(!pane.sort_reversed);
}

#[test]
fn test_event_list_pane_default() {
    let pane = EventListPane::default();
    assert_eq!(pane.selected, 0);
    assert_eq!(pane.scroll_offset, 0);
}

#[test]
fn test_event_list_handle_key_down() {
    let events = vec![
        make_test_event_with_network(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1", "10.0.0.2", Direction::Inbound),
        make_test_event_with_network(2, 2_000_000_000, TransportKind::Zmq, "10.0.0.2", "10.0.0.3", Direction::Outbound),
    ];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
            conversations: None,
        store,
    };

    let mut pane = EventListPane::new();
    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    let action = pane.handle_key(key, &state);

    assert_eq!(pane.selected, 1);
    assert!(matches!(action, Action::SelectEvent(1)));
}

#[test]
fn test_event_list_handle_key_up() {
    let events = vec![
        make_test_event_with_network(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1", "10.0.0.2", Direction::Inbound),
        make_test_event_with_network(2, 2_000_000_000, TransportKind::Zmq, "10.0.0.2", "10.0.0.3", Direction::Outbound),
    ];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(1),
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
            conversations: None,
        store,
    };

    let mut pane = EventListPane::new();
    pane.selected = 1;
    let key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
    let action = pane.handle_key(key, &state);

    assert_eq!(pane.selected, 0);
    assert!(matches!(action, Action::SelectEvent(0)));
}

#[test]
fn test_event_list_handle_key_vim_j_k() {
    let events = vec![
        make_test_event_with_network(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1", "10.0.0.2", Direction::Inbound),
        make_test_event_with_network(2, 2_000_000_000, TransportKind::Zmq, "10.0.0.2", "10.0.0.3", Direction::Outbound),
    ];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
            conversations: None,
        store,
    };

    let mut pane = EventListPane::new();

    // Test 'j' for down
    let key_j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    pane.handle_key(key_j, &state);
    assert_eq!(pane.selected, 1);

    // Test 'k' for up
    let key_k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
    pane.handle_key(key_k, &state);
    assert_eq!(pane.selected, 0);
}

#[test]
fn test_event_list_handle_key_home_end() {
    let events = vec![
        make_test_event_with_network(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1", "10.0.0.2", Direction::Inbound),
        make_test_event_with_network(2, 2_000_000_000, TransportKind::Zmq, "10.0.0.2", "10.0.0.3", Direction::Outbound),
        make_test_event_with_network(3, 3_000_000_000, TransportKind::DdsRtps, "10.0.0.3", "10.0.0.4", Direction::Inbound),
    ];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(1),
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
            conversations: None,
        store,
    };

    let mut pane = EventListPane::new();
    pane.selected = 1;

    // Test Home key
    let key_home = KeyEvent::new(KeyCode::Home, KeyModifiers::NONE);
    pane.handle_key(key_home, &state);
    assert_eq!(pane.selected, 0);
    assert_eq!(pane.scroll_offset, 0);

    // Test End key
    let key_end = KeyEvent::new(KeyCode::End, KeyModifiers::NONE);
    pane.handle_key(key_end, &state);
    assert_eq!(pane.selected, 2);
}

#[test]
fn test_event_list_handle_key_vim_g_g() {
    let events = vec![
        make_test_event_with_network(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1", "10.0.0.2", Direction::Inbound),
        make_test_event_with_network(2, 2_000_000_000, TransportKind::Zmq, "10.0.0.2", "10.0.0.3", Direction::Outbound),
        make_test_event_with_network(3, 3_000_000_000, TransportKind::DdsRtps, "10.0.0.3", "10.0.0.4", Direction::Inbound),
    ];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(1),
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
            conversations: None,
        store,
    };

    let mut pane = EventListPane::new();
    pane.selected = 1;

    // Test 'g' (lowercase) for home
    let key_g = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
    pane.handle_key(key_g, &state);
    assert_eq!(pane.selected, 0);

    // Test 'G' (uppercase) for end
    let key_g_upper = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE);
    pane.handle_key(key_g_upper, &state);
    assert_eq!(pane.selected, 2);
}

#[test]
fn test_event_list_handle_key_pagedown_pageup() {
    let events: Vec<_> = (0..50)
        .map(|i| make_test_event_with_network(
            i,
            i * 1_000_000_000,
            TransportKind::Grpc,
            "10.0.0.1",
            "10.0.0.2",
            Direction::Inbound,
        ))
        .collect();
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
            conversations: None,
        store,
    };

    let mut pane = EventListPane::new();

    // Test PageDown
    let key_pgdn = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
    pane.handle_key(key_pgdn, &state);
    assert_eq!(pane.selected, 20);

    // Test PageUp
    let key_pgup = KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE);
    pane.handle_key(key_pgup, &state);
    assert_eq!(pane.selected, 0);
}

#[test]
fn test_event_list_handle_key_sort_toggle() {
    let events = vec![
        make_test_event_with_network(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1", "10.0.0.2", Direction::Inbound),
    ];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
            conversations: None,
        store,
    };

    let mut pane = EventListPane::new();
    assert_eq!(pane.sort_column, SortColumn::Time);
    assert!(!pane.sort_reversed);

    // Test 's' to cycle sort column
    let key_s = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
    pane.handle_key(key_s, &state);
    assert_eq!(pane.sort_column, SortColumn::Source);

    // Test 'S' to toggle sort direction
    let key_s_upper = KeyEvent::new(KeyCode::Char('S'), KeyModifiers::NONE);
    pane.handle_key(key_s_upper, &state);
    assert!(pane.sort_reversed);

    // Toggle again
    pane.handle_key(key_s_upper, &state);
    assert!(!pane.sort_reversed);
}

#[test]
fn test_event_list_handle_key_empty_store() {
    let store = EventStore::new(vec![]);
    let state = AppState {
        filtered_indices: vec![],
        selected_event: None,
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
            conversations: None,
        store,
    };

    let mut pane = EventListPane::new();
    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    let action = pane.handle_key(key, &state);

    // Should return Action::None for empty store
    assert!(matches!(action, Action::None));
}

#[test]
fn test_event_list_handle_key_boundaries() {
    let events = vec![
        make_test_event_with_network(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1", "10.0.0.2", Direction::Inbound),
        make_test_event_with_network(2, 2_000_000_000, TransportKind::Zmq, "10.0.0.2", "10.0.0.3", Direction::Outbound),
    ];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
            conversations: None,
        store,
    };

    let mut pane = EventListPane::new();

    // At beginning, try to go up
    let key_up = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
    pane.handle_key(key_up, &state);
    assert_eq!(pane.selected, 0); // Should stay at 0

    // Go to end
    pane.selected = 1;
    // At end, try to go down
    let key_down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    pane.handle_key(key_down, &state);
    assert_eq!(pane.selected, 1); // Should stay at 1
}

#[test]
fn test_event_list_render_with_events() {
    let events = vec![
        make_test_event_with_network(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1:1234", "10.0.0.2:5678", Direction::Inbound),
        make_test_event_with_network(2, 2_000_000_000, TransportKind::Zmq, "10.0.0.2:5678", "10.0.0.3:9999", Direction::Outbound),
    ];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
            conversations: None,
        store,
    };

    let mut pane = EventListPane::new();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 20));

    pane.render(Rect::new(0, 0, 120, 20), &mut buffer, &state, &ThemeConfig::dark(), false);

    // Should render without panic and have content
    let mut has_content = false;
    for y in 0..20 {
        for x in 0..120 {
            if buffer[(x, y)].symbol() != " " && !buffer[(x, y)].symbol().is_empty() {
                has_content = true;
                break;
            }
        }
    }
    assert!(has_content, "Event list should render content");
}

#[test]
fn test_event_list_render_empty() {
    let store = EventStore::new(vec![]);
    let state = AppState {
        filtered_indices: vec![],
        selected_event: None,
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
            conversations: None,
        store,
    };

    let mut pane = EventListPane::new();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));

    pane.render(Rect::new(0, 0, 80, 10), &mut buffer, &state, &ThemeConfig::dark(), false);

    // Should render without panic
    // Check for "No events" or "Empty" message
    let mut found_message = false;
    for y in 0..10 {
        for x in 0..80 {
            let symbol = buffer[(x, y)].symbol();
            if symbol == "N" || symbol == "o" || symbol == "E" {
                found_message = true;
                break;
            }
        }
    }
    // Should show some message or at least the pane border
    assert!(found_message || buffer[(0, 0)].symbol() != " ", "Should show something");
}

#[test]
fn test_event_list_sorting_by_different_columns() {
    let events = vec![
        make_test_event_with_network(3, 3_000_000_000, TransportKind::DdsRtps, "192.168.1.3", "192.168.1.1", Direction::Inbound),
        make_test_event_with_network(1, 1_000_000_000, TransportKind::Grpc, "192.168.1.1", "192.168.1.2", Direction::Outbound),
        make_test_event_with_network(2, 2_000_000_000, TransportKind::Zmq, "192.168.1.2", "192.168.1.3", Direction::Inbound),
    ];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
            conversations: None,
        store,
    };

    let mut pane = EventListPane::new();

    // Sort by ID
    pane.sort_column = SortColumn::Id;
    let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 20));
    pane.render(Rect::new(0, 0, 120, 20), &mut buffer, &state, &ThemeConfig::dark(), false);

    // Sort by Protocol
    pane.sort_column = SortColumn::Protocol;
    let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 20));
    pane.render(Rect::new(0, 0, 120, 20), &mut buffer, &state, &ThemeConfig::dark(), false);

    // Sort by Source
    pane.sort_column = SortColumn::Source;
    let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 20));
    pane.render(Rect::new(0, 0, 120, 20), &mut buffer, &state, &ThemeConfig::dark(), false);

    // Sort by Dest
    pane.sort_column = SortColumn::Dest;
    let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 20));
    pane.render(Rect::new(0, 0, 120, 20), &mut buffer, &state, &ThemeConfig::dark(), false);

    // Sort by Direction
    pane.sort_column = SortColumn::Dir;
    let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 20));
    pane.render(Rect::new(0, 0, 120, 20), &mut buffer, &state, &ThemeConfig::dark(), false);

    // All should render without panic
}

#[test]
fn test_event_list_sort_reversed() {
    let events = vec![
        make_test_event_with_network(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1", "10.0.0.2", Direction::Inbound),
        make_test_event_with_network(2, 2_000_000_000, TransportKind::Zmq, "10.0.0.2", "10.0.0.3", Direction::Outbound),
    ];
    let store = EventStore::new(events);
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
            conversations: None,
        store,
    };

    let mut pane = EventListPane::new();
    pane.sort_reversed = true;

    let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 20));
    pane.render(Rect::new(0, 0, 120, 20), &mut buffer, &state, &ThemeConfig::dark(), false);

    // Should render without panic with reversed sort
}
