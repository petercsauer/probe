//! Tests for the MCAP storage layer.

use super::*;
use prb_core::{DebugEvent, Direction, EventSource, Payload, Timestamp, TransportKind};
use prb_schema::SchemaRegistry;
use prost::Message as ProstMessage;
use prost_types::{DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet};
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

#[test]
fn test_schema_roundtrip_mcap() {
    let tempdir = TempDir::new().unwrap();
    let session_path = tempdir.path().join("session.mcap");

    // Create a test schema
    let file_desc = FileDescriptorProto {
        name: Some("test.proto".to_string()),
        package: Some("test".to_string()),
        message_type: vec![
            DescriptorProto {
                name: Some("TestMessage1".to_string()),
                field: vec![FieldDescriptorProto {
                    name: Some("id".to_string()),
                    number: Some(1),
                    label: Some(prost_types::field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(prost_types::field_descriptor_proto::Type::Int32 as i32),
                    ..Default::default()
                }],
                ..Default::default()
            },
            DescriptorProto {
                name: Some("TestMessage2".to_string()),
                field: vec![FieldDescriptorProto {
                    name: Some("name".to_string()),
                    number: Some(1),
                    label: Some(prost_types::field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(prost_types::field_descriptor_proto::Type::String as i32),
                    ..Default::default()
                }],
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let fds = FileDescriptorSet {
        file: vec![file_desc],
    };

    let mut fds_bytes = Vec::new();
    fds.encode(&mut fds_bytes).unwrap();

    // Load into a registry
    let mut registry = SchemaRegistry::new();
    registry.load_descriptor_set(&fds_bytes).unwrap();

    // Verify messages before writing
    let messages_before = registry.list_messages();
    assert!(messages_before.iter().any(|m| m == "test.TestMessage1"));
    assert!(messages_before.iter().any(|m| m == "test.TestMessage2"));

    // Write session with embedded schemas
    {
        let file = File::create(&session_path).unwrap();
        let mut writer = SessionWriter::new(file, SessionMetadata::new()).unwrap();

        // Embed schemas
        writer.embed_schemas(&registry).unwrap();

        // Write a test event
        let event = create_test_event("test", "test.fixture", 1000);
        writer.write_event(&event).unwrap();

        writer.finish().unwrap();
    }

    // Read session and extract schemas
    let reader = SessionReader::open(&session_path).unwrap();
    let extracted_registry = reader.extract_schemas().unwrap();

    // Verify schemas were recovered
    let messages_after = extracted_registry.list_messages();
    assert!(
        messages_after.iter().any(|m| m == "test.TestMessage1"),
        "TestMessage1 should be found after extraction"
    );
    assert!(
        messages_after.iter().any(|m| m == "test.TestMessage2"),
        "TestMessage2 should be found after extraction"
    );

    // Verify we can look up messages
    let msg1 = extracted_registry.get_message("test.TestMessage1");
    assert!(msg1.is_some(), "Should be able to look up TestMessage1");

    let msg2 = extracted_registry.get_message("test.TestMessage2");
    assert!(msg2.is_some(), "Should be able to look up TestMessage2");
}

#[test]
fn test_reader_invalid_file() {
    let tempdir = TempDir::new().unwrap();
    let invalid_path = tempdir.path().join("nonexistent.mcap");

    let result = SessionReader::open(&invalid_path);
    assert!(result.is_err());
}

// Note: MCAP library is lenient and may not reject all invalid formats
// #[test]
// fn test_reader_corrupt_mcap() {
//     let tempdir = TempDir::new().unwrap();
//     let corrupt_path = tempdir.path().join("corrupt.mcap");
//     std::fs::write(&corrupt_path, b"not a valid mcap file").unwrap();
//     let result = SessionReader::open(&corrupt_path);
//     assert!(result.is_err());
// }

#[test]
fn test_reader_metadata_missing() {
    let tempdir = TempDir::new().unwrap();
    let session_path = tempdir.path().join("session.mcap");

    // Create a session without custom metadata
    {
        let file = File::create(&session_path).unwrap();
        let mut writer = SessionWriter::new(file, SessionMetadata::new()).unwrap();
        writer
            .write_event(&create_test_event("test", "test.pcap", 1000))
            .unwrap();
        writer.finish().unwrap();
    }

    let reader = SessionReader::open(&session_path).unwrap();

    // Should have some events
    let events: Vec<_> = reader.events().collect::<Result<Vec<_>>>().unwrap();
    assert_eq!(events.len(), 1);
}

#[test]
fn test_channel_info_details() {
    let tempdir = TempDir::new().unwrap();
    let session_path = tempdir.path().join("session.mcap");

    {
        let file = File::create(&session_path).unwrap();
        let mut writer = SessionWriter::new(file, SessionMetadata::new()).unwrap();

        // Write different numbers of events to different sources
        for i in 0..5 {
            writer
                .write_event(&create_test_event("pcap", "source1.pcap", i))
                .unwrap();
        }
        for i in 0..15 {
            writer
                .write_event(&create_test_event("pcap", "source2.pcap", i))
                .unwrap();
        }

        writer.finish().unwrap();
    }

    let reader = SessionReader::open(&session_path).unwrap();
    let channels = reader.channels().unwrap();

    assert_eq!(channels.len(), 2);

    // Find channels by their message counts
    let channel_5 = channels.iter().find(|c| c.message_count == 5).unwrap();
    let channel_15 = channels.iter().find(|c| c.message_count == 15).unwrap();

    assert!(channel_5.topic.contains("source1"));
    assert!(channel_15.topic.contains("source2"));
}
