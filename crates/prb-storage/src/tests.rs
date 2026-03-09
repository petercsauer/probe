//! Tests for the MCAP storage layer.

use super::*;
use prb_core::{DebugEvent, Direction, EventSource, Payload, Timestamp, TransportKind};
use std::fs::File;
use tempfile::TempDir;

/// Helper to create a test DebugEvent.
fn create_test_event(adapter: &str, origin: &str, timestamp: u64) -> DebugEvent {
    DebugEvent::builder()
        .source(EventSource {
            adapter: adapter.to_string(),
            origin: origin.to_string(),
            network: None,
        })
        .transport(TransportKind::Grpc)
        .direction(Direction::Inbound)
        .payload(Payload::Raw {
            raw: bytes::Bytes::from_static(b"test payload"),
        })
        .timestamp(Timestamp::from_nanos(timestamp))
        .build()
}

#[test]
fn test_session_roundtrip() {
    let tempdir = TempDir::new().unwrap();
    let session_path = tempdir.path().join("session.mcap");

    // Write 100 events
    let metadata = SessionMetadata::new()
        .with_source_file("test.pcap")
        .with_capture_tool("tcpdump");

    let mut events_written = Vec::new();
    {
        let file = File::create(&session_path).unwrap();
        let mut writer = SessionWriter::new(file, metadata.clone()).unwrap();

        for i in 0..100 {
            let event = create_test_event("pcap", "test.pcap", i * 1000);
            events_written.push(event.clone());
            writer.write_event(&event).unwrap();
        }

        writer.finish().unwrap();
    }

    // Read back and verify
    let reader = SessionReader::open(&session_path).unwrap();
    let events_read: Vec<_> = reader.events().collect::<Result<Vec<_>>>().unwrap();

    assert_eq!(events_read.len(), 100);
    for (written, read) in events_written.iter().zip(events_read.iter()) {
        assert_eq!(written.timestamp, read.timestamp);
        assert_eq!(written.source.adapter, read.source.adapter);
        assert_eq!(written.source.origin, read.source.origin);
        assert_eq!(written.transport, read.transport);
        assert_eq!(written.direction, read.direction);
    }
}

#[test]
fn test_session_metadata() {
    let tempdir = TempDir::new().unwrap();
    let session_path = tempdir.path().join("session.mcap");

    let metadata = SessionMetadata::new()
        .with_source_file("test.pcap")
        .with_capture_tool("wireshark")
        .with_command_args("prb ingest test.pcap")
        .with_custom("custom_key", "custom_value");

    {
        let file = File::create(&session_path).unwrap();
        let writer = SessionWriter::new(file, metadata.clone()).unwrap();
        writer.finish().unwrap();
    }

    let reader = SessionReader::open(&session_path).unwrap();
    let read_metadata = reader.metadata().unwrap();

    assert!(read_metadata.is_some());
    let read_metadata = read_metadata.unwrap();
    assert_eq!(read_metadata.source_file, Some("test.pcap".to_string()));
    assert_eq!(read_metadata.capture_tool, Some("wireshark".to_string()));
    assert_eq!(
        read_metadata.command_args,
        Some("prb ingest test.pcap".to_string())
    );
    assert_eq!(
        read_metadata.custom.get("custom_key"),
        Some(&"custom_value".to_string())
    );
}

#[test]
fn test_multi_channel() {
    let tempdir = TempDir::new().unwrap();
    let session_path = tempdir.path().join("session.mcap");

    let metadata = SessionMetadata::new();

    {
        let file = File::create(&session_path).unwrap();
        let mut writer = SessionWriter::new(file, metadata).unwrap();

        // Write events from 3 different sources
        for i in 0..10 {
            writer
                .write_event(&create_test_event("pcap", "source1.pcap", i))
                .unwrap();
            writer
                .write_event(&create_test_event("pcap", "source2.pcap", i))
                .unwrap();
            writer
                .write_event(&create_test_event("fixture", "source3.json", i))
                .unwrap();
        }

        writer.finish().unwrap();
    }

    let reader = SessionReader::open(&session_path).unwrap();
    let channels = reader.channels().unwrap();

    // Should have 3 channels
    assert_eq!(channels.len(), 3);

    // Each channel should have 10 messages
    for channel in channels {
        assert_eq!(channel.message_count, 10);
    }

    // Total events should be 30
    let events: Vec<_> = reader.events().collect::<Result<Vec<_>>>().unwrap();
    assert_eq!(events.len(), 30);
}

#[test]
fn test_empty_session() {
    let tempdir = TempDir::new().unwrap();
    let session_path = tempdir.path().join("session.mcap");

    let metadata = SessionMetadata::new();

    {
        let file = File::create(&session_path).unwrap();
        let writer = SessionWriter::new(file, metadata).unwrap();
        writer.finish().unwrap();
    }

    let reader = SessionReader::open(&session_path).unwrap();
    let events: Vec<_> = reader.events().collect::<Result<Vec<_>>>().unwrap();
    assert_eq!(events.len(), 0);

    let channels = reader.channels().unwrap();
    assert_eq!(channels.len(), 0);
}

#[test]
fn test_large_session() {
    let tempdir = TempDir::new().unwrap();
    let session_path = tempdir.path().join("session.mcap");

    let metadata = SessionMetadata::new();

    {
        let file = File::create(&session_path).unwrap();
        let mut writer = SessionWriter::new(file, metadata).unwrap();

        for i in 0..10_000 {
            let event = create_test_event("pcap", "large.pcap", i);
            writer.write_event(&event).unwrap();
        }

        writer.finish().unwrap();
    }

    let reader = SessionReader::open(&session_path).unwrap();
    let events: Vec<_> = reader.events().collect::<Result<Vec<_>>>().unwrap();

    assert_eq!(events.len(), 10_000);

    // Verify ordering by timestamp
    for i in 0..events.len() - 1 {
        assert!(events[i].timestamp.as_nanos() <= events[i + 1].timestamp.as_nanos());
    }
}
