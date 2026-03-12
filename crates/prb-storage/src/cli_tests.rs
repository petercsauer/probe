//! CLI integration tests for MCAP storage.

#[cfg(test)]
mod tests {
    use crate::{SessionMetadata, SessionReader, SessionWriter};
    use prb_core::{DebugEvent, Direction, EventSource, Payload, Timestamp, TransportKind};
    use std::fs::File;
    use tempfile::TempDir;

    /// Helper to create a test `DebugEvent`.
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
    fn test_cli_ingest_output() {
        let tempdir = TempDir::new().unwrap();
        let session_path = tempdir.path().join("session.mcap");

        // Simulate CLI ingest command
        let metadata = SessionMetadata::new()
            .with_source_file("test.json")
            .with_capture_tool("json-fixture");

        let file = File::create(&session_path).unwrap();
        let mut writer = SessionWriter::new(file, metadata).unwrap();

        // Write some events
        for i in 0..10 {
            writer
                .write_event(&create_test_event("json-fixture", "test.json", i * 1000))
                .unwrap();
        }

        writer.finish().unwrap();

        // Verify the file was created and is valid
        let reader = SessionReader::open(&session_path).unwrap();
        let events: Vec<_> = reader.events().collect::<crate::Result<Vec<_>>>().unwrap();
        assert_eq!(events.len(), 10);
    }

    #[test]
    fn test_cli_inspect_mcap() {
        let tempdir = TempDir::new().unwrap();
        let session_path = tempdir.path().join("session.mcap");

        // Create a test MCAP file
        let metadata = SessionMetadata::new();
        let file = File::create(&session_path).unwrap();
        let mut writer = SessionWriter::new(file, metadata).unwrap();

        for i in 0..5 {
            writer
                .write_event(&create_test_event("test", "source.pcap", i * 1000))
                .unwrap();
        }

        writer.finish().unwrap();

        // Simulate CLI inspect command
        let reader = SessionReader::open(&session_path).unwrap();
        let events: Vec<_> = reader.events().collect::<crate::Result<Vec<_>>>().unwrap();

        assert_eq!(events.len(), 5);
        for (i, event) in events.iter().enumerate() {
            assert_eq!(event.timestamp.as_nanos(), i as u64 * 1000);
            assert_eq!(event.source.adapter, "test");
            assert_eq!(event.source.origin, "source.pcap");
        }
    }
}
