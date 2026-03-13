//! Mouse interaction tests for TUI

use bytes::Bytes;
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use prb_core::{DebugEvent, Direction, EventId, EventSource, Payload, Timestamp, TransportKind};
use prb_tui::App;
use prb_tui::app::PaneId;
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
fn test_mouse_pane_focus_click() {
    // Create app with test events
    let events = vec![
        make_test_event(1, 1_000_000_000),
        make_test_event(2, 2_000_000_000),
    ];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    // Set initial terminal size to establish pane rects
    app.test_set_terminal_size(80, 24);

    // Initially focused on EventList
    assert_eq!(app.get_focus(), PaneId::EventList);

    // Get the decode tree rect for clicking
    let pane_rects = app.get_pane_rects();
    if let Some(decode_rect) = pane_rects.get(&PaneId::DecodeTree) {
        // Click in the middle of the decode tree pane
        let click_col = decode_rect.x + decode_rect.width / 2;
        let click_row = decode_rect.y + decode_rect.height / 2;

        let mouse_event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: click_col,
            row: click_row,
            modifiers: KeyModifiers::empty(),
        };

        app.test_handle_mouse(mouse_event);

        // Focus should have changed to DecodeTree
        assert_eq!(app.get_focus(), PaneId::DecodeTree);
    }
}

#[test]
fn test_mouse_click_all_panes() {
    // Test clicking on each pane changes focus appropriately
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(120, 40);

    let panes = vec![
        PaneId::EventList,
        PaneId::DecodeTree,
        PaneId::HexDump,
        PaneId::Timeline,
    ];

    for pane_id in panes {
        let pane_rects = app.get_pane_rects();
        if let Some(rect) = pane_rects.get(&pane_id) {
            // Click in the center of this pane
            let click_col = rect.x + rect.width / 2;
            let click_row = rect.y + rect.height / 2;

            let mouse_event = MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: click_col,
                row: click_row,
                modifiers: KeyModifiers::empty(),
            };

            app.test_handle_mouse(mouse_event);

            // Verify focus changed to this pane
            assert_eq!(
                app.get_focus(),
                pane_id,
                "Failed to focus {:?} by clicking at ({}, {})",
                pane_id,
                click_col,
                click_row
            );
        }
    }
}

#[test]
fn test_mouse_vertical_split_drag() {
    // Test dragging the vertical split border
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 40);

    let initial_split = app.get_vertical_split();
    assert_eq!(initial_split, 55); // Default 55%

    // Get the event list rect to find the border
    let pane_rects = app.get_pane_rects();
    if let Some(event_list_rect) = pane_rects.get(&PaneId::EventList) {
        let border_row = event_list_rect.y + event_list_rect.height;
        let border_col = event_list_rect.x + event_list_rect.width / 2;

        // Start drag at the border
        let mouse_down = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: border_col,
            row: border_row,
            modifiers: KeyModifiers::empty(),
        };
        app.test_handle_mouse(mouse_down);

        // Drag upward (decrease event list height)
        let mouse_drag = MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: border_col,
            row: border_row.saturating_sub(5),
            modifiers: KeyModifiers::empty(),
        };
        app.test_handle_mouse(mouse_drag);

        // Release
        let mouse_up = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: border_col,
            row: border_row.saturating_sub(5),
            modifiers: KeyModifiers::empty(),
        };
        app.test_handle_mouse(mouse_up);

        // Vertical split should have changed
        let new_split = app.get_vertical_split();
        assert_ne!(
            new_split, initial_split,
            "Vertical split should have changed after drag"
        );
    }
}

#[test]
fn test_mouse_horizontal_split_drag() {
    // Test dragging the horizontal split border
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(120, 40);

    let initial_split = app.get_horizontal_split();
    assert_eq!(initial_split, 40); // Default 40%

    // Get the decode tree rect to find the border
    let pane_rects = app.get_pane_rects();
    if let Some(decode_tree_rect) = pane_rects.get(&PaneId::DecodeTree) {
        let border_col = decode_tree_rect.x + decode_tree_rect.width;
        let border_row = decode_tree_rect.y + decode_tree_rect.height / 2;

        // Start drag at the border
        let mouse_down = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: border_col,
            row: border_row,
            modifiers: KeyModifiers::empty(),
        };
        app.test_handle_mouse(mouse_down);

        // Drag leftward (decrease decode tree width)
        let mouse_drag = MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: border_col.saturating_sub(5),
            row: border_row,
            modifiers: KeyModifiers::empty(),
        };
        app.test_handle_mouse(mouse_drag);

        // Release
        let mouse_up = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: border_col.saturating_sub(5),
            row: border_row,
            modifiers: KeyModifiers::empty(),
        };
        app.test_handle_mouse(mouse_up);

        // Horizontal split should have changed
        let new_split = app.get_horizontal_split();
        assert_ne!(
            new_split, initial_split,
            "Horizontal split should have changed after drag"
        );
    }
}

#[test]
fn test_mouse_scroll_down() {
    // Test scroll wheel down
    let events: Vec<_> = (1..=20)
        .map(|i| make_test_event(i, i * 1_000_000_000))
        .collect();
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    // Initially at the first event
    let _initial_state = app.get_state();

    // Scroll down in the event list area
    let pane_rects = app.get_pane_rects();
    if let Some(event_list_rect) = pane_rects.get(&PaneId::EventList) {
        let scroll_col = event_list_rect.x + 5;
        let scroll_row = event_list_rect.y + 5;

        let scroll_event = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: scroll_col,
            row: scroll_row,
            modifiers: KeyModifiers::empty(),
        };

        app.test_handle_mouse(scroll_event);

        // Selection should have moved (or scroll offset changed)
        // This test verifies scroll is handled without panicking
    }
}

#[test]
fn test_mouse_scroll_up() {
    // Test scroll wheel up
    let events: Vec<_> = (1..=20)
        .map(|i| make_test_event(i, i * 1_000_000_000))
        .collect();
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    // Move to middle of list first
    for _ in 0..10 {
        use crossterm::event::{KeyCode, KeyEvent};
        app.test_handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    }

    let pane_rects = app.get_pane_rects();
    if let Some(event_list_rect) = pane_rects.get(&PaneId::EventList) {
        let scroll_col = event_list_rect.x + 5;
        let scroll_row = event_list_rect.y + 5;

        let scroll_event = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: scroll_col,
            row: scroll_row,
            modifiers: KeyModifiers::empty(),
        };

        app.test_handle_mouse(scroll_event);

        // This test verifies scroll is handled without panicking
    }
}

#[test]
fn test_mouse_interaction_while_zoomed() {
    // Test that mouse interactions work correctly when a pane is zoomed
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    // Zoom the event list pane
    app.set_zoomed_pane(Some(PaneId::EventList));
    app.test_set_terminal_size(80, 24); // Recalculate layout

    // Verify zoomed state
    assert_eq!(app.get_zoomed_pane(), Some(PaneId::EventList));

    // Click in the zoomed pane
    let pane_rects = app.get_pane_rects();
    if let Some(zoomed_rect) = pane_rects.get(&PaneId::EventList) {
        let click_col = zoomed_rect.x + 5;
        let click_row = zoomed_rect.y + 5;

        let mouse_event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: click_col,
            row: click_row,
            modifiers: KeyModifiers::empty(),
        };

        app.test_handle_mouse(mouse_event);

        // Should still be focused on EventList
        assert_eq!(app.get_focus(), PaneId::EventList);
    }
}

#[test]
fn test_mouse_outside_panes() {
    // Test clicking outside of any pane (e.g., in status bar area)
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    let initial_focus = app.get_focus();

    // Click in the status bar area (bottom row)
    let mouse_event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 40,
        row: 23, // Last row (status bar)
        modifiers: KeyModifiers::empty(),
    };

    app.test_handle_mouse(mouse_event);

    // Focus should not change when clicking outside panes
    assert_eq!(app.get_focus(), initial_focus);
}
