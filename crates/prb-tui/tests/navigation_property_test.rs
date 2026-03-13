//! Property tests for navigation invariants

use bytes::Bytes;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prb_core::{DebugEvent, Direction, EventId, EventSource, Payload, Timestamp, TransportKind};
use prb_tui::App;
use prb_tui::event_store::EventStore;
use proptest::prelude::*;
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

proptest! {
    #[test]
    fn navigation_selection_always_valid(
        keys in prop::collection::vec(
            prop::sample::select(vec![KeyCode::Up, KeyCode::Down, KeyCode::PageUp, KeyCode::PageDown]),
            0..100
        ),
        event_count in 1usize..100
    ) {
        let events: Vec<_> = (1..=event_count as u64)
            .map(|i| make_test_event(i, i * 1_000_000_000))
            .collect();
        let store = EventStore::new(events);
        let mut app = App::new(store, None, None);

        app.test_set_terminal_size(80, 24);

        for key in keys {
            app.test_handle_key(KeyEvent::new(key, KeyModifiers::NONE));

            // Invariant: selection must always be valid
            let state = app.get_state();
            if let Some(selected_idx) = state.selected_event {
                prop_assert!(
                    selected_idx < state.filtered_indices.len(),
                    "Selected index {} out of bounds (len: {})",
                    selected_idx,
                    state.filtered_indices.len()
                );

                // The filtered index must point to a valid store index
                if let Some(&store_idx) = state.filtered_indices.get(selected_idx) {
                    prop_assert!(
                        store_idx < state.store.len(),
                        "Store index {} out of bounds (store len: {})",
                        store_idx,
                        state.store.len()
                    );
                }
            }
        }
    }

    #[test]
    fn navigation_with_empty_events_no_panic(
        keys in prop::collection::vec(
            prop::sample::select(vec![KeyCode::Up, KeyCode::Down, KeyCode::PageUp, KeyCode::PageDown]),
            0..50
        )
    ) {
        // Test with no events - should not panic
        let store = EventStore::new(vec![]);
        let mut app = App::new(store, None, None);

        app.test_set_terminal_size(80, 24);

        for key in keys {
            app.test_handle_key(KeyEvent::new(key, KeyModifiers::NONE));

            // With no events, selection should be None
            let state = app.get_state();
            prop_assert!(state.selected_event.is_none(), "Selection should be None with empty store");
        }
    }

    #[test]
    fn navigation_monotonic_with_down_keys(
        down_count in 1usize..50,
        event_count in 10usize..100
    ) {
        let events: Vec<_> = (1..=event_count as u64)
            .map(|i| make_test_event(i, i * 1_000_000_000))
            .collect();
        let store = EventStore::new(events);
        let mut app = App::new(store, None, None);

        app.test_set_terminal_size(80, 24);

        let mut prev_selected = app.get_state().selected_event;

        for _ in 0..down_count {
            app.test_handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

            let current_selected = app.get_state().selected_event;

            // Selection should either increase or stay at the end
            if let (Some(prev), Some(curr)) = (prev_selected, current_selected) {
                prop_assert!(
                    curr >= prev,
                    "Selection moved backward: {} -> {}",
                    prev,
                    curr
                );
            }

            prev_selected = current_selected;
        }
    }

    #[test]
    fn navigation_stays_in_bounds(
        key_sequence in prop::collection::vec(
            prop::sample::select(vec![
                KeyCode::Up,
                KeyCode::Down,
                KeyCode::PageUp,
                KeyCode::PageDown,
                KeyCode::Home,
                KeyCode::End,
            ]),
            0..200
        ),
        event_count in 1usize..200
    ) {
        let events: Vec<_> = (1..=event_count as u64)
            .map(|i| make_test_event(i, i * 1_000_000_000))
            .collect();
        let store = EventStore::new(events);
        let mut app = App::new(store, None, None);

        app.test_set_terminal_size(80, 24);

        for key in key_sequence {
            app.test_handle_key(KeyEvent::new(key, KeyModifiers::NONE));

            let state = app.get_state();

            // Core invariant: selected event must be valid
            if let Some(selected_idx) = state.selected_event {
                prop_assert!(
                    selected_idx < state.filtered_indices.len(),
                    "Selection {} out of bounds (max: {})",
                    selected_idx,
                    state.filtered_indices.len().saturating_sub(1)
                );
            }
        }
    }

    #[test]
    fn tab_navigation_cycles_through_panes(
        tab_count in 1usize..20
    ) {
        use prb_tui::app::PaneId;

        let events = vec![make_test_event(1, 1_000_000_000)];
        let store = EventStore::new(events);
        let mut app = App::new(store, None, None);

        app.test_set_terminal_size(80, 24);

        let initial_focus = app.get_focus();

        for _ in 0..tab_count {
            app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        }

        let final_focus = app.get_focus();

        // After tab_count tabs, we should have cycled through panes
        // The focus should be valid
        let valid_panes = [
            PaneId::EventList,
            PaneId::DecodeTree,
            PaneId::HexDump,
            PaneId::Timeline,
        ];
        prop_assert!(
            valid_panes.contains(&final_focus),
            "Focus on invalid pane: {:?}",
            final_focus
        );

        // After 4 tabs, should return to initial focus (4 panes total)
        if tab_count % 4 == 0 {
            prop_assert_eq!(final_focus, initial_focus, "Should cycle back after 4 tabs");
        }
    }

    #[test]
    fn shift_tab_reverses_tab_navigation(
        forward_tabs in 1usize..10,
        event_count in 1usize..10
    ) {
        let events: Vec<_> = (1..=event_count as u64)
            .map(|i| make_test_event(i, i * 1_000_000_000))
            .collect();
        let store = EventStore::new(events);
        let mut app = App::new(store, None, None);

        app.test_set_terminal_size(80, 24);

        let initial_focus = app.get_focus();

        // Tab forward
        for _ in 0..forward_tabs {
            app.test_handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        }

        // Tab backward the same number of times
        for _ in 0..forward_tabs {
            app.test_handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        }

        let final_focus = app.get_focus();

        // Should return to initial focus
        prop_assert_eq!(
            final_focus,
            initial_focus,
            "Should return to initial focus after forward+backward tabs"
        );
    }

    #[test]
    fn home_end_navigation_bounds(
        event_count in 1usize..100
    ) {
        let events: Vec<_> = (1..=event_count as u64)
            .map(|i| make_test_event(i, i * 1_000_000_000))
            .collect();
        let store = EventStore::new(events);
        let mut app = App::new(store, None, None);

        app.test_set_terminal_size(80, 24);

        // Press Home
        app.test_handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));

        let state_after_home = app.get_state();
        if let Some(selected) = state_after_home.selected_event {
            prop_assert_eq!(selected, 0, "Home should go to first event");
        }

        // Press End
        app.test_handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));

        let state_after_end = app.get_state();
        if let Some(selected) = state_after_end.selected_event {
            let max_idx = state_after_end.filtered_indices.len().saturating_sub(1);
            prop_assert_eq!(selected, max_idx, "End should go to last event");
        }
    }

    #[test]
    fn page_navigation_moves_in_chunks(
        event_count in 30usize..100
    ) {
        let events: Vec<_> = (1..=event_count as u64)
            .map(|i| make_test_event(i, i * 1_000_000_000))
            .collect();
        let store = EventStore::new(events);
        let mut app = App::new(store, None, None);

        app.test_set_terminal_size(80, 24);

        // Press PageDown
        let initial_selection = app.get_state().selected_event.unwrap_or(0);

        app.test_handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));

        let after_pagedown = app.get_state().selected_event.unwrap_or(0);

        // PageDown should move forward (or stay at end)
        prop_assert!(
            after_pagedown >= initial_selection,
            "PageDown should move forward or stay: {} -> {}",
            initial_selection,
            after_pagedown
        );

        // Press PageUp
        app.test_handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));

        let after_pageup = app.get_state().selected_event.unwrap_or(0);

        // PageUp should move backward (or stay at start)
        prop_assert!(
            after_pageup <= after_pagedown,
            "PageUp should move backward or stay: {} -> {}",
            after_pagedown,
            after_pageup
        );
    }
}

// Additional non-proptest test for coverage
#[test]
fn test_navigation_with_single_event() {
    // Edge case: only one event
    let events = vec![make_test_event(1, 1_000_000_000)];
    let store = EventStore::new(events);
    let mut app = App::new(store, None, None);

    app.test_set_terminal_size(80, 24);

    let initial_selected = app.get_state().selected_event;
    assert_eq!(initial_selected, Some(0));

    // Try to move down - should stay at 0
    app.test_handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.get_state().selected_event, Some(0));

    // Try to move up - should stay at 0
    app.test_handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.get_state().selected_event, Some(0));
}
