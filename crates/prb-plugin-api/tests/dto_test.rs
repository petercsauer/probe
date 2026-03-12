//! Tests for DTO types.

use prb_plugin_api::dto::*;
use std::collections::HashMap;

#[test]
fn test_correlation_key_dto_serde() {
    let key = CorrelationKeyDto {
        kind: "stream_id".to_string(),
        value: "12345".to_string(),
    };

    let json = serde_json::to_string(&key).expect("serialize");
    let deserialized: CorrelationKeyDto = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.kind, "stream_id");
    assert_eq!(deserialized.value, "12345");
}

#[test]
fn test_debug_event_dto_minimal() {
    let dto = DebugEventDto::minimal("grpc", "request");

    assert_eq!(dto.transport, "grpc");
    assert_eq!(dto.direction, "request");
    assert_eq!(dto.timestamp_nanos, 0);
    assert!(dto.payload_raw.is_none());
    assert!(dto.payload_decoded.is_none());
    assert!(dto.schema_name.is_none());
    assert!(dto.metadata.is_empty());
    assert!(dto.correlation_keys.is_empty());
    assert!(dto.warnings.is_empty());
}

#[test]
fn test_debug_event_dto_full_serde() {
    let mut metadata = HashMap::new();
    metadata.insert("key".to_string(), "value".to_string());

    let correlation_keys = vec![
        CorrelationKeyDto {
            kind: "stream_id".to_string(),
            value: "123".to_string(),
        },
        CorrelationKeyDto {
            kind: "topic".to_string(),
            value: "test.topic".to_string(),
        },
    ];

    let dto = DebugEventDto {
        timestamp_nanos: 1_234_567_890,
        transport: "zmtp".to_string(),
        direction: "publish".to_string(),
        payload_raw: Some(vec![1, 2, 3, 4]),
        payload_decoded: Some(serde_json::json!({"field": "value"})),
        schema_name: Some("test.Schema".to_string()),
        metadata,
        correlation_keys,
        warnings: vec!["warning1".to_string()],
        src_addr: Some("192.168.1.1:8080".to_string()),
        dst_addr: Some("192.168.1.2:9090".to_string()),
    };

    let json = serde_json::to_string(&dto).expect("serialize");
    let deserialized: DebugEventDto = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.timestamp_nanos, 1_234_567_890);
    assert_eq!(deserialized.transport, "zmtp");
    assert_eq!(deserialized.direction, "publish");
    assert_eq!(deserialized.payload_raw, Some(vec![1, 2, 3, 4]));
    assert_eq!(deserialized.schema_name, Some("test.Schema".to_string()));
    assert_eq!(deserialized.correlation_keys.len(), 2);
    assert_eq!(deserialized.warnings.len(), 1);
}

#[test]
fn test_debug_event_dto_optional_fields_omitted() {
    let dto = DebugEventDto::minimal("tcp", "inbound");

    let json = serde_json::to_string(&dto).expect("serialize");

    // Optional fields should not be present in JSON
    assert!(!json.contains("payload_raw"));
    assert!(!json.contains("payload_decoded"));
    assert!(!json.contains("schema_name"));
    assert!(!json.contains("src_addr"));
    assert!(!json.contains("dst_addr"));
}

#[test]
fn test_debug_event_dto_with_raw_payload_only() {
    let mut dto = DebugEventDto::minimal("grpc", "response");
    dto.payload_raw = Some(vec![0xDE, 0xAD, 0xBE, 0xEF]);

    let json = serde_json::to_string(&dto).expect("serialize");
    let deserialized: DebugEventDto = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.payload_raw, Some(vec![0xDE, 0xAD, 0xBE, 0xEF]));
    assert!(deserialized.payload_decoded.is_none());
}

#[test]
fn test_debug_event_dto_with_decoded_payload_only() {
    let mut dto = DebugEventDto::minimal("rtps", "subscribe");
    dto.payload_decoded = Some(serde_json::json!({
        "message_id": 42,
        "data": "test data"
    }));
    dto.schema_name = Some("MyMessage".to_string());

    let json = serde_json::to_string(&dto).expect("serialize");
    let deserialized: DebugEventDto = serde_json::from_str(&json).expect("deserialize");

    assert!(deserialized.payload_raw.is_none());
    assert!(deserialized.payload_decoded.is_some());
    assert_eq!(deserialized.schema_name, Some("MyMessage".to_string()));

    let decoded = deserialized.payload_decoded.unwrap();
    assert_eq!(decoded["message_id"], 42);
}

#[test]
fn test_correlation_keys_various_kinds() {
    let keys = vec![
        CorrelationKeyDto {
            kind: "stream_id".to_string(),
            value: "1".to_string(),
        },
        CorrelationKeyDto {
            kind: "topic".to_string(),
            value: "events".to_string(),
        },
        CorrelationKeyDto {
            kind: "connection_id".to_string(),
            value: "conn-abc-123".to_string(),
        },
        CorrelationKeyDto {
            kind: "trace_context".to_string(),
            value: "trace123:span456".to_string(),
        },
        CorrelationKeyDto {
            kind: "custom_key".to_string(),
            value: "custom_value".to_string(),
        },
    ];

    let json = serde_json::to_string(&keys).expect("serialize");
    let deserialized: Vec<CorrelationKeyDto> = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.len(), 5);
    assert_eq!(deserialized[0].kind, "stream_id");
    assert_eq!(deserialized[3].kind, "trace_context");
    assert_eq!(deserialized[4].value, "custom_value");
}

#[test]
fn test_correlation_key_dto_clone() {
    let key = CorrelationKeyDto {
        kind: "stream_id".to_string(),
        value: "12345".to_string(),
    };

    let cloned = key.clone();
    assert_eq!(cloned.kind, key.kind);
    assert_eq!(cloned.value, key.value);
}

#[test]
fn test_correlation_key_dto_debug() {
    let key = CorrelationKeyDto {
        kind: "test".to_string(),
        value: "value".to_string(),
    };

    let debug_str = format!("{:?}", key);
    assert!(debug_str.contains("CorrelationKeyDto"));
    assert!(debug_str.contains("test"));
    assert!(debug_str.contains("value"));
}

#[test]
fn test_debug_event_dto_clone() {
    let dto = DebugEventDto::minimal("grpc", "request");
    let cloned = dto.clone();

    assert_eq!(cloned.transport, dto.transport);
    assert_eq!(cloned.direction, dto.direction);
    assert_eq!(cloned.timestamp_nanos, dto.timestamp_nanos);
}

#[test]
fn test_debug_event_dto_debug() {
    let dto = DebugEventDto::minimal("zmtp", "publish");
    let debug_str = format!("{:?}", dto);

    assert!(debug_str.contains("DebugEventDto"));
    assert!(debug_str.contains("zmtp"));
    assert!(debug_str.contains("publish"));
}

#[test]
fn test_debug_event_dto_empty_strings() {
    let dto = DebugEventDto::minimal("", "");

    assert_eq!(dto.transport, "");
    assert_eq!(dto.direction, "");

    let json = serde_json::to_string(&dto).expect("serialize");
    let deserialized: DebugEventDto = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.transport, "");
    assert_eq!(deserialized.direction, "");
}

#[test]
fn test_debug_event_dto_with_warnings() {
    let mut dto = DebugEventDto::minimal("rtps", "subscribe");
    dto.warnings = vec![
        "Warning 1".to_string(),
        "Warning 2".to_string(),
        "Warning 3".to_string(),
    ];

    let json = serde_json::to_string(&dto).expect("serialize");
    let deserialized: DebugEventDto = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.warnings.len(), 3);
    assert_eq!(deserialized.warnings[0], "Warning 1");
    assert_eq!(deserialized.warnings[2], "Warning 3");
}

#[test]
fn test_debug_event_dto_large_payload() {
    let large_payload = vec![0u8; 10000];
    let mut dto = DebugEventDto::minimal("grpc", "request");
    dto.payload_raw = Some(large_payload.clone());

    let json = serde_json::to_string(&dto).expect("serialize");
    let deserialized: DebugEventDto = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.payload_raw.unwrap().len(), 10000);
}

#[test]
fn test_correlation_key_dto_special_characters() {
    let key = CorrelationKeyDto {
        kind: "test-key_with.special/chars".to_string(),
        value: "value:with=special&chars".to_string(),
    };

    let json = serde_json::to_string(&key).expect("serialize");
    let deserialized: CorrelationKeyDto = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.kind, "test-key_with.special/chars");
    assert_eq!(deserialized.value, "value:with=special&chars");
}
