//! Tests for event types.

use super::*;
use bytes::Bytes;

#[test]
fn test_debug_event_serde_roundtrip() {
    let event = DebugEvent::builder()
        .id(EventId::from_raw(42))
        .timestamp(Timestamp::from_nanos(1678901234567890123))
        .source(EventSource {
            adapter: "test-adapter".to_string(),
            origin: "test-origin".to_string(),
            network: Some(NetworkAddr {
                src: "192.168.1.1:8080".to_string(),
                dst: "192.168.1.2:9090".to_string(),
            }),
        })
        .transport(TransportKind::Grpc)
        .direction(Direction::Inbound)
        .payload(Payload::Decoded {
            raw: Bytes::from_static(b"test data"),
            fields: serde_json::json!({ "field1": "value1" }),
            schema_name: Some("TestSchema".to_string()),
        })
        .metadata("key1", "value1")
        .correlation_key(CorrelationKey::StreamId { id: 42 })
        .sequence(1)
        .warning("test warning")
        .build();

    let json = serde_json::to_string(&event).expect("failed to serialize");
    let deserialized: DebugEvent = serde_json::from_str(&json).expect("failed to deserialize");

    assert_eq!(event, deserialized);
}

#[test]
fn test_timestamp_nanosecond_precision() {
    let nanos = 1678901234567890123u64;
    let ts = Timestamp::from_nanos(nanos);
    assert_eq!(ts.as_nanos(), nanos);

    // Test serde roundtrip preserves nanoseconds
    let json = serde_json::to_string(&ts).expect("failed to serialize");
    let deserialized: Timestamp = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(deserialized.as_nanos(), nanos);
}

#[test]
fn test_payload_raw_serde() {
    let raw_data = Bytes::from_static(b"hello world");
    let payload = Payload::Raw {
        raw: raw_data.clone(),
    };

    let json = serde_json::to_string(&payload).expect("failed to serialize");
    let deserialized: Payload = serde_json::from_str(&json).expect("failed to deserialize");

    match deserialized {
        Payload::Raw { raw } => assert_eq!(raw, raw_data),
        _ => panic!("expected Raw payload"),
    }
}

#[test]
fn test_payload_decoded_serde() {
    let raw_data = Bytes::from_static(b"encoded data");
    let fields = serde_json::json!({
        "field1": "value1",
        "field2": 42,
        "nested": { "inner": true }
    });
    let payload = Payload::Decoded {
        raw: raw_data.clone(),
        fields: fields.clone(),
        schema_name: Some("TestSchema".to_string()),
    };

    let json = serde_json::to_string(&payload).expect("failed to serialize");
    let deserialized: Payload = serde_json::from_str(&json).expect("failed to deserialize");

    match deserialized {
        Payload::Decoded {
            raw,
            fields: deserialized_fields,
            schema_name,
        } => {
            assert_eq!(raw, raw_data);
            assert_eq!(deserialized_fields, fields);
            assert_eq!(schema_name, Some("TestSchema".to_string()));
        }
        _ => panic!("expected Decoded payload"),
    }
}

#[test]
fn test_transport_kind_display() {
    assert_eq!(TransportKind::Grpc.to_string(), "gRPC");
    assert_eq!(TransportKind::Zmq.to_string(), "ZMQ");
    assert_eq!(TransportKind::DdsRtps.to_string(), "DDS-RTPS");
    assert_eq!(TransportKind::RawTcp.to_string(), "TCP");
    assert_eq!(TransportKind::RawUdp.to_string(), "UDP");
    assert_eq!(TransportKind::JsonFixture.to_string(), "JSON-Fixture");
}

#[test]
fn test_direction_display() {
    assert_eq!(Direction::Inbound.to_string(), "inbound");
    assert_eq!(Direction::Outbound.to_string(), "outbound");
    assert_eq!(Direction::Unknown.to_string(), "unknown");
}

#[test]
fn test_core_error_display() {
    let error = CoreError::InvalidTimestamp("bad timestamp".to_string());
    assert_eq!(error.to_string(), "invalid timestamp: bad timestamp");

    let error = CoreError::PayloadDecode("decode failed".to_string());
    assert_eq!(error.to_string(), "payload decode failed: decode failed");

    let error = CoreError::UnsupportedTransport("unknown".to_string());
    assert_eq!(error.to_string(), "unsupported transport: unknown");
}

#[test]
fn test_core_error_source_chain() {
    // Test that serde_json::Error properly chains through CoreError::Serialization
    let invalid_json = "{invalid json}";
    let result: Result<serde_json::Value, _> = serde_json::from_str(invalid_json);
    assert!(result.is_err());

    let serde_error = result.unwrap_err();
    let core_error = CoreError::from(serde_error);

    // Verify the error can be displayed
    assert!(core_error.to_string().contains("serialization error"));

    // Verify the source chain is preserved
    use std::error::Error;
    assert!(core_error.source().is_some());
}

#[test]
fn test_event_id_monotonic() {
    let id1 = EventId::next();
    let id2 = EventId::next();
    let id3 = EventId::next();

    assert!(id1.as_u64() < id2.as_u64());
    assert!(id2.as_u64() < id3.as_u64());
}

#[test]
fn test_correlation_key_variants() {
    let keys = vec![
        CorrelationKey::StreamId { id: 42 },
        CorrelationKey::Topic {
            name: "test.topic".to_string(),
        },
        CorrelationKey::ConnectionId {
            id: "conn-123".to_string(),
        },
        CorrelationKey::Custom {
            key: "custom-key".to_string(),
            value: "custom-value".to_string(),
        },
    ];

    for key in keys {
        let json = serde_json::to_string(&key).expect("failed to serialize");
        let deserialized: CorrelationKey =
            serde_json::from_str(&json).expect("failed to deserialize");
        assert_eq!(key, deserialized);
    }
}
