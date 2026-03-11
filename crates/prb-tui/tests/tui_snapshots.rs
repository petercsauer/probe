//! Full-app snapshot tests using insta.
//!
//! These tests capture the complete terminal output for various UI states,
//! making it easy to catch regressions in layout, positioning, or content.

use bytes::Bytes;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prb_core::{DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind};
use prb_tui::{App, event_store::EventStore};
use ratatui::{backend::TestBackend, Terminal};
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

/// Render the app to a TestBackend at the given size.
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
    let mut app = App::new(EventStore::new(events), Some(r#"transport == "gRPC""#.to_string()), None);
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
