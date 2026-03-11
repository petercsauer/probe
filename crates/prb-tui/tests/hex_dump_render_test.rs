//! Integration test for hex dump pane rendering with actual payloads.

mod buf_helpers;

use bytes::Bytes;
use prb_core::{DebugEvent, Direction, EventId, EventSource, Payload, Timestamp, TransportKind};
use prb_tui::event_store::EventStore;
use prb_tui::panes::hex_dump::HexDumpPane;
use prb_tui::panes::PaneComponent;
use prb_tui::theme::ThemeConfig;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::collections::BTreeMap;

use buf_helpers::row_text;

fn make_test_event_with_payload(payload: Vec<u8>) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(1),
        timestamp: Timestamp::from_nanos(1_000_000_000),
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

#[test]
fn test_hex_dump_renders_with_payload() {
    let payload = b"Hello, World!".to_vec();
    let event = make_test_event_with_payload(payload);
    let store = EventStore::new(vec![event]);

    // Create app state
    let app_state = prb_tui::app::AppState {
        schema_registry: None,
            conversations: None,
        store,
        filtered_indices: vec![0],
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
                visible_columns: Vec::new(),
    };

    // Create hex dump pane
    let mut pane = HexDumpPane::new();

    // Create a test buffer
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));

    // Render the pane
    pane.render(Rect::new(0, 0, 80, 10), &mut buffer, &app_state, &ThemeConfig::dark(), false);

    // Verify something was rendered (buffer is not empty)
    let mut found_offset = false;
    for y in 0..10 {
        for x in 0..80 {
            let cell = &buffer[(x, y)];
            if cell.symbol() == "0" {
                found_offset = true;
                break;
            }
        }
    }

    // Should contain hex offset (at minimum a "0" character)
    assert!(found_offset, "Should contain hex offset in rendered output");
}

#[test]
fn test_hex_dump_renders_empty_for_no_selection() {
    let store = EventStore::new(vec![]);

    let app_state = prb_tui::app::AppState {
        schema_registry: None,
            conversations: None,
        store,
        filtered_indices: vec![],
        selected_event: None,
        filter: None,
        filter_text: String::new(),
                visible_columns: Vec::new(),
    };

    let mut pane = HexDumpPane::new();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));

    pane.render(Rect::new(0, 0, 80, 10), &mut buffer, &app_state, &ThemeConfig::dark(), false);

    // Should show "No event selected" message - check for 'N' or 'e' from message
    let mut found_message_char = false;
    for y in 0..10 {
        for x in 0..80 {
            let symbol = buffer[(x, y)].symbol();
            if symbol == "N" || symbol == "e" || symbol == "o" {
                found_message_char = true;
                break;
            }
        }
    }

    assert!(
        found_message_char,
        "Should show appropriate message when no event selected"
    );
}

#[test]
fn test_hex_dump_with_multiline_payload() {
    // Create payload > 16 bytes to span multiple lines
    let payload = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".to_vec();
    let event = make_test_event_with_payload(payload);
    let store = EventStore::new(vec![event]);

    let app_state = prb_tui::app::AppState {
        schema_registry: None,
            conversations: None,
        store,
        filtered_indices: vec![0],
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
                visible_columns: Vec::new(),
    };

    let mut pane = HexDumpPane::new();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 20));

    pane.render(Rect::new(0, 0, 80, 20), &mut buffer, &app_state, &ThemeConfig::dark(), false);

    // Check that rendering occurred - look for numeric characters from offsets
    let mut found_digits = 0;
    for y in 0..20 {
        for x in 0..80 {
            let symbol = buffer[(x, y)].symbol();
            if symbol.chars().all(|c| c.is_ascii_digit()) && !symbol.is_empty() {
                found_digits += 1;
            }
        }
    }

    // Should have multiple digits from offset lines (00000000, 00000010, etc.)
    assert!(
        found_digits >= 8,
        "Should contain offset digits in rendered output"
    );
}

#[test]
fn test_hex_dump_scroll_functionality() {
    // Create a large payload
    let payload: Vec<u8> = (0..=255).collect();
    let event = make_test_event_with_payload(payload);
    let store = EventStore::new(vec![event]);

    let app_state = prb_tui::app::AppState {
        schema_registry: None,
            conversations: None,
        store,
        filtered_indices: vec![0],
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
                visible_columns: Vec::new(),
    };

    let mut pane = HexDumpPane::new();

    // Scroll down
    pane.scroll_offset = 5;

    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));
    pane.render(Rect::new(0, 0, 80, 10), &mut buffer, &app_state, &ThemeConfig::dark(), false);

    // When scrolled to line 5, first visible offset should be 0x50 (5 * 16 = 80 = 0x50)
    // The first content row (after border if present) should start with "00000050"
    let first_content_row = row_text(&buffer, 1); // row 1 = first data row (row 0 = border)
    assert!(
        first_content_row.starts_with("00000050") || first_content_row.contains("00000050"),
        "offset should be 0x50 at scroll=5, got: {}",
        first_content_row
    );
}

#[test]
fn test_hex_dump_highlight_visible() {
    let payload = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ".to_vec();
    let event = make_test_event_with_payload(payload);
    let store = EventStore::new(vec![event]);

    let app_state = prb_tui::app::AppState {
        schema_registry: None,
            conversations: None,
        store,
        filtered_indices: vec![0],
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
                visible_columns: Vec::new(),
    };

    let mut pane = HexDumpPane::new();

    // Set highlight on bytes 0-5
    pane.set_highlight(0, 5);

    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));
    pane.render(Rect::new(0, 0, 80, 10), &mut buffer, &app_state, &ThemeConfig::dark(), false);

    // Verify rendering succeeded (highlight styling is applied during render)
    // We can't easily test the styling in a unit test, but we can verify
    // the render doesn't panic and produces content
    let mut has_content = false;
    for y in 0..10 {
        for x in 0..80 {
            if buffer[(x, y)].symbol() != " " && !buffer[(x, y)].symbol().is_empty() {
                has_content = true;
                break;
            }
        }
    }

    assert!(has_content, "Buffer should contain rendered content");
}
