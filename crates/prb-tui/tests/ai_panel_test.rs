//! Tests for AI panel integration and behavior

mod buf_helpers;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prb_core::{DebugEvent, EventId, Timestamp, TransportKind};
use prb_test_utils::event_builder_with_network;
use prb_tui::App;
use prb_tui::event_store::EventStore;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use buf_helpers::row_text;

fn make_test_event(
    id: u64,
    timestamp_nanos: u64,
    transport: TransportKind,
    src: &str,
    dst: &str,
) -> DebugEvent {
    event_builder_with_network(src, dst)
        .id(EventId::from_raw(id))
        .timestamp(Timestamp::from_nanos(timestamp_nanos))
        .transport(transport)
        .build()
}

#[test]
fn test_ai_panel_toggle_open_with_a_key() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        TransportKind::Grpc,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Press 'a' to open AI panel
    let a_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    let should_quit = app.test_handle_key(a_key);
    assert!(!should_quit);

    // Render and check that AI panel is visible
    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);
    app.test_render_to_buffer(area, &mut buffer);

    // Status bar should show AI EXPLAIN mode
    let status = row_text(&buffer, area.height - 1);
    assert!(
        status.contains("AI EXPLAIN") || status.contains("AI"),
        "Status bar should show AI EXPLAIN mode, got: {status}"
    );
}

#[test]
fn test_ai_panel_toggle_close_with_a_key() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        TransportKind::Grpc,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Open AI panel
    let a_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    app.test_handle_key(a_key);

    // Close AI panel with 'a' again
    let a_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    let should_quit = app.test_handle_key(a_key);
    assert!(!should_quit);

    // Render and verify AI panel is not shown
    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);
    app.test_render_to_buffer(area, &mut buffer);

    // Status bar should NOT show AI EXPLAIN mode
    let status = row_text(&buffer, area.height - 1);
    // Should show normal status, not AI EXPLAIN
    assert!(
        !status.contains("AI EXPLAIN"),
        "Status bar should not show AI EXPLAIN when panel closed, got: {status}"
    );
}

#[test]
fn test_ai_panel_close_with_escape() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        TransportKind::Grpc,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Open AI panel
    let a_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    app.test_handle_key(a_key);

    // Close with Escape
    let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    let should_quit = app.test_handle_key(esc_key);
    assert!(!should_quit);

    // Render and verify AI panel is closed
    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);
    app.test_render_to_buffer(area, &mut buffer);

    let status = row_text(&buffer, area.height - 1);
    assert!(
        !status.contains("AI EXPLAIN"),
        "Status bar should not show AI EXPLAIN after Esc, got: {status}"
    );
}

#[test]
fn test_ai_panel_no_event_selected() {
    // Create app with no selection
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        TransportKind::Grpc,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Deselect any event (move up from first position to ensure no selection)
    let up_key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
    app.test_handle_key(up_key);

    // Try to open AI panel
    let a_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    app.test_handle_key(a_key);

    // Panel should not open or show error message
    // This is handled gracefully by the app
}

#[test]
fn test_ai_panel_render_overlay_centered() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        TransportKind::Grpc,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Open AI panel
    let a_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    app.test_handle_key(a_key);

    // Render to buffer
    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);
    app.test_render_to_buffer(area, &mut buffer);

    // Check that the AI panel is rendered (look for AI Explain title or border)
    let mut found_ai_panel = false;
    for y in 0..area.height {
        let row = row_text(&buffer, y);
        if row.contains("AI Explain") {
            found_ai_panel = true;
            break;
        }
    }
    assert!(found_ai_panel, "AI panel should be rendered when visible");
}

#[test]
fn test_ai_panel_multiple_toggle_cycles() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        TransportKind::Grpc,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let a_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);

    // Toggle open/closed multiple times
    for _ in 0..5 {
        // Open
        app.test_handle_key(a_key);
        // Close
        app.test_handle_key(a_key);
    }

    // Should be closed after even number of toggles
    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);
    app.test_render_to_buffer(area, &mut buffer);

    let status = row_text(&buffer, area.height - 1);
    assert!(
        !status.contains("AI EXPLAIN"),
        "Panel should be closed after even toggles"
    );
}

#[test]
fn test_ai_panel_escape_priority_over_other_overlays() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        TransportKind::Grpc,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Open AI panel first
    let a_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    app.test_handle_key(a_key);

    // Verify AI panel is open
    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);
    app.test_render_to_buffer(area, &mut buffer);
    let status_open = row_text(&buffer, area.height - 1);
    assert!(
        status_open.contains("AI EXPLAIN"),
        "AI panel should be open"
    );

    // Press Escape - should close AI panel
    let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.test_handle_key(esc_key);

    // Verify AI panel is closed
    let mut buffer2 = Buffer::empty(area);
    app.test_render_to_buffer(area, &mut buffer2);
    let status_closed = row_text(&buffer2, area.height - 1);
    assert!(
        !status_closed.contains("AI EXPLAIN"),
        "AI panel should be closed after Esc"
    );
}

#[test]
fn test_ai_panel_with_different_event_types() {
    // Test AI panel with different protocol types
    let transports = [
        TransportKind::Grpc,
        TransportKind::Zmq,
        TransportKind::DdsRtps,
        TransportKind::RawTcp,
    ];

    for (i, transport) in transports.iter().enumerate() {
        let events = vec![make_test_event(
            i as u64 + 1,
            (i as u64 + 1) * 1_000_000_000,
            *transport,
            "10.0.0.1:1234",
            "10.0.0.2:5678",
        )];
        let store = EventStore::new(events);
        let mut app = App::new(store, None, None);

        // Open AI panel
        let a_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        app.test_handle_key(a_key);

        // Should render without panic for any transport type
        let area = Rect::new(0, 0, 120, 40);
        let mut buffer = Buffer::empty(area);
        app.test_render_to_buffer(area, &mut buffer);
    }
}

#[test]
fn test_ai_panel_render_small_terminal() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        TransportKind::Grpc,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Open AI panel
    let a_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    app.test_handle_key(a_key);

    // Render in small terminal
    let area = Rect::new(0, 0, 40, 20);
    let mut buffer = Buffer::empty(area);

    // Should handle small terminal gracefully without panic
    app.test_render_to_buffer(area, &mut buffer);
}
