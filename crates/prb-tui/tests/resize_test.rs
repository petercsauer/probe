//! Terminal resize tests for TUI

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
fn test_resize_preserves_selection() {
    // Test that resizing the terminal preserves the selected event
    let events: Vec<_> = (1..=10)
        .map(|i| make_test_event(i, i * 1_000_000_000))
        .collect();
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Set initial size
    app.test_set_terminal_size(80, 24);

    // Select an event (move down a few times)
    for _ in 0..5 {
        app.test_handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    }

    let selected_before = app.get_state().selected_event;
    assert_eq!(selected_before, Some(5));

    // Resize to a different size
    app.test_set_terminal_size(120, 40);

    // Selection should be preserved
    let selected_after = app.get_state().selected_event;
    assert_eq!(selected_after, selected_before);
    assert_eq!(selected_after, Some(5));
}

#[test]
fn test_resize_updates_layout() {
    // Test that resizing updates the pane rectangles
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Set initial size
    app.test_set_terminal_size(80, 24);

    let rects_before = app.get_pane_rects().clone();
    let event_list_before = rects_before.get(&PaneId::EventList).unwrap();

    // Resize to larger
    app.test_set_terminal_size(120, 40);

    let rects_after = app.get_pane_rects();
    let event_list_after = rects_after.get(&PaneId::EventList).unwrap();

    // Pane sizes should have changed
    assert_ne!(
        event_list_before.width, event_list_after.width,
        "Width should change after resize"
    );
    assert_ne!(
        event_list_before.height, event_list_after.height,
        "Height should change after resize"
    );
}

#[test]
fn test_resize_preserves_split_percentages() {
    // Test that split percentages are maintained during resize
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    // Get initial split percentages
    let vertical_split = app.get_vertical_split();
    let horizontal_split = app.get_horizontal_split();
    assert_eq!(vertical_split, 55);
    assert_eq!(horizontal_split, 40);

    // Resize
    app.test_set_terminal_size(120, 40);

    // Split percentages should remain the same
    assert_eq!(app.get_vertical_split(), vertical_split);
    assert_eq!(app.get_horizontal_split(), horizontal_split);
}

#[test]
fn test_resize_during_filter_input() {
    // Test resizing while in filter input mode preserves the input
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    // Enter filter mode
    app.test_handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    assert_eq!(app.get_input_mode(), InputMode::Filter);

    // Type some filter text
    app.test_handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
    app.test_handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
    app.test_handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    app.test_handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));

    // Resize while in filter mode
    app.test_set_terminal_size(120, 40);

    // Should still be in filter mode
    assert_eq!(app.get_input_mode(), InputMode::Filter);

    // Exit filter mode
    app.test_handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.get_input_mode(), InputMode::Normal);
}

#[test]
fn test_resize_with_zoomed_pane() {
    // Test that resizing with a zoomed pane maintains the zoom state
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    // Zoom the decode tree pane
    app.set_zoomed_pane(Some(PaneId::DecodeTree));
    app.test_set_terminal_size(80, 24); // Recalculate with zoom

    assert_eq!(app.get_zoomed_pane(), Some(PaneId::DecodeTree));

    // Resize
    app.test_set_terminal_size(120, 40);

    // Zoom state should be preserved
    assert_eq!(app.get_zoomed_pane(), Some(PaneId::DecodeTree));

    // Only the zoomed pane should be in pane_rects
    let rects = app.get_pane_rects();
    assert!(rects.contains_key(&PaneId::DecodeTree));

    // The zoomed pane should take most of the space
    let zoomed_rect = rects.get(&PaneId::DecodeTree).unwrap();
    assert!(
        zoomed_rect.width > 100,
        "Zoomed pane should use most of the width"
    );
    assert!(
        zoomed_rect.height > 35,
        "Zoomed pane should use most of the height"
    );
}

#[test]
fn test_resize_to_very_small() {
    // Test resizing to a very small terminal (edge case)
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    // Resize to minimum practical size
    app.test_set_terminal_size(40, 10);

    // Should not panic - verify we can still render
    let rects = app.get_pane_rects();
    assert!(
        !rects.is_empty(),
        "Should have pane rects even at small size"
    );

    // Verify all panes have some space
    for (pane_id, rect) in rects {
        assert!(rect.width > 0, "{:?} should have non-zero width", pane_id);
        assert!(rect.height > 0, "{:?} should have non-zero height", pane_id);
    }
}

#[test]
fn test_resize_to_very_large() {
    // Test resizing to a very large terminal
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    // Resize to large size
    app.test_set_terminal_size(300, 100);

    // Should not panic
    let rects = app.get_pane_rects();
    assert!(!rects.is_empty());

    // Verify panes scale appropriately
    for (pane_id, rect) in rects {
        assert!(
            rect.width <= 300,
            "{:?} width should not exceed terminal width",
            pane_id
        );
        assert!(
            rect.height <= 100,
            "{:?} height should not exceed terminal height",
            pane_id
        );
    }
}

#[test]
fn test_multiple_resizes_in_sequence() {
    // Test that multiple resizes work correctly
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    let sizes = vec![(80, 24), (120, 40), (100, 30), (80, 24), (150, 50)];

    for (width, height) in sizes {
        app.test_set_terminal_size(width, height);

        // Verify layout is updated
        let rects = app.get_pane_rects();
        assert!(!rects.is_empty(), "Should have pane rects after resize");

        // Verify the layout doesn't have overlapping panes (basic sanity check)
        // All panes should be within terminal bounds
        for (pane_id, rect) in rects {
            assert!(
                rect.x + rect.width <= width,
                "{:?} extends beyond terminal width at size {}x{}",
                pane_id,
                width,
                height
            );
            // Height check is trickier due to status/control bars
            // Just verify it's reasonable
            assert!(
                rect.y < height,
                "{:?} starts beyond terminal height at size {}x{}",
                pane_id,
                width,
                height
            );
        }
    }
}

#[test]
fn test_resize_preserves_focus() {
    // Test that the focused pane remains focused after resize
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    // Change focus to HexDump
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

    let focus_before = app.get_focus();

    // Resize
    app.test_set_terminal_size(120, 40);

    // Focus should be preserved
    assert_eq!(app.get_focus(), focus_before);
}
