//! Integration tests covering cross-pane interactions and edge cases

use bytes::Bytes;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prb_core::{DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind};
use prb_tui::event_store::EventStore;
use prb_tui::App;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::collections::BTreeMap;

fn make_event(id: u64, ts: u64, transport: TransportKind) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(ts),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: Some(NetworkAddr {
                src: "10.0.0.1:1234".to_string(),
                dst: "10.0.0.2:5678".to_string(),
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
fn test_app_focus_cycling_full() {
    use prb_tui::app::PaneId;

    let events = vec![make_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Start at EventList
    assert_eq!(app.get_focus(), PaneId::EventList);

    // Cycle through all panes using Tab
    let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);

    app.test_handle_key(tab);
    assert_eq!(app.get_focus(), PaneId::DecodeTree);

    app.test_handle_key(tab);
    assert_eq!(app.get_focus(), PaneId::HexDump);

    app.test_handle_key(tab);
    assert_eq!(app.get_focus(), PaneId::Timeline);

    app.test_handle_key(tab);
    assert_eq!(app.get_focus(), PaneId::EventList); // Back to start

    // Now cycle backwards using BackTab
    let backtab = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);

    app.test_handle_key(backtab);
    assert_eq!(app.get_focus(), PaneId::Timeline);

    app.test_handle_key(backtab);
    assert_eq!(app.get_focus(), PaneId::HexDump);

    app.test_handle_key(backtab);
    assert_eq!(app.get_focus(), PaneId::DecodeTree);

    app.test_handle_key(backtab);
    assert_eq!(app.get_focus(), PaneId::EventList); // Back to start
}

#[test]
fn test_app_filter_workflow_complete() {
    use prb_tui::app::InputMode;

    let events = vec![
        make_event(1, 1_000_000_000, TransportKind::Grpc),
        make_event(2, 2_000_000_000, TransportKind::Zmq),
        make_event(3, 3_000_000_000, TransportKind::Grpc),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Initial state: no filter, 3 events
    assert_eq!(app.get_state().filtered_indices.len(), 3);
    assert!(app.get_state().filter.is_none());

    // Enter filter mode
    let slash = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    app.test_handle_key(slash);
    assert_eq!(app.get_input_mode(), InputMode::Filter);

    // Exit filter mode without applying (Esc)
    let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.test_handle_key(esc);
    assert_eq!(app.get_input_mode(), InputMode::Normal);
    assert_eq!(app.get_state().filtered_indices.len(), 3); // Still all events

    // Enter filter mode again and apply empty filter
    app.test_handle_key(slash);
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    app.test_handle_key(enter);
    assert_eq!(app.get_input_mode(), InputMode::Normal);
    assert_eq!(app.get_state().filtered_indices.len(), 3); // Empty filter = no filter

    // Clear any existing filter with Esc in normal mode
    app.test_handle_key(esc);
    assert_eq!(app.get_state().filtered_indices.len(), 3);
}

#[test]
fn test_app_help_mode_interactions() {
    use prb_tui::app::InputMode;

    let events = vec![make_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Enter help mode with '?'
    let question = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    app.test_handle_key(question);
    assert_eq!(app.get_input_mode(), InputMode::Help);

    // Random keys should be ignored in help mode
    let random = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
    let should_quit = app.test_handle_key(random);
    assert!(!should_quit);
    assert_eq!(app.get_input_mode(), InputMode::Help); // Still in help

    // Exit with Esc
    let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    let should_quit = app.test_handle_key(esc);
    assert!(!should_quit);
    assert_eq!(app.get_input_mode(), InputMode::Normal);

    // Re-enter and exit with '?'
    app.test_handle_key(question);
    assert_eq!(app.get_input_mode(), InputMode::Help);

    app.test_handle_key(question);
    assert_eq!(app.get_input_mode(), InputMode::Normal);

    // Re-enter and exit with 'q' (should exit help, not quit app)
    app.test_handle_key(question);
    assert_eq!(app.get_input_mode(), InputMode::Help);

    let q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    let should_quit = app.test_handle_key(q);
    assert!(!should_quit);
    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_app_render_sequence() {
    let events = vec![
        make_event(1, 1_000_000_000, TransportKind::Grpc),
        make_event(2, 2_000_000_000, TransportKind::Zmq),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let area = Rect::new(0, 0, 120, 40);

    // Render multiple times to simulate animation frames
    for _ in 0..10 {
        let mut buffer = Buffer::empty(area);
        app.test_render_to_buffer(area, &mut buffer);

        // Cycle focus
        let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        app.test_handle_key(tab);
    }

    // Should not panic or corrupt state
}

#[test]
fn test_app_with_all_transports_rendered() {
    let events = vec![
        make_event(1, 1_000_000_000, TransportKind::Grpc),
        make_event(2, 2_000_000_000, TransportKind::Zmq),
        make_event(3, 3_000_000_000, TransportKind::DdsRtps),
        make_event(4, 4_000_000_000, TransportKind::RawTcp),
        make_event(5, 5_000_000_000, TransportKind::RawUdp),
        make_event(6, 6_000_000_000, TransportKind::JsonFixture),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);

    app.test_render_to_buffer(area, &mut buffer);

    // Status bar should show protocol counts
    let last_line_y = area.height - 1;
    let mut has_protocol_counts = false;
    for x in 0..area.width {
        let symbol = buffer[(x, last_line_y)].symbol();
        if symbol == ":" || symbol.chars().any(|c| c.is_ascii_digit()) {
            has_protocol_counts = true;
            break;
        }
    }
    assert!(has_protocol_counts, "Should show protocol counts in status bar");
}

#[test]
fn test_app_quit_with_ctrl_c() {
    let events = vec![make_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let should_quit = app.test_handle_key(ctrl_c);

    assert!(should_quit, "Ctrl+C should trigger quit");
}

#[test]
fn test_app_quit_with_q() {
    let events = vec![make_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    let should_quit = app.test_handle_key(q);

    assert!(should_quit, "'q' should trigger quit");
}

#[test]
fn test_app_doesnt_quit_on_random_keys() {
    let events = vec![make_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let random_keys = vec![
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
    ];

    for key in random_keys {
        let should_quit = app.test_handle_key(key);
        assert!(!should_quit, "Random keys should not trigger quit");
    }
}

#[test]
fn test_app_multiple_renders_different_sizes() {
    let events = vec![make_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let sizes = vec![
        (40, 20),
        (80, 24),
        (120, 40),
        (160, 50),
        (50, 30),
    ];

    for (width, height) in sizes {
        let area = Rect::new(0, 0, width, height);
        let mut buffer = Buffer::empty(area);
        app.test_render_to_buffer(area, &mut buffer);
    }

    // Should handle all size changes without panic
}

#[test]
fn test_app_initial_state() {
    use prb_tui::app::{InputMode, PaneId};

    let events = vec![make_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let app = App::new(store, None, None);

    // Check initial state
    assert_eq!(app.get_focus(), PaneId::EventList);
    assert_eq!(app.get_input_mode(), InputMode::Normal);
    assert_eq!(app.get_state().selected_event, Some(0));
    assert!(app.get_filter_error().is_none());
}

#[test]
fn test_app_initial_state_empty_store() {
    use prb_tui::app::{InputMode, PaneId};

    let store = EventStore::new(vec![]);
    let app = App::new(store, None, None);

    // Check initial state with empty store
    assert_eq!(app.get_focus(), PaneId::EventList);
    assert_eq!(app.get_input_mode(), InputMode::Welcome);
    assert_eq!(app.get_state().selected_event, None); // No selection
    assert!(app.get_filter_error().is_none());
}

#[test]
fn test_app_render_with_different_directions() {
    let mut events = vec![];
    for i in 0..3 {
        let mut event = make_event(i, i * 1_000_000_000, TransportKind::Grpc);
        event.direction = match i {
            0 => Direction::Inbound,
            1 => Direction::Outbound,
            _ => Direction::Unknown,
        };
        events.push(event);
    }

    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);

    app.test_render_to_buffer(area, &mut buffer);

    // Should render all direction types
}
