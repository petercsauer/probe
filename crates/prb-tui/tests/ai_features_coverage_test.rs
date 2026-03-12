//! Coverage tests for AI features: prompt building, response parsing, smart suggestions

use bytes::Bytes;
use prb_core::{
    DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind,
};
use std::collections::BTreeMap;

// Re-export the modules we're testing
use prb_tui::ai_features::AiFeatureError;
use prb_tui::ai_smart::{
    CaptureContext, build_anomaly_summary, parse_anomaly_response, parse_protocol_response,
};

/// Helper to create a test event with customizable metadata
fn create_test_event(
    id: u64,
    timestamp_nanos: u64,
    transport: TransportKind,
    metadata: BTreeMap<String, String>,
) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(timestamp_nanos),
        source: EventSource {
            adapter: "test".into(),
            origin: "test.mcap".into(),
            network: Some(NetworkAddr {
                src: "10.0.0.1:1234".to_string(),
                dst: "10.0.0.2:5678".to_string(),
            }),
        },
        transport,
        direction: Direction::Inbound,
        payload: Payload::Raw {
            raw: Bytes::from_static(b"test payload"),
        },
        metadata,
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}

// ============================================================================
// Priority 1: Prompt Building Tests (~150 lines)
// ============================================================================

#[test]
fn test_capture_context_build_with_multiple_transports() {
    let events = vec![
        create_test_event(1, 1_000_000_000, TransportKind::Grpc, BTreeMap::new()),
        create_test_event(2, 2_000_000_000, TransportKind::Zmq, BTreeMap::new()),
        create_test_event(3, 3_000_000_000, TransportKind::DdsRtps, BTreeMap::new()),
    ];

    let context = CaptureContext::build(&events);

    assert_eq!(context.total_events, 3);
    assert_eq!(context.transports.len(), 3);
    assert!(context.transports.contains(&TransportKind::Grpc));
    assert!(context.transports.contains(&TransportKind::Zmq));
    assert!(context.transports.contains(&TransportKind::DdsRtps));
}

#[test]
fn test_capture_context_build_with_metadata_fields() {
    let mut metadata1 = BTreeMap::new();
    metadata1.insert("grpc.method".to_string(), "/api/GetUser".to_string());
    metadata1.insert("grpc.status".to_string(), "0".to_string());

    let mut metadata2 = BTreeMap::new();
    metadata2.insert("grpc.method".to_string(), "/api/CreateUser".to_string());
    metadata2.insert("grpc.status".to_string(), "2".to_string());

    let events = vec![
        create_test_event(1, 1_000_000_000, TransportKind::Grpc, metadata1),
        create_test_event(2, 2_000_000_000, TransportKind::Grpc, metadata2),
    ];

    let context = CaptureContext::build(&events);

    assert!(
        context
            .available_fields
            .contains(&"grpc.method".to_string())
    );
    assert!(
        context
            .available_fields
            .contains(&"grpc.status".to_string())
    );

    // Check sample metadata
    assert!(context.sample_metadata.contains_key("grpc.method"));
    let methods = context.sample_metadata.get("grpc.method").unwrap();
    assert!(
        methods.contains(&"/api/GetUser".to_string())
            || methods.contains(&"/api/CreateUser".to_string())
    );
}

#[test]
fn test_capture_context_format_fields_includes_transport_and_direction() {
    let events = vec![create_test_event(
        1,
        1_000_000_000,
        TransportKind::Grpc,
        BTreeMap::new(),
    )];
    let context = CaptureContext::build(&events);

    let formatted = context.format_fields();

    assert!(formatted.contains("Available fields:"));
    assert!(formatted.contains("transport"));
    assert!(formatted.contains("direction"));
    assert!(formatted.contains("gRPC, ZMQ, DDS, TCP, UDP"));
    assert!(formatted.contains("Inbound, Outbound"));
}

#[test]
fn test_capture_context_format_fields_includes_metadata() {
    let mut metadata = BTreeMap::new();
    metadata.insert("grpc.method".to_string(), "/api/Test".to_string());
    metadata.insert("grpc.service".to_string(), "TestService".to_string());

    let events = vec![create_test_event(
        1,
        1_000_000_000,
        TransportKind::Grpc,
        metadata,
    )];
    let context = CaptureContext::build(&events);

    let formatted = context.format_fields();

    assert!(formatted.contains("grpc.method"));
    assert!(formatted.contains("grpc.service"));
    assert!(formatted.contains("\"/api/Test\""));
    assert!(formatted.contains("\"TestService\""));
}

#[test]
fn test_capture_context_limits_sample_values() {
    let mut metadata = BTreeMap::new();
    metadata.insert("test.field".to_string(), "value1".to_string());

    // Create 10 events with different values
    let mut events = Vec::new();
    for i in 1..=10 {
        let mut meta = BTreeMap::new();
        meta.insert("test.field".to_string(), format!("value{}", i));
        events.push(create_test_event(
            i,
            i * 1_000_000_000,
            TransportKind::Grpc,
            meta,
        ));
    }

    let context = CaptureContext::build(&events);

    // Should limit to 5 sample values
    let samples = context.sample_metadata.get("test.field").unwrap();
    assert!(
        samples.len() <= 5,
        "Should limit samples to 5, got {}",
        samples.len()
    );
}

#[test]
fn test_capture_context_samples_first_100_events() {
    // Create 150 events
    let mut events = Vec::new();
    for i in 1..=150 {
        let mut metadata = BTreeMap::new();
        metadata.insert("event.id".to_string(), i.to_string());
        events.push(create_test_event(
            i,
            i * 1_000_000_000,
            TransportKind::Grpc,
            metadata,
        ));
    }

    let context = CaptureContext::build(&events);

    // Should record all 150 events in total_events
    assert_eq!(context.total_events, 150);

    // But should only sample from first 100 for metadata
    // Event 101-150 shouldn't be in samples
    let samples = context.sample_metadata.get("event.id").unwrap();
    assert!(
        !samples.contains(&"150".to_string()),
        "Should not sample beyond first 100 events"
    );
}

#[test]
fn test_capture_context_deduplicates_fields() {
    let mut metadata = BTreeMap::new();
    metadata.insert("grpc.method".to_string(), "/api/Test".to_string());

    // Create multiple events with same field
    let events = vec![
        create_test_event(1, 1_000_000_000, TransportKind::Grpc, metadata.clone()),
        create_test_event(2, 2_000_000_000, TransportKind::Grpc, metadata.clone()),
        create_test_event(3, 3_000_000_000, TransportKind::Grpc, metadata),
    ];

    let context = CaptureContext::build(&events);

    // Should only have one instance of grpc.method
    let field_count = context
        .available_fields
        .iter()
        .filter(|f| *f == "grpc.method")
        .count();
    assert_eq!(field_count, 1, "Fields should be deduplicated");
}

#[test]
fn test_capture_context_sorts_fields() {
    let mut metadata = BTreeMap::new();
    metadata.insert("z.field".to_string(), "value".to_string());
    metadata.insert("a.field".to_string(), "value".to_string());
    metadata.insert("m.field".to_string(), "value".to_string());

    let events = vec![create_test_event(
        1,
        1_000_000_000,
        TransportKind::Grpc,
        metadata,
    )];
    let context = CaptureContext::build(&events);

    // Check that fields are sorted
    let mut prev_field = String::new();
    for field in &context.available_fields {
        assert!(
            field >= &prev_field,
            "Fields should be sorted alphabetically"
        );
        prev_field = field.clone();
    }
}

#[test]
fn test_build_anomaly_summary_includes_total_events() {
    let events = vec![
        create_test_event(1, 1_000_000_000, TransportKind::Grpc, BTreeMap::new()),
        create_test_event(2, 2_000_000_000, TransportKind::Zmq, BTreeMap::new()),
    ];

    let context = CaptureContext::build(&events);
    let summary = build_anomaly_summary(&events, &context);

    assert!(summary.contains("Total events: 2"));
}

#[test]
fn test_build_anomaly_summary_includes_transports() {
    let events = vec![
        create_test_event(1, 1_000_000_000, TransportKind::Grpc, BTreeMap::new()),
        create_test_event(2, 2_000_000_000, TransportKind::Zmq, BTreeMap::new()),
    ];

    let context = CaptureContext::build(&events);
    let summary = build_anomaly_summary(&events, &context);

    assert!(summary.contains("Transports:"));
    assert!(summary.contains("Grpc") || summary.contains("gRPC"));
}

#[test]
fn test_build_anomaly_summary_counts_grpc_errors() {
    let mut metadata_ok = BTreeMap::new();
    metadata_ok.insert("grpc.status".to_string(), "0".to_string());

    let mut metadata_err = BTreeMap::new();
    metadata_err.insert("grpc.status".to_string(), "2".to_string());

    let events = vec![
        create_test_event(1, 1_000_000_000, TransportKind::Grpc, metadata_ok),
        create_test_event(2, 2_000_000_000, TransportKind::Grpc, metadata_err.clone()),
        create_test_event(3, 3_000_000_000, TransportKind::Grpc, metadata_err),
    ];

    let context = CaptureContext::build(&events);
    let summary = build_anomaly_summary(&events, &context);

    assert!(summary.contains("Errors: 2"));
    assert!(summary.contains("66.7%") || summary.contains("66.6%"));
}

#[test]
fn test_build_anomaly_summary_counts_http_errors() {
    let mut metadata_ok = BTreeMap::new();
    metadata_ok.insert("http.status".to_string(), "200".to_string());

    let mut metadata_err = BTreeMap::new();
    metadata_err.insert("http.status".to_string(), "500".to_string());

    let events = vec![
        create_test_event(1, 1_000_000_000, TransportKind::RawTcp, metadata_ok),
        create_test_event(2, 2_000_000_000, TransportKind::RawTcp, metadata_err),
    ];

    let context = CaptureContext::build(&events);
    let summary = build_anomaly_summary(&events, &context);

    assert!(summary.contains("Errors: 1"));
    assert!(summary.contains("50"));
}

#[test]
fn test_build_anomaly_summary_includes_sample_metadata() {
    let mut metadata = BTreeMap::new();
    metadata.insert("grpc.method".to_string(), "/api/Test".to_string());
    metadata.insert("grpc.service".to_string(), "TestService".to_string());

    let events = vec![create_test_event(
        1,
        1_000_000_000,
        TransportKind::Grpc,
        metadata,
    )];

    let context = CaptureContext::build(&events);
    let summary = build_anomaly_summary(&events, &context);

    assert!(summary.contains("Sample metadata:"));
    assert!(summary.contains("grpc.method") || summary.contains("grpc.service"));
}

// ============================================================================
// Priority 2: Response Parsing Tests (~80 lines)
// ============================================================================

#[test]
fn test_parse_anomaly_response_empty_array() {
    let json_response = "[]";
    let events = vec![create_test_event(
        1,
        1_000_000_000,
        TransportKind::Grpc,
        BTreeMap::new(),
    )];

    let result = parse_anomaly_response(json_response, &events);

    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 0);
}

#[test]
fn test_parse_anomaly_response_single_anomaly() {
    let json_response = r#"[
        {
            "title": "High Error Rate",
            "description": "50% of requests failing",
            "severity": "high",
            "filter": "grpc.status != \"0\""
        }
    ]"#;

    let mut metadata_ok = BTreeMap::new();
    metadata_ok.insert("grpc.status".to_string(), "0".to_string());
    let mut metadata_err = BTreeMap::new();
    metadata_err.insert("grpc.status".to_string(), "2".to_string());

    let events = vec![
        create_test_event(1, 1_000_000_000, TransportKind::Grpc, metadata_ok),
        create_test_event(2, 2_000_000_000, TransportKind::Grpc, metadata_err),
    ];

    let result = parse_anomaly_response(json_response, &events);

    assert!(result.is_ok());
    let anomalies = result.unwrap();
    assert_eq!(anomalies.len(), 1);
    assert_eq!(anomalies[0].title, "High Error Rate");
    assert_eq!(anomalies[0].description, "50% of requests failing");
    assert!(matches!(
        anomalies[0].severity,
        prb_tui::ai_smart::AnomalySeverity::High
    ));
}

#[test]
fn test_parse_anomaly_response_extracts_json_from_markdown() {
    let response_with_markdown = r#"Here are the anomalies I found:

```json
[
    {
        "title": "Test Anomaly",
        "description": "Test description",
        "severity": "medium"
    }
]
```

Let me know if you need more details."#;

    let events = vec![create_test_event(
        1,
        1_000_000_000,
        TransportKind::Grpc,
        BTreeMap::new(),
    )];

    let result = parse_anomaly_response(response_with_markdown, &events);

    assert!(result.is_ok());
    let anomalies = result.unwrap();
    assert_eq!(anomalies.len(), 1);
    assert_eq!(anomalies[0].title, "Test Anomaly");
}

#[test]
fn test_parse_anomaly_response_severity_levels() {
    let json_response = r#"[
        {"title": "Low", "description": "low", "severity": "low"},
        {"title": "Medium", "description": "medium", "severity": "medium"},
        {"title": "High", "description": "high", "severity": "high"},
        {"title": "Default", "description": "unknown", "severity": "unknown"}
    ]"#;

    let events = vec![create_test_event(
        1,
        1_000_000_000,
        TransportKind::Grpc,
        BTreeMap::new(),
    )];

    let result = parse_anomaly_response(json_response, &events);

    assert!(result.is_ok());
    let anomalies = result.unwrap();
    assert_eq!(anomalies.len(), 4);

    use prb_tui::ai_smart::AnomalySeverity;
    assert!(matches!(anomalies[0].severity, AnomalySeverity::Low));
    assert!(matches!(anomalies[1].severity, AnomalySeverity::Medium));
    assert!(matches!(anomalies[2].severity, AnomalySeverity::High));
    assert!(matches!(anomalies[3].severity, AnomalySeverity::Medium)); // Unknown defaults to Medium
}

#[test]
fn test_parse_anomaly_response_with_filter_matching() {
    let json_response = r#"[
        {
            "title": "Errors Detected",
            "description": "Multiple errors",
            "severity": "high",
            "filter": "grpc.status != \"0\""
        }
    ]"#;

    let mut metadata1 = BTreeMap::new();
    metadata1.insert("grpc.status".to_string(), "0".to_string());
    let mut metadata2 = BTreeMap::new();
    metadata2.insert("grpc.status".to_string(), "2".to_string());
    let mut metadata3 = BTreeMap::new();
    metadata3.insert("grpc.status".to_string(), "5".to_string());

    let events = vec![
        create_test_event(1, 1_000_000_000, TransportKind::Grpc, metadata1),
        create_test_event(2, 2_000_000_000, TransportKind::Grpc, metadata2),
        create_test_event(3, 3_000_000_000, TransportKind::Grpc, metadata3),
    ];

    let result = parse_anomaly_response(json_response, &events);

    assert!(result.is_ok());
    let anomalies = result.unwrap();
    assert_eq!(anomalies.len(), 1);

    // Should have matched events at indices 1 and 2
    assert_eq!(anomalies[0].event_indices.len(), 2);
    assert!(anomalies[0].event_indices.contains(&1));
    assert!(anomalies[0].event_indices.contains(&2));
}

#[test]
fn test_parse_anomaly_response_invalid_json() {
    let invalid_json = "not json at all";
    let events = vec![create_test_event(
        1,
        1_000_000_000,
        TransportKind::Grpc,
        BTreeMap::new(),
    )];

    let result = parse_anomaly_response(invalid_json, &events);

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("JSON parse error"));
}

#[test]
fn test_parse_anomaly_response_not_array() {
    let json_response = r#"{"error": "not an array"}"#;
    let events = vec![create_test_event(
        1,
        1_000_000_000,
        TransportKind::Grpc,
        BTreeMap::new(),
    )];

    let result = parse_anomaly_response(json_response, &events);

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not an array"));
}

#[test]
fn test_parse_protocol_response_single_protocol() {
    let json_response = r#"[
        {
            "protocol": "HTTP/2",
            "confidence": 0.95,
            "description": "Detected HTTP/2 magic bytes"
        }
    ]"#;

    let result = parse_protocol_response(json_response);

    assert!(result.is_ok());
    let hints = result.unwrap();
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].protocol_name, "HTTP/2");
    assert!((hints[0].confidence - 0.95).abs() < 0.01);
    assert_eq!(hints[0].description, "Detected HTTP/2 magic bytes");
}

#[test]
fn test_parse_protocol_response_multiple_protocols() {
    let json_response = r#"[
        {"protocol": "gRPC", "confidence": 0.8, "description": "gRPC over HTTP/2"},
        {"protocol": "HTTP/2", "confidence": 0.6, "description": "Could be plain HTTP/2"},
        {"protocol": "MQTT", "confidence": 0.2, "description": "Low confidence match"}
    ]"#;

    let result = parse_protocol_response(json_response);

    assert!(result.is_ok());
    let hints = result.unwrap();
    assert_eq!(hints.len(), 3);
    assert_eq!(hints[0].protocol_name, "gRPC");
    assert_eq!(hints[1].protocol_name, "HTTP/2");
    assert_eq!(hints[2].protocol_name, "MQTT");
}

#[test]
fn test_parse_protocol_response_with_markdown() {
    let response = r#"Based on the hex dump analysis:

[
    {"protocol": "TLS", "confidence": 0.9, "description": "TLS handshake detected"}
]

This appears to be encrypted traffic."#;

    let result = parse_protocol_response(response);

    assert!(result.is_ok());
    let hints = result.unwrap();
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].protocol_name, "TLS");
}

#[test]
fn test_parse_protocol_response_defaults_missing_fields() {
    let json_response = r#"[
        {"protocol": "Unknown"}
    ]"#;

    let result = parse_protocol_response(json_response);

    assert!(result.is_ok());
    let hints = result.unwrap();
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].protocol_name, "Unknown");
    assert!((hints[0].confidence - 0.5).abs() < 0.01); // Defaults to 0.5
    assert_eq!(hints[0].description, "No description");
}

// ============================================================================
// Priority 3: Smart Suggestions Tests (~50 lines)
// ============================================================================

#[test]
fn test_ai_feature_error_display() {
    let err = AiFeatureError::ApiRequest("timeout".to_string());
    assert!(err.to_string().contains("API request failed: timeout"));

    let err = AiFeatureError::InvalidResponse("empty".to_string());
    assert!(err.to_string().contains("Invalid response: empty"));

    let err = AiFeatureError::NoEvents;
    assert!(err.to_string().contains("No events to analyze"));
}

#[test]
fn test_detect_anomalies_grpc_status_errors() {
    let mut metadata_ok = BTreeMap::new();
    metadata_ok.insert("grpc.status".to_string(), "0".to_string());

    let mut metadata_err1 = BTreeMap::new();
    metadata_err1.insert("grpc.status".to_string(), "2".to_string());

    let mut metadata_err2 = BTreeMap::new();
    metadata_err2.insert("grpc.status".to_string(), "14".to_string());

    let events = vec![
        create_test_event(1, 1_000_000_000, TransportKind::Grpc, metadata_ok),
        create_test_event(2, 2_000_000_000, TransportKind::Grpc, metadata_err1),
        create_test_event(3, 3_000_000_000, TransportKind::Grpc, metadata_err2),
    ];

    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = prb_ai::AiConfig::default();
    let anomalies = rt
        .block_on(prb_tui::ai_features::detect_anomalies(&events, &config))
        .unwrap();

    assert_eq!(anomalies.len(), 2);
    assert!(anomalies.contains(&1));
    assert!(anomalies.contains(&2));
}

#[test]
fn test_detect_anomalies_no_errors() {
    let mut metadata = BTreeMap::new();
    metadata.insert("grpc.status".to_string(), "0".to_string());

    let events = vec![
        create_test_event(1, 1_000_000_000, TransportKind::Grpc, metadata.clone()),
        create_test_event(2, 2_000_000_000, TransportKind::Grpc, metadata),
    ];

    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = prb_ai::AiConfig::default();
    let anomalies = rt
        .block_on(prb_tui::ai_features::detect_anomalies(&events, &config))
        .unwrap();

    assert_eq!(anomalies.len(), 0);
}

#[test]
fn test_detect_anomalies_empty_events() {
    let events: Vec<DebugEvent> = vec![];

    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = prb_ai::AiConfig::default();
    let anomalies = rt
        .block_on(prb_tui::ai_features::detect_anomalies(&events, &config))
        .unwrap();

    assert_eq!(anomalies.len(), 0);
}

#[test]
fn test_detect_anomalies_non_grpc_protocols() {
    let events = vec![
        create_test_event(1, 1_000_000_000, TransportKind::Zmq, BTreeMap::new()),
        create_test_event(2, 2_000_000_000, TransportKind::DdsRtps, BTreeMap::new()),
        create_test_event(3, 3_000_000_000, TransportKind::RawTcp, BTreeMap::new()),
    ];

    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = prb_ai::AiConfig::default();
    let anomalies = rt
        .block_on(prb_tui::ai_features::detect_anomalies(&events, &config))
        .unwrap();

    // Currently only gRPC errors are detected
    assert_eq!(anomalies.len(), 0);
}
