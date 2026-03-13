//! Keyboard-only accessibility tests for TUI

use bytes::Bytes;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prb_core::{DebugEvent, Direction, EventId, EventSource, Payload, Timestamp, TransportKind};
use prb_tui::App;
use prb_tui::app::{InputMode, PaneId};
use prb_tui::event_store::EventStore;
use std::collections::BTreeMap;

fn make_test_event(id: u64, timestamp_nanos: u64) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(timestamp_nanos),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: None,
        },
        transport: TransportKind::Grpc,
        direction: Direction::Inbound,
        payload: Payload::Raw {
            raw: Bytes::from(vec![1, 2, 3, 4]),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}

#[test]
fn test_keyboard_navigate_all_panes() {
    // Test that Tab key can navigate through all panes
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    let panes = [
        PaneId::EventList,
        PaneId::DecodeTree,
        PaneId::HexDump,
        PaneId::Timeline,
    ];

    // Start at EventList
    assert_eq!(app.get_focus(), PaneId::EventList);

    // Tab through all panes
    for expected_pane in panes.iter().skip(1) {
        app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(
            app.get_focus(),
            *expected_pane,
            "Tab should move to {:?}",
            expected_pane
        );
    }

    // One more tab should cycle back to EventList
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.get_focus(), PaneId::EventList);
}

#[test]
fn test_keyboard_reverse_navigate_panes() {
    // Test that Shift+Tab navigates backward through panes
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    // Start at EventList
    assert_eq!(app.get_focus(), PaneId::EventList);

    // Shift+Tab should go to Timeline (previous pane)
    app.test_handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    assert_eq!(app.get_focus(), PaneId::Timeline);

    // Shift+Tab again should go to HexDump
    app.test_handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    assert_eq!(app.get_focus(), PaneId::HexDump);

    // Shift+Tab again should go to DecodeTree
    app.test_handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    assert_eq!(app.get_focus(), PaneId::DecodeTree);

    // Shift+Tab again should cycle back to EventList
    app.test_handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    assert_eq!(app.get_focus(), PaneId::EventList);
}

#[test]
fn test_keyboard_open_help_overlay() {
    // Test that '?' opens help overlay
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    assert_eq!(app.get_input_mode(), InputMode::Normal);

    // Press '?' to open help
    app.test_handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));

    assert_eq!(app.get_input_mode(), InputMode::Help);

    // Press Esc to close
    app.test_handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_keyboard_open_filter_mode() {
    // Test that '/' opens filter mode
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    assert_eq!(app.get_input_mode(), InputMode::Normal);

    // Press '/' to open filter
    app.test_handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));

    assert_eq!(app.get_input_mode(), InputMode::Filter);

    // Press Esc to close
    app.test_handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_keyboard_quit_application() {
    // Test that 'q' quits the application
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    // Press 'q' should return true (indicating quit)
    let should_quit = app.test_handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));

    assert!(should_quit, "'q' should signal quit");
}

#[test]
fn test_keyboard_goto_event() {
    // Test that 'g' opens go-to-event overlay
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    assert_eq!(app.get_input_mode(), InputMode::Normal);

    // Press '#' to open go-to-event
    app.test_handle_key(KeyEvent::new(KeyCode::Char('#'), KeyModifiers::NONE));

    assert_eq!(app.get_input_mode(), InputMode::GoToEvent);

    // Press Esc to close
    app.test_handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_keyboard_event_navigation() {
    // Test that arrow keys navigate through events
    let events: Vec<_> = (1..=10)
        .map(|i| make_test_event(i, i * 1_000_000_000))
        .collect();
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    // Start at first event
    assert_eq!(app.get_state().selected_event, Some(0));

    // Press Down to move to next event
    app.test_handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.get_state().selected_event, Some(1));

    // Press Down again
    app.test_handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.get_state().selected_event, Some(2));

    // Press Up to move back
    app.test_handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.get_state().selected_event, Some(1));

    // Press Home to go to first
    app.test_handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
    assert_eq!(app.get_state().selected_event, Some(0));

    // Press End to go to last
    app.test_handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    assert_eq!(app.get_state().selected_event, Some(9));
}

#[test]
fn test_keyboard_page_navigation() {
    // Test that PageUp/PageDown navigate by pages
    let events: Vec<_> = (1..=50)
        .map(|i| make_test_event(i, i * 1_000_000_000))
        .collect();
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    let initial_selection = app.get_state().selected_event.unwrap();

    // Press PageDown
    app.test_handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));

    let after_pagedown = app.get_state().selected_event.unwrap();
    assert!(
        after_pagedown > initial_selection,
        "PageDown should move forward"
    );

    // Press PageUp
    app.test_handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));

    let after_pageup = app.get_state().selected_event.unwrap();
    assert!(after_pageup < after_pagedown, "PageUp should move backward");
}

#[test]
fn test_keyboard_zoom_pane() {
    // Test that 'z' zooms the current pane
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    // Initially not zoomed
    assert_eq!(app.get_zoomed_pane(), None);

    // Press 'z' to zoom
    app.test_handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));

    // Should now be zoomed on EventList
    assert_eq!(app.get_zoomed_pane(), Some(PaneId::EventList));

    // Press 'z' again to unzoom
    app.test_handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));

    // Should be unzoomed
    assert_eq!(app.get_zoomed_pane(), None);
}

#[test]
fn test_keyboard_all_overlays_accessible() {
    // Test that all major overlays can be opened with keyboard shortcuts
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    let test_cases = vec![
        ('?', InputMode::Help),
        ('/', InputMode::Filter),
        ('#', InputMode::GoToEvent),
    ];

    for (key_char, expected_mode) in test_cases {
        // Open overlay
        app.test_handle_key(KeyEvent::new(KeyCode::Char(key_char), KeyModifiers::NONE));
        assert_eq!(
            app.get_input_mode(),
            expected_mode,
            "Key '{}' should open {:?}",
            key_char,
            expected_mode
        );

        // Close with Esc
        app.test_handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(
            app.get_input_mode(),
            InputMode::Normal,
            "Esc should close {:?}",
            expected_mode
        );
    }
}

#[test]
fn test_keyboard_no_mouse_only_features() {
    // This test verifies that all core functionality is accessible via keyboard
    // by performing a complete workflow without any mouse input

    let events: Vec<_> = (1..=20)
        .map(|i| make_test_event(i, i * 1_000_000_000))
        .collect();
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(120, 40);

    // 1. Navigate through events
    for _ in 0..5 {
        app.test_handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    }
    assert_eq!(app.get_state().selected_event, Some(5));

    // 2. Navigate through all panes
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.get_focus(), PaneId::DecodeTree);

    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.get_focus(), PaneId::HexDump);

    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.get_focus(), PaneId::Timeline);

    // 3. Go back to EventList
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.get_focus(), PaneId::EventList);

    // 4. Open and close filter
    app.test_handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    assert_eq!(app.get_input_mode(), InputMode::Filter);

    app.test_handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.get_input_mode(), InputMode::Normal);

    // 5. Zoom a pane
    app.test_handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
    assert_eq!(app.get_zoomed_pane(), Some(PaneId::EventList));

    app.test_handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
    assert_eq!(app.get_zoomed_pane(), None);

    // 6. Use Home/End navigation
    app.test_handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    assert_eq!(app.get_state().selected_event, Some(19));

    app.test_handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
    assert_eq!(app.get_state().selected_event, Some(0));

    // All operations completed successfully without mouse
}

#[test]
fn test_keyboard_escape_from_all_modes() {
    // Test that Escape key reliably exits all input modes
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    let modes_to_test = vec![
        ('/', InputMode::Filter),
        ('?', InputMode::Help),
        ('#', InputMode::GoToEvent),
    ];

    for (key_to_open, mode) in modes_to_test {
        // Open the mode
        app.test_handle_key(KeyEvent::new(
            KeyCode::Char(key_to_open),
            KeyModifiers::NONE,
        ));
        assert_eq!(app.get_input_mode(), mode, "Failed to open {:?}", mode);

        // Press Escape
        app.test_handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        // Should be back in Normal mode
        assert_eq!(
            app.get_input_mode(),
            InputMode::Normal,
            "Escape should exit {:?}",
            mode
        );
    }
}

#[test]
fn test_keyboard_ctrl_c_alternative_quit() {
    // Test that Ctrl+C also quits (common terminal convention)
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    // Press Ctrl+C
    let should_quit = app.test_handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));

    assert!(should_quit, "Ctrl+C should signal quit");
}

#[test]
fn test_keyboard_accessible_event_selection() {
    // Verify that keyboard navigation allows precise event selection
    let events: Vec<_> = (1..=100)
        .map(|i| make_test_event(i, i * 1_000_000_000))
        .collect();
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    // Navigate to event 50 using various keyboard shortcuts
    app.test_handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    assert_eq!(app.get_state().selected_event, Some(99));

    // Use PageUp to go back
    for _ in 0..3 {
        app.test_handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
    }

    let after_pageups = app.get_state().selected_event.unwrap();
    assert!(after_pageups < 99, "Should have moved backward");

    // Use arrow keys for fine-grained control
    for _ in 0..10 {
        app.test_handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    }

    let after_arrows = app.get_state().selected_event.unwrap();
    assert!(
        after_arrows < after_pageups,
        "Arrow keys provide fine control"
    );
}
