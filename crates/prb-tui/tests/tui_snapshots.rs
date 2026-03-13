//! Full-app snapshot tests using insta.
//!
//! These tests capture the complete terminal output for various UI states,
//! making it easy to catch regressions in layout, positioning, or content.

use bytes::Bytes;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prb_core::{
    DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind,
};
use prb_tui::{App, event_store::EventStore};
use ratatui::{Terminal, backend::TestBackend};
use std::collections::BTreeMap;

/// Helper to create a test event with minimal setup.
fn make_grpc_event(id: u64) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(id * 1_000_000_000),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: Some(NetworkAddr {
                src: "10.0.0.1:1234".to_string(),
                dst: "10.0.0.2:5678".to_string(),
            }),
        },
        transport: TransportKind::Grpc,
        direction: Direction::Inbound,
        payload: Payload::Raw {
            raw: Bytes::from(b"Hello gRPC".to_vec()),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}

fn make_zmq_event(id: u64) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(id * 1_000_000_000),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: Some(NetworkAddr {
                src: "10.0.0.2:5678".to_string(),
                dst: "10.0.0.3:9999".to_string(),
            }),
        },
        transport: TransportKind::Zmq,
        direction: Direction::Outbound,
        payload: Payload::Raw {
            raw: Bytes::from(b"Hello ZMQ".to_vec()),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}

/// Render the app to a `TestBackend` at the given size.
fn render_app(app: &mut App, width: u16, height: u16) -> TestBackend {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| app.test_render_to_buffer(f.area(), f.buffer_mut()))
        .unwrap();
    terminal.backend().clone()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// State 1: Empty store
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn snapshot_empty_store_80x24() {
    let mut app = App::new(EventStore::new(vec![]), None, None);
    let backend = render_app(&mut app, 80, 24);
    insta::assert_snapshot!(backend);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// State 2: Two events, normal view
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn snapshot_two_events_120x40() {
    let events = vec![make_grpc_event(1), make_zmq_event(2)];
    let mut app = App::new(EventStore::new(events), None, None);
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// State 3: Active filter with match count
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn snapshot_active_filter_120x40() {
    let events = vec![make_grpc_event(1), make_zmq_event(2), make_grpc_event(3)];
    let mut app = App::new(
        EventStore::new(events),
        Some(r#"transport == "gRPC""#.to_string()),
        None,
    );
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// State 4: Help overlay
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn snapshot_help_overlay_120x40() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None, None);
    app.test_handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// State 5: Filter input mode
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn snapshot_filter_input_mode_120x40() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None, None);
    app.test_handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// State 6: Each pane focused (4 snapshots)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn snapshot_decode_tree_focused_120x40() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None, None);
    // Tab once → DecodeTree
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_hex_dump_focused_120x40() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None, None);
    // Tab twice → HexDump
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_timeline_focused_120x40() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None, None);
    // Tab three times → Timeline
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// State 7: Terminal size variants (80x24)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn snapshot_two_events_80x24() {
    let events = vec![make_grpc_event(1), make_zmq_event(2)];
    let mut app = App::new(EventStore::new(events), None, None);
    let backend = render_app(&mut app, 80, 24);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_decode_tree_focused_80x24() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None, None);
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let backend = render_app(&mut app, 80, 24);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_hex_dump_focused_80x24() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None, None);
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let backend = render_app(&mut app, 80, 24);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_timeline_focused_80x24() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None, None);
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let backend = render_app(&mut app, 80, 24);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_active_filter_80x24() {
    let events = vec![make_grpc_event(1), make_zmq_event(2), make_grpc_event(3)];
    let mut app = App::new(
        EventStore::new(events),
        Some(r#"transport == "gRPC""#.to_string()),
        None,
    );
    let backend = render_app(&mut app, 80, 24);
    insta::assert_snapshot!(backend);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// State 8: Input mode snapshots
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn snapshot_goto_event_mode_120x40() {
    let events = vec![make_grpc_event(1), make_grpc_event(2)];
    let mut app = App::new(EventStore::new(events), None, None);
    app.test_handle_key(KeyEvent::new(KeyCode::Char('#'), KeyModifiers::NONE));
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_welcome_mode_120x40() {
    // Welcome mode is shown when store is empty on first launch
    let mut app = App::new(EventStore::new(vec![]), None, None);
    // Force welcome mode by creating app fresh
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_command_palette_120x40() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None, None);
    app.test_handle_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE));
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_plugin_manager_120x40() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None, None);
    app.test_handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_export_dialog_120x40() {
    let events = vec![make_grpc_event(1), make_grpc_event(2)];
    let mut app = App::new(EventStore::new(events), None, None);
    app.test_handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_copy_mode_120x40() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None, None);
    app.test_handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

#[test]
#[ignore] // Terminal rendering differs in CI - investigate separately
fn snapshot_capture_config_120x40() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None, None);
    app.test_handle_key(KeyEvent::new(KeyCode::Char('L'), KeyModifiers::SHIFT));
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_session_info_120x40() {
    let events = vec![make_grpc_event(1), make_grpc_event(2)];
    let mut app = App::new(EventStore::new(events), None, None);
    app.test_handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_ai_filter_mode_120x40() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None, None);
    app.test_handle_key(KeyEvent::new(KeyCode::Char('@'), KeyModifiers::NONE));
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_which_key_overlay_120x40() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None, None);
    // Press 'f' to show which-key for filter shortcuts
    app.test_handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// State 9: Error states
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn snapshot_filter_no_matches_120x40() {
    let events = vec![make_grpc_event(1), make_zmq_event(2)];
    let mut app = App::new(
        EventStore::new(events),
        Some(r#"transport == "DDS""#.to_string()),
        None,
    );
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_filter_parse_error_120x40() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(
        EventStore::new(events),
        Some("invalid == syntax &&".to_string()),
        None,
    );
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_empty_store_with_filter_input_120x40() {
    let mut app = App::new(EventStore::new(vec![]), None, None);
    app.test_handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// State 10: Edge cases
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn snapshot_very_long_event_list_120x40() {
    // Create 100 events to test scrolling UI
    let events: Vec<_> = (1..=100)
        .map(|i| {
            if i % 2 == 0 {
                make_grpc_event(i)
            } else {
                make_zmq_event(i)
            }
        })
        .collect();
    let mut app = App::new(EventStore::new(events), None, None);
    // Scroll down a bit to show we're in the middle of a long list
    for _ in 0..10 {
        app.test_handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    }
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_wide_payload_120x40() {
    // Create event with very wide payload
    let wide_payload = b"A".repeat(500);
    let mut event = make_grpc_event(1);
    event.payload = Payload::Raw {
        raw: Bytes::from(wide_payload),
    };
    let mut app = App::new(EventStore::new(vec![event]), None, None);
    // Switch to hex dump to see horizontal scrolling
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_unicode_data_120x40() {
    // Create event with Unicode in payload
    let unicode_payload = "Hello 世界 🌍 Привет مرحبا";
    let mut event = make_grpc_event(1);
    event.payload = Payload::Raw {
        raw: Bytes::from(unicode_payload.as_bytes().to_vec()),
    };
    event.source.origin = "测试 🧪".to_string();
    let mut app = App::new(EventStore::new(vec![event]), None, None);
    let backend = render_app(&mut app, 120, 40);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_small_terminal_60x15() {
    let events = vec![make_grpc_event(1), make_zmq_event(2)];
    let mut app = App::new(EventStore::new(events), None, None);
    let backend = render_app(&mut app, 60, 15);
    insta::assert_snapshot!(backend);
}

#[test]
fn snapshot_help_overlay_80x24() {
    let events = vec![make_grpc_event(1)];
    let mut app = App::new(EventStore::new(events), None, None);
    app.test_handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    let backend = render_app(&mut app, 80, 24);
    insta::assert_snapshot!(backend);
}
