//! Comprehensive state management tests for app.rs
//! Focus: state machine, mode transitions, event selection, filter application

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prb_core::{DebugEvent, Direction, EventId, EventSource, Payload, Timestamp, TransportKind};
use prb_query::Filter;
use prb_tui::App;
use prb_tui::app::{AppState, InputMode, PaneId};
use prb_tui::event_store::EventStore;
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

// ============================================================================
// Priority 1: State Machine Tests (~200 lines)
// ============================================================================

#[test]
fn test_input_mode_normal_to_filter() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    assert_eq!(app.get_input_mode(), InputMode::Normal);

    let key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    app.test_handle_key(key);

    assert_eq!(app.get_input_mode(), InputMode::Filter);
}

#[test]
fn test_input_mode_filter_to_normal_on_esc() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Enter filter mode
    let slash_key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    app.test_handle_key(slash_key);
    assert_eq!(app.get_input_mode(), InputMode::Filter);

    // Exit with Esc
    let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.test_handle_key(esc_key);
    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_input_mode_filter_to_normal_on_enter() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Enter filter mode
    let slash_key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    app.test_handle_key(slash_key);

    // Apply filter with Enter
    let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    app.test_handle_key(enter_key);
    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_input_mode_normal_to_help() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    app.test_handle_key(key);

    assert_eq!(app.get_input_mode(), InputMode::Help);
}

#[test]
fn test_input_mode_help_to_normal_on_esc() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Enter help
    let help_key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    app.test_handle_key(help_key);
    assert_eq!(app.get_input_mode(), InputMode::Help);

    // Exit with Esc
    let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.test_handle_key(esc_key);
    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_input_mode_help_toggle() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let help_key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);

    // Toggle on
    app.test_handle_key(help_key);
    assert_eq!(app.get_input_mode(), InputMode::Help);

    // Toggle off
    app.test_handle_key(help_key);
    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_input_mode_goto_event() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let key = KeyEvent::new(KeyCode::Char('#'), KeyModifiers::NONE);
    app.test_handle_key(key);

    assert_eq!(app.get_input_mode(), InputMode::GoToEvent);
}

#[test]
fn test_input_mode_goto_event_exit_on_esc() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Enter goto mode
    let goto_key = KeyEvent::new(KeyCode::Char('#'), KeyModifiers::NONE);
    app.test_handle_key(goto_key);
    assert_eq!(app.get_input_mode(), InputMode::GoToEvent);

    // Exit with Esc
    let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.test_handle_key(esc_key);
    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_input_mode_command_palette() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let key = KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE);
    app.test_handle_key(key);

    assert_eq!(app.get_input_mode(), InputMode::CommandPalette);
}

#[test]
fn test_input_mode_ai_filter() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let key = KeyEvent::new(KeyCode::Char('@'), KeyModifiers::NONE);
    app.test_handle_key(key);

    assert_eq!(app.get_input_mode(), InputMode::AiFilter);
}

#[test]
fn test_input_mode_export_dialog() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
    app.test_handle_key(key);

    assert_eq!(app.get_input_mode(), InputMode::ExportDialog);
}

#[test]
fn test_input_mode_session_info() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
    app.test_handle_key(key);

    assert_eq!(app.get_input_mode(), InputMode::SessionInfo);
}

#[test]
fn test_input_mode_session_info_exit_on_esc() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Enter session info
    let info_key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
    app.test_handle_key(info_key);
    assert_eq!(app.get_input_mode(), InputMode::SessionInfo);

    // Exit with Esc
    let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.test_handle_key(esc_key);
    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_input_mode_copy_mode() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let key = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
    app.test_handle_key(key);

    assert_eq!(app.get_input_mode(), InputMode::CopyMode);
}

#[test]
fn test_pane_focus_cycle_tab() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    assert_eq!(app.get_focus(), PaneId::EventList);

    let tab_key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);

    app.test_handle_key(tab_key);
    assert_eq!(app.get_focus(), PaneId::DecodeTree);

    app.test_handle_key(tab_key);
    assert_eq!(app.get_focus(), PaneId::HexDump);

    app.test_handle_key(tab_key);
    assert_eq!(app.get_focus(), PaneId::Timeline);

    app.test_handle_key(tab_key);
    assert_eq!(app.get_focus(), PaneId::EventList);
}

#[test]
fn test_pane_focus_reverse_cycle_backtab() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    assert_eq!(app.get_focus(), PaneId::EventList);

    let backtab_key = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);

    app.test_handle_key(backtab_key);
    assert_eq!(app.get_focus(), PaneId::Timeline);

    app.test_handle_key(backtab_key);
    assert_eq!(app.get_focus(), PaneId::HexDump);

    app.test_handle_key(backtab_key);
    assert_eq!(app.get_focus(), PaneId::DecodeTree);

    app.test_handle_key(backtab_key);
    assert_eq!(app.get_focus(), PaneId::EventList);
}

#[test]
fn test_event_selection_initial_state() {
    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
    ];
    let store = EventStore::new(events);
    let app = App::new(store, None, None);

    assert_eq!(app.get_state().selected_event, Some(0));
    assert_eq!(app.get_state().filtered_indices.len(), 2);
}

#[test]
fn test_event_selection_empty_store() {
    let store = EventStore::new(vec![]);
    let app = App::new(store, None, None);

    assert_eq!(app.get_state().selected_event, None);
    assert_eq!(app.get_state().filtered_indices.len(), 0);
}

#[test]
fn test_filter_application_clears_on_esc() {
    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, Some(r#"transport == "gRPC""#.to_string()), None);

    // Should have filter applied
    assert!(app.get_state().filter.is_some());
    assert_eq!(app.get_state().filtered_indices.len(), 1);

    // Press Esc to clear filter
    let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.test_handle_key(esc_key);

    // Filter should be cleared
    assert!(app.get_state().filter.is_none());
    assert_eq!(app.get_state().filtered_indices.len(), 2);
}

#[test]
fn test_filter_clears_filter_text() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, Some(r#"transport == "gRPC""#.to_string()), None);

    assert!(!app.get_state().filter_text.is_empty());

    let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.test_handle_key(esc_key);

    assert!(app.get_state().filter_text.is_empty());
}

#[test]
fn test_filter_resets_selection() {
    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, Some(r#"transport == "gRPC""#.to_string()), None);

    // Clear filter
    let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.test_handle_key(esc_key);

    // Selection should reset to 0
    assert_eq!(app.get_state().selected_event, Some(0));
}

#[test]
fn test_app_state_filter_indices_match_filter() {
    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
        make_test_event(3, 3_000_000_000, TransportKind::Grpc),
    ];
    let store = EventStore::new(events);

    let filter = Filter::parse(r#"transport == "gRPC""#).unwrap();
    let filtered_indices = store.filter_indices(&filter);

    let state = AppState {
        filtered_indices,
        selected_event: Some(0),
        filter: Some(filter),
        filter_text: r#"transport == "gRPC""#.to_string(),
        schema_registry: None,
        conversations: None,
        store,
        visible_columns: Vec::new(),
    };

    assert_eq!(state.filtered_indices.len(), 2);
}

#[test]
fn test_app_state_multiple_transports() {
    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
        make_test_event(3, 3_000_000_000, TransportKind::DdsRtps),
        make_test_event(4, 4_000_000_000, TransportKind::RawTcp),
    ];
    let store = EventStore::new(events.clone());

    let state = AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
        conversations: None,
        store,
        visible_columns: Vec::new(),
    };

    assert_eq!(state.filtered_indices.len(), 4);
    assert_eq!(state.store.len(), 4);
}

// ============================================================================
// Priority 2: Command Handling Tests (~100 lines)
// ============================================================================

#[test]
fn test_quit_command_q_key() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    let should_quit = app.test_handle_key(key);

    assert!(should_quit);
}

#[test]
fn test_quit_command_ctrl_c() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let should_quit = app.test_handle_key(key);

    assert!(should_quit);
}

#[test]
fn test_non_quit_keys_return_false() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let keys = vec![
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
    ];

    for key in keys {
        let should_quit = app.test_handle_key(key);
        assert!(!should_quit);
    }
}

#[test]
fn test_help_navigation_j_key() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Enter help mode
    let help_key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    app.test_handle_key(help_key);

    // Press 'j' to scroll down (should not crash)
    let j_key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    app.test_handle_key(j_key);

    assert_eq!(app.get_input_mode(), InputMode::Help);
}

#[test]
fn test_help_navigation_k_key() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Enter help mode
    let help_key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    app.test_handle_key(help_key);

    // Press 'k' to scroll up (should not crash)
    let k_key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
    app.test_handle_key(k_key);

    assert_eq!(app.get_input_mode(), InputMode::Help);
}

#[test]
fn test_help_navigation_arrow_keys() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Enter help mode
    let help_key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    app.test_handle_key(help_key);

    // Test arrow keys
    let down_key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    app.test_handle_key(down_key);

    let up_key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
    app.test_handle_key(up_key);

    assert_eq!(app.get_input_mode(), InputMode::Help);
}

#[test]
fn test_goto_event_enter_exits_mode() {
    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Enter goto mode
    let goto_key = KeyEvent::new(KeyCode::Char('#'), KeyModifiers::NONE);
    app.test_handle_key(goto_key);
    assert_eq!(app.get_input_mode(), InputMode::GoToEvent);

    // Press Enter to exit
    let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    app.test_handle_key(enter_key);

    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_multiple_mode_transitions() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Normal -> Filter -> Normal
    let slash_key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    app.test_handle_key(slash_key);
    assert_eq!(app.get_input_mode(), InputMode::Filter);

    let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.test_handle_key(esc_key);
    assert_eq!(app.get_input_mode(), InputMode::Normal);

    // Normal -> Help -> Normal
    let help_key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    app.test_handle_key(help_key);
    assert_eq!(app.get_input_mode(), InputMode::Help);

    app.test_handle_key(esc_key);
    assert_eq!(app.get_input_mode(), InputMode::Normal);

    // Normal -> Goto -> Normal
    let goto_key = KeyEvent::new(KeyCode::Char('#'), KeyModifiers::NONE);
    app.test_handle_key(goto_key);
    assert_eq!(app.get_input_mode(), InputMode::GoToEvent);

    app.test_handle_key(esc_key);
    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

// ============================================================================
// Priority 3: Error State Handling Tests (~100 lines)
// ============================================================================

#[test]
fn test_filter_error_initially_none() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let app = App::new(store, None, None);

    assert!(app.get_filter_error().is_none());
}

#[test]
fn test_invalid_filter_at_initialization() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    // Invalid filter should be silently ignored
    let app = App::new(store, Some("invalid !! syntax".to_string()), None);

    assert!(app.get_state().filter.is_none());
}

#[test]
fn test_empty_store_with_filter() {
    let store = EventStore::new(vec![]);
    let app = App::new(store, Some(r#"transport == "gRPC""#.to_string()), None);

    assert_eq!(app.get_state().filtered_indices.len(), 0);
    assert_eq!(app.get_state().selected_event, None);
}

#[test]
fn test_filter_matching_no_events() {
    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Grpc),
    ];
    let store = EventStore::new(events);
    // Filter that matches nothing
    let app = App::new(store, Some(r#"transport == "ZMQ""#.to_string()), None);

    assert_eq!(app.get_state().filtered_indices.len(), 0);
    assert_eq!(app.get_state().selected_event, None);
}

#[test]
fn test_filter_error_cleared_on_entering_filter_mode() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Enter filter mode - error should be cleared
    let slash_key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    app.test_handle_key(slash_key);

    assert!(app.get_filter_error().is_none());
}

#[test]
fn test_state_consistency_after_filter_clear() {
    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
        make_test_event(3, 3_000_000_000, TransportKind::Grpc),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, Some(r#"transport == "gRPC""#.to_string()), None);

    assert_eq!(app.get_state().filtered_indices.len(), 2);

    // Clear filter
    let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.test_handle_key(esc_key);

    // All events should be visible
    assert_eq!(app.get_state().filtered_indices.len(), 3);
    assert!(app.get_state().filter.is_none());
    assert!(app.get_state().filter_text.is_empty());
}

#[test]
fn test_welcome_mode_dismisses_on_any_key() {
    let store = EventStore::new(vec![]);
    let mut app = App::new(store, None, None);

    // Empty store starts in Welcome mode
    assert_eq!(app.get_input_mode(), InputMode::Welcome);

    // Any key should dismiss
    let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
    app.test_handle_key(key);

    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_welcome_mode_dismisses_on_enter() {
    let store = EventStore::new(vec![]);
    let mut app = App::new(store, None, None);

    assert_eq!(app.get_input_mode(), InputMode::Welcome);

    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    app.test_handle_key(key);

    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_esc_with_no_filter_stays_normal() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    assert_eq!(app.get_input_mode(), InputMode::Normal);

    // Esc with no filter should not crash
    let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.test_handle_key(esc_key);

    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_session_info_mode_dismisses_with_i_key() {
    let events = vec![make_test_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Enter session info
    let info_key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
    app.test_handle_key(info_key);
    assert_eq!(app.get_input_mode(), InputMode::SessionInfo);

    // Dismiss with 'i' key
    app.test_handle_key(info_key);
    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_state_consistency_with_large_dataset() {
    let events: Vec<_> = (0..500)
        .map(|i| {
            let transport = if i % 2 == 0 {
                TransportKind::Grpc
            } else {
                TransportKind::Zmq
            };
            make_test_event(i, i * 1_000_000, transport)
        })
        .collect();

    let store = EventStore::new(events);
    let app = App::new(store, Some(r#"transport == "gRPC""#.to_string()), None);

    // Should have 250 gRPC events
    assert_eq!(app.get_state().filtered_indices.len(), 250);
    assert_eq!(app.get_state().selected_event, Some(0));
}

#[test]
fn test_filter_with_or_condition() {
    let events = vec![
        make_test_event(1, 1_000_000_000, TransportKind::Grpc),
        make_test_event(2, 2_000_000_000, TransportKind::Zmq),
        make_test_event(3, 3_000_000_000, TransportKind::DdsRtps),
    ];
    let store = EventStore::new(events);
    let app = App::new(
        store,
        Some(r#"transport == "gRPC" || transport == "ZMQ""#.to_string()),
        None,
    );

    // Should match gRPC and ZMQ
    assert_eq!(app.get_state().filtered_indices.len(), 2);
}

#[test]
fn test_pane_id_next_prev_consistency() {
    // Test that next().prev() returns to original
    assert_eq!(PaneId::EventList.next().prev(), PaneId::EventList);
    assert_eq!(PaneId::DecodeTree.next().prev(), PaneId::DecodeTree);
    assert_eq!(PaneId::HexDump.next().prev(), PaneId::HexDump);
    assert_eq!(PaneId::Timeline.next().prev(), PaneId::Timeline);
}

#[test]
fn test_pane_id_prev_next_consistency() {
    // Test that prev().next() returns to original
    assert_eq!(PaneId::EventList.prev().next(), PaneId::EventList);
    assert_eq!(PaneId::DecodeTree.prev().next(), PaneId::DecodeTree);
    assert_eq!(PaneId::HexDump.prev().next(), PaneId::HexDump);
    assert_eq!(PaneId::Timeline.prev().next(), PaneId::Timeline);
}
