//! Unit and integration tests for app.rs

use prb_core::{DebugEvent, Direction, EventId, EventSource, Payload, Timestamp, TransportKind};
use prb_tui::event_store::EventStore;
use prb_tui::App;
use std::collections::BTreeMap;

fn make_test_event(id: u64, timestamp_nanos: u64, transport: TransportKind) -> DebugEvent {
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
            raw: bytes::Bytes::from(vec![1, 2, 3, 4]),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}

#[test]
fn test_app_new_with_empty_store() {
    let store = EventStore::new(vec![]);
    let _app = App::new(store, None, None);
    // App should be constructed successfully even with empty store
    // This test verifies initialization doesn't panic
}

#[test]
fn test_app_new_with_events() {
    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
    ];
    let store = EventStore::new(events);
    let _app = App::new(store, None, None);
    // App should be constructed successfully with events
    // This test verifies initialization doesn't panic
}

#[test]
fn test_app_new_with_initial_filter() {
    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
    ];
    let store = EventStore::new(events);
    let _app = App::new(store, Some(r#"transport == "gRPC""#.to_string()), None);
    // App should be constructed successfully with initial filter
    // This test verifies initialization with filter doesn't panic
}

#[test]
fn test_app_new_with_invalid_initial_filter() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    // Invalid filter should be ignored and app should still construct
    let _app = App::new(store, Some("invalid filter syntax".to_string()), None);
    // This test verifies initialization with invalid filter doesn't panic
}

#[test]
fn test_pane_id_next_cycle() {
    use prb_tui::app::PaneId;

    assert_eq!(PaneId::EventList.next(), PaneId::DecodeTree);
    assert_eq!(PaneId::DecodeTree.next(), PaneId::HexDump);
    assert_eq!(PaneId::HexDump.next(), PaneId::Timeline);
    assert_eq!(PaneId::Timeline.next(), PaneId::EventList);
}

#[test]
fn test_pane_id_prev_cycle() {
    use prb_tui::app::PaneId;

    assert_eq!(PaneId::EventList.prev(), PaneId::Timeline);
    assert_eq!(PaneId::DecodeTree.prev(), PaneId::EventList);
    assert_eq!(PaneId::HexDump.prev(), PaneId::DecodeTree);
    assert_eq!(PaneId::Timeline.prev(), PaneId::HexDump);
}

#[test]
fn test_app_state_initialization() {
    use prb_tui::app::AppState;

    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
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

    assert_eq!(state.filtered_indices.len(), 2);
    assert_eq!(state.selected_event, Some(0));
    assert!(state.filter.is_none());
    assert!(state.filter_text.is_empty());
}

#[test]
fn test_app_state_with_filter() {
    use prb_query::Filter;
    use prb_tui::app::AppState;

    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
        make_test_event(3, 3_000_000_000, TransportKind::Grpc),
    ];
    let store = EventStore::new(events);

    let filter = Filter::parse(r#"transport == "gRPC""#).unwrap();
    let filtered_indices = store.filter_indices(&filter);

    let state = AppState {
        filtered_indices: filtered_indices.clone(),
        selected_event: Some(0),
        filter: Some(filter),
        filter_text: r#"transport == "gRPC""#.to_string(),
        schema_registry: None,
            conversations: None,
        store,
    };

    // Should have 2 gRPC events
    assert_eq!(state.filtered_indices.len(), 2);
    assert!(state.filter.is_some());
    assert_eq!(state.filter_text, r#"transport == "gRPC""#);
}
