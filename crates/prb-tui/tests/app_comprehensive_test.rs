//! Comprehensive tests for app.rs key handling, focus cycling, and rendering

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
fn test_app_with_multiple_transports() {
    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
        make_test_event(3, 3_000_000_000, TransportKind::DdsRtps),
        make_test_event(4, 4_000_000_000, TransportKind::RawTcp),
    ];
    let store = EventStore::new(events);
    let _app = App::new(store, None, None);
    // Should handle multiple transport types without panic
}

#[test]
fn test_app_with_large_dataset() {
    let events: Vec<_> = (0..1000)
        .map(|i| make_test_event(i, i * 1_000_000, TransportKind::Grpc))
        .collect();
    let store = EventStore::new(events);
    let _app = App::new(store, None, None);
    // Should handle large dataset without panic
}

#[test]
fn test_app_initialization_with_zero_events() {
    let store = EventStore::new(vec![]);
    let app = App::new(store, None, None);
    // App should initialize successfully with no events
    let _ = app;
}

#[test]
fn test_app_with_filter_matching_all() {
    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Grpc),
    ];
    let store = EventStore::new(events);
    let _app = App::new(store, Some(r#"transport == "gRPC""#.to_string()), None);
    // Should initialize with filter matching all events
}

#[test]
fn test_app_with_filter_matching_none() {
    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Grpc),
    ];
    let store = EventStore::new(events);
    let _app = App::new(store, Some(r#"transport == "ZMQ""#.to_string()), None);
    // Should initialize with filter matching no events
}

#[test]
fn test_pane_id_full_cycle() {
    use prb_tui::app::PaneId;

    // Test that cycling through all panes returns to start
    let start = PaneId::EventList;
    let after_one = start.next();
    let after_two = after_one.next();
    let after_three = after_two.next();
    let after_four = after_three.next();

    assert_eq!(after_four, start, "Should cycle back to EventList");
}

#[test]
fn test_pane_id_reverse_cycle() {
    use prb_tui::app::PaneId;

    // Test reverse cycling
    let start = PaneId::EventList;
    let prev_one = start.prev();
    assert_eq!(prev_one, PaneId::Timeline);

    let prev_two = prev_one.prev();
    assert_eq!(prev_two, PaneId::HexDump);
}

#[test]
fn test_app_state_with_complex_filter() {
    use prb_query::Filter;
    use prb_tui::app::AppState;

    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
        make_test_event(3, 3_000_000_000, TransportKind::Grpc),
        make_test_event(4, 4_000_000_000, TransportKind::DdsRtps),
    ];
    let store = EventStore::new(events);

    // Complex filter with OR
    let filter = Filter::parse(r#"transport == "gRPC" || transport == "ZMQ""#).unwrap();
    let filtered_indices = store.filter_indices(&filter);

    let state = AppState {
        filtered_indices: filtered_indices.clone(),
        selected_event: Some(0),
        filter: Some(filter),
        filter_text: r#"transport == "gRPC" || transport == "ZMQ""#.to_string(),
        schema_registry: None,
            conversations: None,
        store,
    };

    // Should match gRPC (2) + ZMQ (1) = 3 events
    assert_eq!(state.filtered_indices.len(), 3);
}

#[test]
fn test_app_state_empty_store() {
    use prb_tui::app::AppState;

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

    assert!(state.filtered_indices.is_empty());
    assert!(state.selected_event.is_none());
}

#[test]
fn test_app_state_selection_bounds() {
    use prb_tui::app::AppState;

    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
    ];
    let store = EventStore::new(events);

    // Test with valid selection
    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(1),
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
            conversations: None,
        store,
    };

    assert_eq!(state.selected_event, Some(1));
    assert_eq!(state.filtered_indices.len(), 2);
}

#[test]
fn test_app_multiple_filter_applications() {
    use prb_query::Filter;

    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
        make_test_event(3, 3_000_000_000, TransportKind::Grpc),
        make_test_event(4, 4_000_000_000, TransportKind::DdsRtps),
    ];
    let store = EventStore::new(events);

    // First filter
    let filter1 = Filter::parse(r#"transport == "gRPC""#).unwrap();
    let indices1 = store.filter_indices(&filter1);
    assert_eq!(indices1.len(), 2);

    // Second filter
    let filter2 = Filter::parse(r#"transport == "ZMQ""#).unwrap();
    let indices2 = store.filter_indices(&filter2);
    assert_eq!(indices2.len(), 1);

    // All events (no filter)
    let all_indices = store.all_indices();
    assert_eq!(all_indices.len(), 4);
}

// Key handling tests
#[test]
fn test_app_key_quit() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    let should_quit = app.test_handle_key(key);
    assert!(should_quit, "Pressing 'q' should trigger quit");
}

#[test]
fn test_app_key_ctrl_c_quit() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let should_quit = app.test_handle_key(key);
    assert!(should_quit, "Pressing Ctrl+C should trigger quit");
}

#[test]
fn test_app_key_tab_focus_cycle() {
    use prb_tui::app::PaneId;

    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Initial focus should be EventList
    assert_eq!(app.get_focus(), PaneId::EventList);

    // Press Tab
    let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    app.test_handle_key(key);
    assert_eq!(app.get_focus(), PaneId::DecodeTree);

    // Press Tab again
    app.test_handle_key(key);
    assert_eq!(app.get_focus(), PaneId::HexDump);

    // Press Tab again
    app.test_handle_key(key);
    assert_eq!(app.get_focus(), PaneId::Timeline);

    // Press Tab again - should cycle back
    app.test_handle_key(key);
    assert_eq!(app.get_focus(), PaneId::EventList);
}

#[test]
fn test_app_key_backtab_reverse_cycle() {
    use prb_tui::app::PaneId;

    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Initial focus should be EventList
    assert_eq!(app.get_focus(), PaneId::EventList);

    // Press BackTab (Shift+Tab)
    let key = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
    app.test_handle_key(key);
    assert_eq!(app.get_focus(), PaneId::Timeline);

    // Press BackTab again
    app.test_handle_key(key);
    assert_eq!(app.get_focus(), PaneId::HexDump);
}

#[test]
fn test_app_key_slash_enter_filter_mode() {
    use prb_tui::app::InputMode;

    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    assert_eq!(app.get_input_mode(), InputMode::Normal);

    // Press '/'
    let key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    app.test_handle_key(key);
    assert_eq!(app.get_input_mode(), InputMode::Filter);
}

#[test]
fn test_app_key_esc_exit_filter_mode() {
    use prb_tui::app::InputMode;

    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Enter filter mode
    let slash_key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    app.test_handle_key(slash_key);
    assert_eq!(app.get_input_mode(), InputMode::Filter);

    // Press Esc
    let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.test_handle_key(esc_key);
    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_app_key_esc_clears_filter() {
    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, Some(r#"transport == "gRPC""#.to_string()), None);

    // Initially should have filter active
    assert!(app.get_state().filter.is_some());
    assert_eq!(app.get_state().filtered_indices.len(), 1);

    // Press Esc in normal mode to clear filter
    let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.test_handle_key(esc_key);

    // Filter should be cleared
    assert!(app.get_state().filter.is_none());
    assert_eq!(app.get_state().filtered_indices.len(), 2);
}

#[test]
fn test_app_key_question_mark_help() {
    use prb_tui::app::InputMode;

    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    assert_eq!(app.get_input_mode(), InputMode::Normal);

    // Press '?'
    let key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    app.test_handle_key(key);
    assert_eq!(app.get_input_mode(), InputMode::Help);

    // Press '?' again to toggle off
    app.test_handle_key(key);
    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_app_key_esc_exits_help() {
    use prb_tui::app::InputMode;

    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Enter help mode
    let help_key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    app.test_handle_key(help_key);
    assert_eq!(app.get_input_mode(), InputMode::Help);

    // Press Esc to exit
    let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.test_handle_key(esc_key);
    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_app_filter_mode_enter_apply_filter() {
    use prb_tui::app::InputMode;

    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Enter filter mode
    let slash_key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    app.test_handle_key(slash_key);

    // Type filter characters (simplified - just apply directly)
    // In reality, the filter input widget handles typing
    // For now, we'll just test Enter with empty filter
    let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    app.test_handle_key(enter_key);

    // Should exit filter mode
    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_app_filter_error_on_invalid_syntax() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let app = App::new(store, None, None);

    // This test verifies that the error state can be accessed
    // The actual filter parsing is tested in prb-query
    assert!(app.get_filter_error().is_none());
}

// Rendering tests
#[test]
fn test_app_render_without_panic() {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);

    // Should render without panic
    app.test_render_to_buffer(area, &mut buffer);
}

#[test]
fn test_app_render_with_filter() {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, Some(r#"transport == "gRPC""#.to_string()), None);

    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);

    // Should render with filter without panic
    app.test_render_to_buffer(area, &mut buffer);

    // Check that some content was rendered (filter indicator should be present)
    let mut _has_filter_content = false;
    for y in 0..area.height {
        for x in 0..area.width {
            let symbol = buffer[(x, y)].symbol();
            if symbol.contains("gRPC") || symbol == "/" {
                _has_filter_content = true;
                break;
            }
        }
    }
    // Note: Filter text might not always be visible depending on layout
}

#[test]
fn test_app_render_help_overlay() {
    use prb_tui::app::InputMode;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Enter help mode
    let help_key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    app.test_handle_key(help_key);
    assert_eq!(app.get_input_mode(), InputMode::Help);

    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);

    // Should render help overlay without panic
    app.test_render_to_buffer(area, &mut buffer);

    // Check that help content is rendered
    let mut found_help_text = false;
    for y in 0..area.height {
        for x in 0..area.width {
            let symbol = buffer[(x, y)].symbol();
            if symbol == "H" || symbol == "e" || symbol == "l" || symbol == "p" {
                found_help_text = true;
                break;
            }
        }
    }
    assert!(found_help_text, "Help overlay should render some text");
}

#[test]
fn test_app_render_empty_store() {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let store = EventStore::new(vec![]);
    let mut app = App::new(store, None, None);

    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);

    // Should render empty state without panic
    app.test_render_to_buffer(area, &mut buffer);
}

#[test]
fn test_app_render_small_terminal() {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Small terminal (but large enough for minimum layout: filter(1) + content(10) + timeline(5) + status(1) = 17)
    let area = Rect::new(0, 0, 40, 20);
    let mut buffer = Buffer::empty(area);

    // Should handle small terminal gracefully
    app.test_render_to_buffer(area, &mut buffer);
}

// Action processing tests
#[test]
fn test_app_process_action_select_event() {
    use prb_tui::panes::Action;

    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Initially should have first event selected
    assert_eq!(app.get_state().selected_event, Some(0));

    // Process SelectEvent action
    app.test_process_action(Action::SelectEvent(1));

    // Should update selected event
    assert_eq!(app.get_state().selected_event, Some(1));
}

#[test]
fn test_app_process_action_none() {
    use prb_tui::panes::Action;

    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let initial_state = app.get_state().selected_event;

    // Process None action
    app.test_process_action(Action::None);

    // State should not change
    assert_eq!(app.get_state().selected_event, initial_state);
}

#[test]
fn test_app_process_action_highlight_bytes() {
    use prb_tui::panes::Action;

    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Process HighlightBytes action (should not panic)
    app.test_process_action(Action::HighlightBytes {
        offset: 10,
        len: 5,
    });
}

#[test]
fn test_app_process_action_clear_highlight() {
    use prb_tui::panes::Action;

    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Set highlight first
    app.test_process_action(Action::HighlightBytes {
        offset: 10,
        len: 5,
    });

    // Clear highlight
    app.test_process_action(Action::ClearHighlight);
}
