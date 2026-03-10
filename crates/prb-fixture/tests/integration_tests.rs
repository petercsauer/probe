//! Integration tests for prb-fixture JSON adapter.

use prb_core::{CaptureAdapter, TransportKind, Direction, Payload};
use prb_fixture::JsonFixtureAdapter;
use camino::Utf8PathBuf;
use std::path::PathBuf;

/// Helper to get fixture path from workspace root
fn fixture_path(name: &str) -> Utf8PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    let path = workspace_root.join("fixtures").join(name);
    Utf8PathBuf::from_path_buf(path).expect("valid UTF-8 path")
}

#[test]
fn test_capture_adapter_trait_object_safe() {
    // Verify CaptureAdapter is dyn-safe by constructing a trait object
    let adapter = JsonFixtureAdapter::new(fixture_path("empty.json"));
    let _boxed: Box<dyn CaptureAdapter> = Box::new(adapter);
}

#[test]
fn test_fixture_parse_grpc_sample() {
    let mut adapter = JsonFixtureAdapter::new(fixture_path("grpc_sample.json"));
    let events: Vec<_> = adapter.ingest().collect::<Result<Vec<_>, _>>().unwrap();

    assert_eq!(events.len(), 2, "Expected 2 events in grpc_sample.json");

    // Verify first event
    assert_eq!(events[0].transport, TransportKind::Grpc);
    assert_eq!(events[0].direction, Direction::Outbound);

    // Verify second event
    assert_eq!(events[1].transport, TransportKind::Grpc);
    assert_eq!(events[1].direction, Direction::Inbound);
}

#[test]
fn test_fixture_parse_multi_transport() {
    let mut adapter = JsonFixtureAdapter::new(fixture_path("multi_transport.json"));
    let events: Vec<_> = adapter.ingest().collect::<Result<Vec<_>, _>>().unwrap();

    assert_eq!(events.len(), 4, "Expected 4 events in multi_transport.json");

    // Verify transport types
    assert_eq!(events[0].transport, TransportKind::Grpc);
    assert_eq!(events[1].transport, TransportKind::Zmq);
    assert_eq!(events[2].transport, TransportKind::DdsRtps);
    assert_eq!(events[3].transport, TransportKind::RawTcp);
}

#[test]
fn test_fixture_parse_empty() {
    let mut adapter = JsonFixtureAdapter::new(fixture_path("empty.json"));
    let events: Vec<_> = adapter.ingest().collect::<Result<Vec<_>, _>>().unwrap();

    assert_eq!(events.len(), 0, "Expected 0 events in empty.json");
}

#[test]
fn test_fixture_parse_malformed() {
    let mut adapter = JsonFixtureAdapter::new(fixture_path("malformed.json"));
    let result: Result<Vec<_>, _> = adapter.ingest().collect();

    assert!(result.is_err(), "Expected parse error for malformed.json");
}

#[test]
fn test_fixture_unsupported_version() {
    // Create a temporary fixture with version 99
    use std::io::Write;
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("version99.json");
    let mut file = std::fs::File::create(&path).unwrap();
    write!(file, r#"{{"version": 99, "events": []}}"#).unwrap();

    let utf8_path = Utf8PathBuf::from_path_buf(path).unwrap();
    let mut adapter = JsonFixtureAdapter::new(utf8_path);
    let result: Result<Vec<_>, _> = adapter.ingest().collect();

    assert!(result.is_err(), "Expected unsupported version error");
    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(err_msg.contains("version") || err_msg.contains("99"),
            "Error should mention version: {}", err_msg);
}

#[test]
fn test_fixture_missing_payload() {
    // Create fixture with event missing both payload fields
    use std::io::Write;
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("missing_payload.json");
    let mut file = std::fs::File::create(&path).unwrap();
    write!(file, r#"{{
        "version": 1,
        "events": [{{
            "timestamp_ns": 1700000000000000000,
            "transport": "grpc",
            "direction": "outbound"
        }}]
    }}"#).unwrap();

    let utf8_path = Utf8PathBuf::from_path_buf(path).unwrap();
    let mut adapter = JsonFixtureAdapter::new(utf8_path);
    let result: Result<Vec<_>, _> = adapter.ingest().collect();

    assert!(result.is_err(), "Expected error for missing payload");
}

#[test]
fn test_fixture_both_payloads() {
    // Create fixture with event having both payload fields
    use std::io::Write;
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("both_payloads.json");
    let mut file = std::fs::File::create(&path).unwrap();
    write!(file, r#"{{
        "version": 1,
        "events": [{{
            "timestamp_ns": 1700000000000000000,
            "transport": "grpc",
            "direction": "outbound",
            "payload_base64": "dGVzdA==",
            "payload_utf8": "test"
        }}]
    }}"#).unwrap();

    let utf8_path = Utf8PathBuf::from_path_buf(path).unwrap();
    let mut adapter = JsonFixtureAdapter::new(utf8_path);
    let result: Result<Vec<_>, _> = adapter.ingest().collect();

    assert!(result.is_err(), "Expected error for both payload fields");
}

#[test]
fn test_fixture_base64_decode() {
    let mut adapter = JsonFixtureAdapter::new(fixture_path("grpc_sample.json"));
    let events: Vec<_> = adapter.ingest().collect::<Result<Vec<_>, _>>().unwrap();

    // First event has base64 payload "CgRKb2huEAE="
    let payload = &events[0].payload;
    match payload {
        Payload::Raw { raw } => {
            assert!(!raw.is_empty(), "Payload should not be empty");
        }
        Payload::Decoded { raw, .. } => {
            assert!(!raw.is_empty(), "Payload should not be empty");
        }
    }
}

#[test]
fn test_fixture_utf8_payload() {
    let mut adapter = JsonFixtureAdapter::new(fixture_path("multi_transport.json"));
    let events: Vec<_> = adapter.ingest().collect::<Result<Vec<_>, _>>().unwrap();

    // Event at index 1 has UTF-8 payload "test message"
    let payload = &events[1].payload;
    match payload {
        Payload::Raw { raw } => {
            let text = std::str::from_utf8(raw).unwrap();
            assert_eq!(text, "test message");
        }
        Payload::Decoded { raw, .. } => {
            let text = std::str::from_utf8(raw).unwrap();
            assert_eq!(text, "test message");
        }
    }
}

#[test]
fn test_fixture_metadata_preserved() {
    let mut adapter = JsonFixtureAdapter::new(fixture_path("grpc_sample.json"));
    let events: Vec<_> = adapter.ingest().collect::<Result<Vec<_>, _>>().unwrap();

    // First event has metadata
    let metadata = &events[0].metadata;
    assert!(metadata.contains_key("grpc.method"), "Expected grpc.method metadata");
    assert_eq!(metadata.get("grpc.method").unwrap(), "/example.Service/GetUser");
    assert_eq!(metadata.get("h2.stream_id").unwrap(), "1");
}

#[test]
fn test_fixture_network_addr() {
    let mut adapter = JsonFixtureAdapter::new(fixture_path("grpc_sample.json"));
    let events: Vec<_> = adapter.ingest().collect::<Result<Vec<_>, _>>().unwrap();

    // First event has source addresses
    let source = &events[0].source;
    assert!(source.network.is_some(), "Expected network addresses");

    let network = source.network.as_ref().unwrap();
    assert_eq!(network.src, "10.0.0.1:52341");
    assert_eq!(network.dst, "10.0.0.2:8080");
}

#[test]
fn test_fixture_adapter_name() {
    let adapter = JsonFixtureAdapter::new(fixture_path("empty.json"));
    assert_eq!(adapter.name(), "json-fixture");
}

#[test]
fn test_fixture_io_error_nonexistent_file() {
    let mut adapter = JsonFixtureAdapter::new(Utf8PathBuf::from("/nonexistent/path/fixture.json"));
    let result: Result<Vec<_>, _> = adapter.ingest().collect();

    assert!(result.is_err(), "Expected I/O error for nonexistent file");
}

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;
    use prb_fixture::FixtureEvent;
    use std::collections::BTreeMap;

    /// Generate arbitrary FixtureEvent for property testing
    fn arb_fixture_event() -> impl Strategy<Value = FixtureEvent> {
        (
            any::<u64>(),
            prop::sample::select(vec!["grpc", "zmq", "dds-rtps", "tcp", "udp"]),
            prop::sample::select(vec!["inbound", "outbound", "unknown"]),
            prop::option::of("[a-zA-Z0-9+/=]{1,100}"),
            prop::option::of("[a-zA-Z0-9 ]{1,100}"),
        ).prop_map(|(ts, transport, direction, b64, utf8)| {
            // Ensure only one payload type
            let (payload_base64, payload_utf8) = match (b64, utf8) {
                (Some(b), None) => (Some(b), None),
                (None, Some(u)) => (None, Some(u)),
                (Some(b), Some(_)) => (Some(b), None), // Prefer base64
                (None, None) => (Some("dGVzdA==".to_string()), None), // Default valid base64
            };

            FixtureEvent {
                timestamp_ns: ts,
                transport: transport.to_string(),
                direction: direction.to_string(),
                payload_base64,
                payload_utf8,
                metadata: BTreeMap::new(),
                source: None,
            }
        })
    }

    proptest! {
        #[test]
        fn test_fixture_event_arbitrary(event in arb_fixture_event()) {
            // This test verifies that arbitrary FixtureEvents don't panic during processing
            // We create a temporary fixture file and try to parse it
            use std::io::Write;

            let temp_dir = tempfile::tempdir().unwrap();
            let path = temp_dir.path().join("proptest.json");
            let mut file = std::fs::File::create(&path).unwrap();

            let fixture_json = serde_json::json!({
                "version": 1,
                "events": [event]
            });

            write!(file, "{}", fixture_json).unwrap();
            drop(file);

            let utf8_path = Utf8PathBuf::from_path_buf(path).unwrap();
            let mut adapter = JsonFixtureAdapter::new(utf8_path);

            // Try to ingest - should not panic even if it errors
            let _result: Result<Vec<_>, _> = adapter.ingest().collect();
        }
    }
}
