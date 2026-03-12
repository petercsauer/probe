//! Tests for hex dump search, navigation, and data transformation logic (S11).

use bytes::Bytes;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prb_core::{DebugEvent, Direction, EventId, EventSource, Payload, Timestamp, TransportKind};
use prb_tui::app::AppState;
use prb_tui::event_store::EventStore;
use prb_tui::panes::PaneComponent;
use prb_tui::panes::hex_dump::{ByteGrouping, HexDumpPane};
use std::collections::BTreeMap;

fn make_test_event(payload: Vec<u8>) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(1),
        timestamp: Timestamp::from_nanos(1000),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: None,
        },
        transport: TransportKind::Grpc,
        direction: Direction::Inbound,
        payload: Payload::Raw {
            raw: Bytes::from(payload),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}

fn make_app_state(events: Vec<DebugEvent>, selected_event: Option<usize>) -> AppState {
    let store = EventStore::new(events);
    AppState {
        filtered_indices: store.all_indices(),
        selected_event,
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
        conversations: None,
        store,
        visible_columns: Vec::new(),
    }
}

#[test]
fn test_byte_grouping_cycle() {
    assert_eq!(ByteGrouping::One.cycle(), ByteGrouping::Two);
    assert_eq!(ByteGrouping::Two.cycle(), ByteGrouping::Four);
    assert_eq!(ByteGrouping::Four.cycle(), ByteGrouping::One);
}

#[test]
fn test_hex_search_ascii() {
    let payload = b"Hello World! Testing search functionality.".to_vec();
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();

    // Enter search mode
    let key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    pane.handle_key(key, &state);

    // Type "World"
    for ch in "World".chars() {
        let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE);
        pane.handle_key(key, &state);
    }

    // Press Enter to execute search
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    pane.handle_key(key, &state);

    // Cursor should have moved to the match
    assert_eq!(pane.cursor_offset, 6); // "World" starts at byte 6
}

#[test]
fn test_hex_search_hex_pattern() {
    let payload = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x11, 0x22, 0x33];
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();

    // Enter search mode
    let key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    pane.handle_key(key, &state);

    // Type hex pattern "DEADBEEF"
    for ch in "DEADBEEF".chars() {
        let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE);
        pane.handle_key(key, &state);
    }

    // Press Enter to execute search
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    pane.handle_key(key, &state);

    // Cursor should have moved to the match at offset 0
    assert_eq!(pane.cursor_offset, 0);
}

#[test]
fn test_hex_search_hex_pattern_with_spaces() {
    let payload = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x11, 0x22, 0x33];
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();

    // Enter search mode
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
        &state,
    );

    // Type hex pattern "DE AD BE EF" (with spaces)
    for ch in "DE AD BE EF".chars() {
        pane.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), &state);
    }

    // Press Enter to execute search
    pane.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &state);

    // Should find the pattern at offset 0
    assert_eq!(pane.cursor_offset, 0);
}

#[test]
fn test_hex_search_next_match() {
    let payload = b"test test test".to_vec();
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();

    // Search for "test"
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
        &state,
    );
    for ch in "test".chars() {
        pane.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), &state);
    }
    pane.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &state);

    // Should be at first match (offset 0)
    assert_eq!(pane.cursor_offset, 0);

    // Press 'n' for next match
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        &state,
    );
    assert_eq!(pane.cursor_offset, 5); // Second "test" at offset 5

    // Press 'n' again
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        &state,
    );
    assert_eq!(pane.cursor_offset, 10); // Third "test" at offset 10

    // Press 'n' again - should wrap to first match
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        &state,
    );
    assert_eq!(pane.cursor_offset, 0); // Wrap to first match
}

#[test]
fn test_hex_search_prev_match() {
    let payload = b"test test test".to_vec();
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();

    // Search for "test"
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
        &state,
    );
    for ch in "test".chars() {
        pane.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), &state);
    }
    pane.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &state);

    // Should be at first match (offset 0)
    assert_eq!(pane.cursor_offset, 0);

    // Press 'N' (Shift+n) for previous match - should wrap to last
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT),
        &state,
    );
    assert_eq!(pane.cursor_offset, 10); // Wrap to last match

    // Press 'N' again
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT),
        &state,
    );
    assert_eq!(pane.cursor_offset, 5); // Previous match
}

#[test]
fn test_hex_search_no_matches() {
    let payload = b"Hello World".to_vec();
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();
    let initial_offset = pane.cursor_offset;

    // Search for non-existent pattern
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
        &state,
    );
    for ch in "NOTFOUND".chars() {
        pane.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), &state);
    }
    pane.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &state);

    // Cursor should not have moved
    assert_eq!(pane.cursor_offset, initial_offset);

    // Pressing 'n' should not panic
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        &state,
    );
}

#[test]
fn test_hex_search_escape_cancels() {
    let payload = b"Hello World".to_vec();
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();

    // Enter search mode
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
        &state,
    );

    // Type something
    for ch in "test".chars() {
        pane.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), &state);
    }

    // Press Escape to cancel
    pane.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &state);

    // Subsequent 'n' should not find anything (search was cancelled)
    let initial_offset = pane.cursor_offset;
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        &state,
    );
    assert_eq!(pane.cursor_offset, initial_offset);
}

#[test]
fn test_hex_search_backspace() {
    let payload = b"Hello World".to_vec();
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();

    // Enter search mode
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
        &state,
    );

    // Type "World"
    for ch in "World".chars() {
        pane.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), &state);
    }

    // Backspace twice
    pane.handle_key(
        KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        &state,
    );
    pane.handle_key(
        KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        &state,
    );

    // Now search buffer should be "Wor" - execute search
    pane.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &state);

    // Should find "Wor" in "World" at offset 6
    assert_eq!(pane.cursor_offset, 6);
}

#[test]
fn test_jump_to_offset() {
    let payload = vec![0u8; 1000];
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();

    // Enter jump-to-offset mode
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
        &state,
    );

    // Type "100" (hex offset)
    for ch in "100".chars() {
        pane.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), &state);
    }

    // Press Enter
    pane.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &state);

    // Should jump to offset 0x100 (256 decimal)
    assert_eq!(pane.cursor_offset, 256);
    assert_eq!(pane.scroll_offset, 16); // 256 / 16
}

#[test]
fn test_jump_to_offset_with_0x_prefix() {
    let payload = vec![0u8; 1000];
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();

    // Enter jump-to-offset mode
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
        &state,
    );

    // Type "0x100"
    for ch in "0x100".chars() {
        pane.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), &state);
    }

    // Press Enter
    pane.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &state);

    // Should jump to offset 0x100 (256 decimal)
    assert_eq!(pane.cursor_offset, 256);
}

#[test]
fn test_jump_to_offset_escape_cancels() {
    let payload = vec![0u8; 1000];
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();
    let initial_offset = pane.cursor_offset;

    // Enter jump-to-offset mode
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
        &state,
    );

    // Type something
    for ch in "200".chars() {
        pane.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), &state);
    }

    // Press Escape to cancel
    pane.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &state);

    // Cursor should not have moved
    assert_eq!(pane.cursor_offset, initial_offset);
}

#[test]
fn test_jump_to_end_with_uppercase_g() {
    let payload = vec![0u8; 1000];
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();

    // Press 'G' (Shift+g) to jump to end
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT),
        &state,
    );

    // scroll_offset should be very large (will be clamped during render)
    assert_eq!(pane.scroll_offset, usize::MAX);
}

#[test]
fn test_byte_grouping_toggle() {
    let payload = vec![0u8; 100];
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();

    // Initial grouping is One
    assert_eq!(pane.byte_grouping, ByteGrouping::One);

    // Press 'b' to cycle
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
        &state,
    );
    assert_eq!(pane.byte_grouping, ByteGrouping::Two);

    // Press 'b' again
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
        &state,
    );
    assert_eq!(pane.byte_grouping, ByteGrouping::Four);

    // Press 'b' again - should wrap to One
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
        &state,
    );
    assert_eq!(pane.byte_grouping, ByteGrouping::One);
}

#[test]
fn test_scroll_down() {
    let mut pane = HexDumpPane::new();

    pane.scroll_down(5);
    assert_eq!(pane.scroll_offset, 5);

    pane.scroll_down(3);
    assert_eq!(pane.scroll_offset, 8);
}

#[test]
fn test_scroll_up() {
    let mut pane = HexDumpPane::new();
    pane.scroll_offset = 10;

    pane.scroll_up(3);
    assert_eq!(pane.scroll_offset, 7);

    pane.scroll_up(10); // Should not go below 0
    assert_eq!(pane.scroll_offset, 0);
}

#[test]
fn test_page_down() {
    let payload = vec![0u8; 1000];
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();
    assert_eq!(pane.scroll_offset, 0);

    // Press PageDown
    pane.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), &state);
    assert_eq!(pane.scroll_offset, 16);

    // Press PageDown again
    pane.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), &state);
    assert_eq!(pane.scroll_offset, 32);
}

#[test]
fn test_page_up() {
    let payload = vec![0u8; 1000];
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();
    pane.scroll_offset = 32;

    // Press PageUp
    pane.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE), &state);
    assert_eq!(pane.scroll_offset, 16);

    // Press PageUp again
    pane.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE), &state);
    assert_eq!(pane.scroll_offset, 0);
}

#[test]
fn test_home_key() {
    let payload = vec![0u8; 1000];
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();
    pane.scroll_offset = 100;

    // Press Home
    pane.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), &state);
    assert_eq!(pane.scroll_offset, 0);
}

#[test]
fn test_mark_event_for_diff() {
    let payload = vec![1, 2, 3, 4, 5];
    let event = make_test_event(payload.clone());
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();

    // Press 'm' to mark current event
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
        &state,
    );

    // marked_event_bytes should be set (we can't directly inspect it, but render would show it)
    // This exercises the code path
}

#[test]
fn test_copy_highlighted_bytes() {
    let payload = vec![0xDE, 0xAD, 0xBE, 0xEF];
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();

    // Set a highlight region
    pane.set_highlight(0, 4);

    // Press 'y' to copy
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
        &state,
    );

    // This exercises the copy_bytes_to_clipboard code path
    // In a real terminal, OSC 52 would be sent
}

#[test]
fn test_arrow_key_navigation() {
    let payload = vec![0u8; 1000];
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();

    // Down arrow
    pane.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &state);
    assert_eq!(pane.scroll_offset, 1);

    // Down arrow again
    pane.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &state);
    assert_eq!(pane.scroll_offset, 2);

    // Up arrow
    pane.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), &state);
    assert_eq!(pane.scroll_offset, 1);
}

#[test]
fn test_vim_keys_navigation() {
    let payload = vec![0u8; 1000];
    let event = make_test_event(payload);
    let state = make_app_state(vec![event], Some(0));

    let mut pane = HexDumpPane::new();

    // 'j' for down
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        &state,
    );
    assert_eq!(pane.scroll_offset, 1);

    // 'j' again
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        &state,
    );
    assert_eq!(pane.scroll_offset, 2);

    // 'k' for up
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
        &state,
    );
    assert_eq!(pane.scroll_offset, 1);
}
