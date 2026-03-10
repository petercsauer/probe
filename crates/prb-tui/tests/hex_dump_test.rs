//! Standalone tests for hex dump pane functionality (S10).

use bytes::Bytes;
use prb_core::{DebugEvent, Direction, EventId, EventSource, Payload, Timestamp, TransportKind};
use prb_tui::event_store::EventStore;
use prb_tui::panes::hex_dump::HexDumpPane;
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

#[test]
fn test_hex_dump_pane_creation() {
    let pane = HexDumpPane::new();
    assert_eq!(pane.scroll_offset, 0);
    assert_eq!(pane.highlight, None);
}

#[test]
fn test_set_highlight() {
    let mut pane = HexDumpPane::new();

    pane.set_highlight(32, 16);

    assert_eq!(pane.highlight, Some((32, 16)));
    assert_eq!(pane.scroll_offset, 2, "Should scroll to line containing offset 32");
}

#[test]
fn test_set_highlight_auto_scroll() {
    let mut pane = HexDumpPane::new();

    // Highlight at byte 256 (line 16: 256 / 16 = 16)
    pane.set_highlight(256, 8);

    assert_eq!(pane.scroll_offset, 16);

    // Highlight at byte 0 (line 0)
    pane.set_highlight(0, 8);

    assert_eq!(pane.scroll_offset, 0);
}

#[test]
fn test_clear_highlight() {
    let mut pane = HexDumpPane::new();

    pane.set_highlight(100, 50);
    assert!(pane.highlight.is_some());

    pane.clear_highlight();
    assert!(pane.highlight.is_none());
}

#[test]
fn test_cross_highlighting_integration() {
    // This test verifies that the hex dump pane correctly stores
    // highlight information that would come from decode tree selection

    let mut pane = HexDumpPane::new();

    // Simulate decode tree selecting a field at bytes 8-16
    pane.set_highlight(8, 8);

    assert_eq!(pane.highlight, Some((8, 8)));

    // Clear when decode tree selection changes
    pane.clear_highlight();

    assert_eq!(pane.highlight, None);
}

#[test]
fn test_hex_dump_with_payload() {
    // Create test event with payload
    let payload = b"Hello, World! This is a test payload.".to_vec();
    let event = make_test_event(payload.clone());

    let store = EventStore::new(vec![event]);

    // Verify event was stored correctly
    assert_eq!(store.len(), 1);

    let retrieved = store.get(0).unwrap();
    match &retrieved.payload {
        Payload::Raw { raw } => {
            assert_eq!(raw.as_ref(), payload.as_slice());
        }
        _ => panic!("Expected Raw payload"),
    }
}

#[test]
fn test_hex_dump_16_bytes_per_line() {
    // Verify the hex dump format uses 16 bytes per line
    // This is implicitly tested by the set_highlight auto-scroll logic

    let mut pane = HexDumpPane::new();

    // Line 0: bytes 0-15
    pane.set_highlight(0, 1);
    assert_eq!(pane.scroll_offset, 0);

    // Line 1: bytes 16-31
    pane.set_highlight(16, 1);
    assert_eq!(pane.scroll_offset, 1);

    // Line 2: bytes 32-47
    pane.set_highlight(32, 1);
    assert_eq!(pane.scroll_offset, 2);
}

#[test]
fn test_hex_dump_large_offset() {
    let mut pane = HexDumpPane::new();

    // Test with a large offset (4KB)
    pane.set_highlight(4096, 32);

    assert_eq!(pane.scroll_offset, 256); // 4096 / 16 = 256
    assert_eq!(pane.highlight, Some((4096, 32)));
}

#[test]
fn test_hex_dump_empty_payload() {
    let event = make_test_event(vec![]);
    let store = EventStore::new(vec![event]);

    assert_eq!(store.len(), 1);

    let retrieved = store.get(0).unwrap();
    match &retrieved.payload {
        Payload::Raw { raw } => {
            assert!(raw.is_empty());
        }
        _ => panic!("Expected Raw payload"),
    }
}
