//! Tests for timeline bucket calculations, zoom, and cursor interaction (S11).

use bytes::Bytes;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prb_core::{DebugEvent, Direction, EventId, EventSource, Payload, Timestamp, TransportKind};
use prb_tui::app::AppState;
use prb_tui::event_store::EventStore;
use prb_tui::panes::timeline::TimelinePane;
use prb_tui::panes::{Action, PaneComponent};
use prb_tui::theme::ThemeConfig;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::collections::BTreeMap;

fn make_event_at_time(id: u64, nanos: u64, transport: TransportKind) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(nanos),
        source: EventSource {
            adapter: "test".into(),
            origin: "test.pcap".into(),
            network: None,
        },
        transport,
        direction: Direction::Inbound,
        payload: Payload::Raw {
            raw: Bytes::from(vec![1, 2, 3]),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}

fn make_app_state(events: Vec<DebugEvent>, selected: Option<usize>) -> AppState {
    let store = EventStore::new(events);
    AppState {
        filtered_indices: store.all_indices(),
        selected_event: selected,
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
        conversations: None,
        store,
        visible_columns: Vec::new(),
    }
}

#[test]
fn test_timeline_cursor_left_right() {
    let events = vec![
        make_event_at_time(1, 1_000_000_000, TransportKind::Grpc),
        make_event_at_time(2, 2_000_000_000, TransportKind::Grpc),
    ];
    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Render to set bucket count
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));
    pane.render(
        Rect::new(0, 0, 80, 10),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Press Right to move cursor
    pane.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &state);

    // Cursor should have moved (internal state)
    // Press Left to move back
    pane.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), &state);

    // Should not panic
}

#[test]
fn test_timeline_cursor_bounds_checking() {
    let events = vec![make_event_at_time(1, 1_000_000_000, TransportKind::Grpc)];
    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Render to set bucket count
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));
    pane.render(
        Rect::new(0, 0, 80, 10),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Move cursor right many times - should stop at end
    for _ in 0..200 {
        pane.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &state);
    }

    // Move cursor left many times - should stop at beginning
    for _ in 0..200 {
        pane.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), &state);
    }

    // Should not panic or overflow
}

#[test]
fn test_timeline_enter_jumps_to_event() {
    let events = vec![
        make_event_at_time(1, 1_000_000_000, TransportKind::Grpc),
        make_event_at_time(2, 2_000_000_000, TransportKind::Grpc),
        make_event_at_time(3, 3_000_000_000, TransportKind::Grpc),
    ];
    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Render to set bucket count
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));
    pane.render(
        Rect::new(0, 0, 80, 10),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Move cursor to a bucket
    pane.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &state);

    // Press Enter
    let action = pane.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &state);

    // Should return SelectEvent action (if bucket has events)
    // Or Action::None if bucket is empty
    match action {
        Action::SelectEvent(_) => {
            // Success - found an event in bucket
        }
        Action::None => {
            // Bucket was empty
        }
        _ => panic!("Unexpected action from Enter key"),
    }
}

#[test]
fn test_timeline_shift_left_right_creates_selection() {
    let events = vec![
        make_event_at_time(1, 1_000_000_000, TransportKind::Grpc),
        make_event_at_time(2, 2_000_000_000, TransportKind::Grpc),
    ];
    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Render to set bucket count
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));
    pane.render(
        Rect::new(0, 0, 80, 10),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Initialize cursor
    pane.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &state);

    // Press Shift+Right to start selection
    pane.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), &state);

    // Press Shift+Right again to expand selection
    pane.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), &state);

    // Should not panic - selection internal state should be updated
}

#[test]
fn test_timeline_shift_left_selection() {
    let events = vec![
        make_event_at_time(1, 1_000_000_000, TransportKind::Grpc),
        make_event_at_time(2, 2_000_000_000, TransportKind::Grpc),
    ];
    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Render to set bucket count
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));
    pane.render(
        Rect::new(0, 0, 80, 10),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Move cursor to middle
    pane.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &state);
    pane.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &state);

    // Press Shift+Left to create selection
    pane.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT), &state);

    // Should not panic
}

#[test]
fn test_timeline_escape_clears_selection() {
    let events = vec![
        make_event_at_time(1, 1_000_000_000, TransportKind::Grpc),
        make_event_at_time(2, 2_000_000_000, TransportKind::Grpc),
    ];
    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Render to set bucket count
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));
    pane.render(
        Rect::new(0, 0, 80, 10),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Create a selection
    pane.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &state);
    pane.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), &state);

    // Press Escape
    let action = pane.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &state);

    assert!(matches!(action, Action::None));
    // Selection should be cleared internally
}

#[test]
fn test_timeline_zoom_in() {
    let events = vec![make_event_at_time(1, 1_000_000_000, TransportKind::Grpc)];
    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Initial render
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));
    pane.render(
        Rect::new(0, 0, 80, 10),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Press '+' to zoom in
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE),
        &state,
    );

    // Re-render to see new bucket count
    let mut buffer2 = Buffer::empty(Rect::new(0, 0, 80, 10));
    pane.render(
        Rect::new(0, 0, 80, 10),
        &mut buffer2,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Bucket count should have increased (more detail)
    // Can't directly inspect, but no panic is good
}

#[test]
fn test_timeline_zoom_in_with_equals() {
    let events = vec![make_event_at_time(1, 1_000_000_000, TransportKind::Grpc)];
    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Press '=' (alternative to '+')
    let action = pane.handle_key(
        KeyEvent::new(KeyCode::Char('='), KeyModifiers::NONE),
        &state,
    );

    assert!(matches!(action, Action::None));
    // Zoom level should have increased
}

#[test]
fn test_timeline_zoom_out() {
    let events = vec![make_event_at_time(1, 1_000_000_000, TransportKind::Grpc)];
    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Press '-' to zoom out
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('-'), KeyModifiers::NONE),
        &state,
    );

    // Should not panic
}

#[test]
fn test_timeline_zoom_out_with_underscore() {
    let events = vec![make_event_at_time(1, 1_000_000_000, TransportKind::Grpc)];
    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Press '_' (Shift+minus)
    let action = pane.handle_key(
        KeyEvent::new(KeyCode::Char('_'), KeyModifiers::SHIFT),
        &state,
    );

    assert!(matches!(action, Action::None));
    // Zoom level should have decreased
}

#[test]
fn test_timeline_zoom_limits() {
    let events = vec![make_event_at_time(1, 1_000_000_000, TransportKind::Grpc)];
    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Zoom in many times
    for _ in 0..20 {
        pane.handle_key(
            KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE),
            &state,
        );
    }

    // Zoom out many times
    for _ in 0..20 {
        pane.handle_key(
            KeyEvent::new(KeyCode::Char('-'), KeyModifiers::NONE),
            &state,
        );
    }

    // Should not panic or overflow - zoom should be clamped
}

#[test]
fn test_timeline_mode_toggle_without_conversations() {
    let events = vec![make_event_at_time(1, 1_000_000_000, TransportKind::Grpc)];
    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Press 'h' to toggle mode (should not toggle without conversations)
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
        &state,
    );

    // Should not panic
}

#[test]
fn test_timeline_mode_toggle_uppercase_h() {
    let events = vec![make_event_at_time(1, 1_000_000_000, TransportKind::Grpc)];
    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Press 'H' (Shift+h) to toggle mode
    let action = pane.handle_key(
        KeyEvent::new(KeyCode::Char('H'), KeyModifiers::SHIFT),
        &state,
    );

    assert!(matches!(action, Action::None));
}

#[test]
fn test_timeline_render_multi_protocol_sparklines() {
    let events = vec![
        make_event_at_time(1, 1_000_000_000, TransportKind::Grpc),
        make_event_at_time(2, 1_500_000_000, TransportKind::Zmq),
        make_event_at_time(3, 2_000_000_000, TransportKind::Grpc),
        make_event_at_time(4, 2_500_000_000, TransportKind::DdsRtps),
    ];

    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Render in multi-protocol mode (default)
    let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 15));
    pane.render(
        Rect::new(0, 0, 120, 15),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Should render multiple protocol sparklines without panic
}

#[test]
fn test_timeline_cursor_render_in_focused_mode() {
    let events = vec![
        make_event_at_time(1, 1_000_000_000, TransportKind::Grpc),
        make_event_at_time(2, 2_000_000_000, TransportKind::Grpc),
    ];

    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Render to initialize
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));
    pane.render(
        Rect::new(0, 0, 80, 10),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Move cursor
    pane.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &state);

    // Render again with cursor visible (focused)
    let mut buffer2 = Buffer::empty(Rect::new(0, 0, 80, 10));
    pane.render(
        Rect::new(0, 0, 80, 10),
        &mut buffer2,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Cursor should be rendered (we can't easily test visual style, but no panic)
}

#[test]
fn test_timeline_selection_render() {
    let events = vec![
        make_event_at_time(1, 1_000_000_000, TransportKind::Grpc),
        make_event_at_time(2, 2_000_000_000, TransportKind::Grpc),
    ];

    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Render to initialize
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));
    pane.render(
        Rect::new(0, 0, 80, 10),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Create selection
    pane.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &state);
    pane.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), &state);
    pane.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), &state);

    // Render with selection visible
    let mut buffer2 = Buffer::empty(Rect::new(0, 0, 80, 10));
    pane.render(
        Rect::new(0, 0, 80, 10),
        &mut buffer2,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Selection should be rendered (no panic)
}

#[test]
fn test_timeline_with_filtered_events() {
    use prb_query::Filter;

    let events = vec![
        make_event_at_time(1, 1_000_000_000, TransportKind::Grpc),
        make_event_at_time(2, 2_000_000_000, TransportKind::Zmq),
        make_event_at_time(3, 3_000_000_000, TransportKind::Grpc),
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

    let mut pane = TimelinePane::new();

    // Render with filter active
    let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 10));
    pane.render(
        Rect::new(0, 0, 120, 10),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Should show filtered events and "(filtered)" indicator without panic
}

#[test]
fn test_timeline_zoom_level_display() {
    let events = vec![make_event_at_time(1, 1_000_000_000, TransportKind::Grpc)];
    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Zoom in
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE),
        &state,
    );

    // Render with zoom indicator
    let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 10));
    pane.render(
        Rect::new(0, 0, 120, 10),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Should render without panic - zoom indicator appears when zoom != 1.0
}

#[test]
fn test_timeline_small_area_graceful_handling() {
    let events = vec![make_event_at_time(1, 1_000_000_000, TransportKind::Grpc)];
    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Very small area
    let mut buffer = Buffer::empty(Rect::new(0, 0, 5, 1));
    pane.render(
        Rect::new(0, 0, 5, 1),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Should handle gracefully without panic
}

#[test]
fn test_timeline_cursor_info_display() {
    let events = vec![
        make_event_at_time(1, 1_000_000_000, TransportKind::Grpc),
        make_event_at_time(2, 1_500_000_000, TransportKind::Grpc),
        make_event_at_time(3, 2_000_000_000, TransportKind::Grpc),
    ];

    let state = make_app_state(events, Some(0));

    let mut pane = TimelinePane::new();

    // Render to initialize
    let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 10));
    pane.render(
        Rect::new(0, 0, 120, 10),
        &mut buffer,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Move cursor to a bucket
    pane.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &state);

    // Render with cursor info
    let mut buffer2 = Buffer::empty(Rect::new(0, 0, 120, 10));
    pane.render(
        Rect::new(0, 0, 120, 10),
        &mut buffer2,
        &state,
        &ThemeConfig::dark(),
        true,
    );

    // Should render cursor position and event count in legend without panic
}
