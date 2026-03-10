//! Detailed rendering tests for app.rs to improve coverage

use bytes::Bytes;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prb_core::{DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind};
use prb_tui::event_store::EventStore;
use prb_tui::App;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::collections::BTreeMap;

fn make_test_event_full(
    id: u64,
    timestamp_nanos: u64,
    transport: TransportKind,
    src: &str,
    dst: &str,
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
        direction: Direction::Inbound,
        payload: Payload::Raw {
            raw: Bytes::from(vec![0x48, 0x65, 0x6c, 0x6c, 0x6f]),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}

#[test]
fn test_app_render_all_panes_visible() {
    let events = vec![
        make_test_event_full(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1:1234", "10.0.0.2:5678"),
        make_test_event_full(2, 2_000_000_000, TransportKind::Zmq, "10.0.0.2:5678", "10.0.0.3:9999"),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, None);

    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);

    app.test_render_to_buffer(area, &mut buffer);

    // Verify that content was rendered (check for non-empty cells)
    let mut non_empty_cells = 0;
    for y in 0..area.height {
        for x in 0..area.width {
            if buffer[(x, y)].symbol() != " " {
                non_empty_cells += 1;
            }
        }
    }
    assert!(non_empty_cells > 100, "Should render substantial content");
}

#[test]
fn test_app_render_filter_bar_with_active_filter() {
    let events = vec![
        make_test_event_full(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1:1234", "10.0.0.2:5678"),
        make_test_event_full(2, 2_000_000_000, TransportKind::Zmq, "10.0.0.2:5678", "10.0.0.3:9999"),
        make_test_event_full(3, 3_000_000_000, TransportKind::Grpc, "10.0.0.3:9999", "10.0.0.4:1111"),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, Some(r#"transport == "gRPC""#.to_string()));

    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);

    app.test_render_to_buffer(area, &mut buffer);

    // Check that filter bar shows match count [2/3]
    let mut found_bracket = false;
    for y in 0..area.height {
        for x in 0..area.width {
            if buffer[(x, y)].symbol() == "[" {
                found_bracket = true;
                break;
            }
        }
    }
    assert!(found_bracket, "Should show match count in brackets");
}

#[test]
fn test_app_render_status_bar_content() {
    let events = vec![
        make_test_event_full(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1:1234", "10.0.0.2:5678"),
        make_test_event_full(2, 2_000_000_000, TransportKind::Zmq, "10.0.0.2:5678", "10.0.0.3:9999"),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, None);

    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);

    app.test_render_to_buffer(area, &mut buffer);

    // Status bar should show "2 events" and keybind hints
    let last_line_y = area.height - 1;
    let mut status_text = String::new();
    for x in 0..area.width {
        status_text.push_str(buffer[(x, last_line_y)].symbol());
    }

    assert!(status_text.contains("event") || status_text.contains("Tab") || status_text.contains("quit"),
        "Status bar should show event count or keybinds");
}

#[test]
fn test_app_render_with_filter_mode_active() {
    let events = vec![make_test_event_full(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1:1234", "10.0.0.2:5678")];
    let store = EventStore::new(events);
    let mut app = App::new(store, None);

    // Enter filter mode
    let slash_key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    app.test_handle_key(slash_key);

    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);

    app.test_render_to_buffer(area, &mut buffer);

    // Should show cursor/indicator in filter mode
    let first_line_text: String = (0..area.width)
        .map(|x| buffer[(x, 0)].symbol())
        .collect();

    assert!(first_line_text.contains("/") || first_line_text.contains("▏"),
        "Should show filter input indicator");
}

#[test]
fn test_app_render_different_pane_focus() {
    use prb_tui::app::PaneId;

    let events = vec![make_test_event_full(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1:1234", "10.0.0.2:5678")];
    let store = EventStore::new(events);
    let mut app = App::new(store, None);

    // Focus on different panes and render
    let panes = [
        PaneId::EventList,
        PaneId::DecodeTree,
        PaneId::HexDump,
        PaneId::Timeline,
    ];

    for _ in panes {
        let area = Rect::new(0, 0, 120, 40);
        let mut buffer = Buffer::empty(area);
        app.test_render_to_buffer(area, &mut buffer);

        // Cycle to next pane
        let tab_key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        app.test_handle_key(tab_key);
    }
}

#[test]
fn test_app_render_very_small_terminal() {
    let events = vec![make_test_event_full(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1:1234", "10.0.0.2:5678")];
    let store = EventStore::new(events);
    let mut app = App::new(store, None);

    // Minimum viable terminal size
    let area = Rect::new(0, 0, 30, 17);
    let mut buffer = Buffer::empty(area);

    // Should handle gracefully without panic
    app.test_render_to_buffer(area, &mut buffer);
}

#[test]
fn test_app_render_very_wide_terminal() {
    let events = vec![make_test_event_full(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1:1234", "10.0.0.2:5678")];
    let store = EventStore::new(events);
    let mut app = App::new(store, None);

    // Very wide terminal
    let area = Rect::new(0, 0, 250, 40);
    let mut buffer = Buffer::empty(area);

    // Should handle gracefully without panic
    app.test_render_to_buffer(area, &mut buffer);
}

#[test]
fn test_app_render_help_overlay_small_terminal() {
    let events = vec![make_test_event_full(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1:1234", "10.0.0.2:5678")];
    let store = EventStore::new(events);
    let mut app = App::new(store, None);

    // Enter help mode
    let help_key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    app.test_handle_key(help_key);

    // Small terminal
    let area = Rect::new(0, 0, 50, 25);
    let mut buffer = Buffer::empty(area);

    app.test_render_to_buffer(area, &mut buffer);

    // Should render help without panic even in small space
}

#[test]
fn test_app_render_help_overlay_very_small() {
    let events = vec![make_test_event_full(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1:1234", "10.0.0.2:5678")];
    let store = EventStore::new(events);
    let mut app = App::new(store, None);

    // Enter help mode
    let help_key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    app.test_handle_key(help_key);

    // Very small terminal (help overlay constrained)
    let area = Rect::new(0, 0, 30, 17);
    let mut buffer = Buffer::empty(area);

    app.test_render_to_buffer(area, &mut buffer);
}

#[test]
fn test_app_filter_mode_typing_simulation() {
    let events = vec![
        make_test_event_full(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1:1234", "10.0.0.2:5678"),
        make_test_event_full(2, 2_000_000_000, TransportKind::Zmq, "10.0.0.2:5678", "10.0.0.3:9999"),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, None);

    // Enter filter mode
    let slash_key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    app.test_handle_key(slash_key);

    // Type some characters
    let char_t = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE);
    app.test_handle_key(char_t);

    let char_r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
    app.test_handle_key(char_r);

    // Verify still in filter mode
    assert_eq!(app.get_input_mode(), prb_tui::app::InputMode::Filter);

    // Exit filter mode without applying
    let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.test_handle_key(esc_key);

    assert_eq!(app.get_input_mode(), prb_tui::app::InputMode::Normal);
}

#[test]
fn test_app_render_with_multiple_protocol_types() {
    let events = vec![
        make_test_event_full(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1:1234", "10.0.0.2:5678"),
        make_test_event_full(2, 2_000_000_000, TransportKind::Zmq, "10.0.0.2:5678", "10.0.0.3:9999"),
        make_test_event_full(3, 3_000_000_000, TransportKind::DdsRtps, "10.0.0.3:9999", "10.0.0.4:1111"),
        make_test_event_full(4, 4_000_000_000, TransportKind::RawTcp, "10.0.0.4:1111", "10.0.0.5:2222"),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, None);

    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);

    app.test_render_to_buffer(area, &mut buffer);

    // Status bar should show protocol counts for multiple types
    let last_line_y = area.height - 1;
    let mut found_protocol_indicator = false;
    for x in 0..area.width {
        let symbol = buffer[(x, last_line_y)].symbol();
        if symbol == ":" || symbol.chars().any(|c| c.is_ascii_digit()) {
            found_protocol_indicator = true;
            break;
        }
    }
    assert!(found_protocol_indicator, "Should show protocol counts");
}

#[test]
fn test_app_render_empty_filter_prompt() {
    let events = vec![make_test_event_full(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1:1234", "10.0.0.2:5678")];
    let store = EventStore::new(events);
    let mut app = App::new(store, None);

    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);

    app.test_render_to_buffer(area, &mut buffer);

    // Check filter bar shows "type / to filter" when no filter is active
    let first_line_text: String = (0..area.width)
        .map(|x| buffer[(x, 0)].symbol())
        .collect();

    assert!(first_line_text.contains("/") || first_line_text.contains("filter"),
        "Should show filter prompt");
}

#[test]
fn test_app_key_in_help_mode_ignored() {
    use prb_tui::app::InputMode;

    let events = vec![make_test_event_full(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1:1234", "10.0.0.2:5678")];
    let store = EventStore::new(events);
    let mut app = App::new(store, None);

    // Enter help mode
    let help_key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    app.test_handle_key(help_key);
    assert_eq!(app.get_input_mode(), InputMode::Help);

    // Try pressing random keys - should be ignored except for exit keys
    let random_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    let should_quit = app.test_handle_key(random_key);
    assert!(!should_quit);
    assert_eq!(app.get_input_mode(), InputMode::Help);

    // Press 'q' to exit help (not quit app)
    let q_key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    let should_quit = app.test_handle_key(q_key);
    assert!(!should_quit);
    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_app_process_action_quit() {
    use prb_tui::panes::Action;

    let events = vec![make_test_event_full(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1:1234", "10.0.0.2:5678")];
    let store = EventStore::new(events);
    let mut app = App::new(store, None);

    // Process Quit action (should not panic)
    app.test_process_action(Action::Quit);
}

#[test]
fn test_app_render_layout_constraints() {
    let events = vec![make_test_event_full(1, 1_000_000_000, TransportKind::Grpc, "10.0.0.1:1234", "10.0.0.2:5678")];
    let store = EventStore::new(events);
    let mut app = App::new(store, None);

    // Test various terminal sizes to ensure layout constraints work
    let sizes = vec![
        (40, 20),
        (80, 24),
        (120, 40),
        (200, 60),
    ];

    for (width, height) in sizes {
        let area = Rect::new(0, 0, width, height);
        let mut buffer = Buffer::empty(area);
        app.test_render_to_buffer(area, &mut buffer);
        // Should not panic with any reasonable size
    }
}
