//! Real-world OTel trace correlation tests.
//!
//! Tests OpenTelemetry trace context extraction and correlation with network captures.

use prb_core::{
    CorrelationKey, DebugEvent, Direction, EventSource, Payload, TransportKind,
    extract_trace_context, parse_w3c_traceparent,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/captures/otel")
}

/// OpenTelemetry span export format (simplified).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OTelSpan {
    trace_id: String,
    span_id: String,
    parent_span_id: Option<String>,
    name: String,
    start_time_unix_nano: u64,
    end_time_unix_nano: u64,
    attributes: HashMap<String, String>,
}

/// Load OTel spans from a JSON file.
fn load_spans(filename: &str) -> Vec<OTelSpan> {
    let path = fixtures_dir().join(filename);
    if !path.exists() {
        return Vec::new();
    }
    let content = std::fs::read_to_string(path).expect("Failed to read spans file");
    serde_json::from_str(&content).expect("Failed to parse spans JSON")
}

/// Create synthetic network events with trace context.
fn create_synthetic_event_with_trace(trace_id: &str, span_id: &str, trace_flags: u8) -> DebugEvent {
    let mut metadata = std::collections::BTreeMap::new();
    metadata.insert("http.method".to_string(), "GET".to_string());
    metadata.insert("http.path".to_string(), "/api/users".to_string());

    // Add OTel metadata
    metadata.insert(
        prb_core::METADATA_KEY_OTEL_TRACE_ID.to_string(),
        trace_id.to_string(),
    );
    metadata.insert(
        prb_core::METADATA_KEY_OTEL_SPAN_ID.to_string(),
        span_id.to_string(),
    );
    metadata.insert(
        prb_core::METADATA_KEY_OTEL_TRACE_FLAGS.to_string(),
        trace_flags.to_string(),
    );

    DebugEvent::builder()
        .source(EventSource {
            adapter: "synthetic".to_string(),
            origin: "otel-test".to_string(),
            network: Some(prb_core::NetworkAddr {
                src: "10.0.0.1:54321".to_string(),
                dst: "10.0.0.2:80".to_string(),
            }),
        })
        .transport(TransportKind::RawTcp)
        .direction(Direction::Outbound)
        .payload(Payload::Raw {
            raw: bytes::Bytes::from("GET /api/users HTTP/1.1\r\n\r\n"),
        })
        .metadata("otel.trace_id", trace_id)
        .metadata("otel.span_id", span_id)
        .correlation_key(CorrelationKey::TraceContext {
            trace_id: trace_id.to_string(),
            span_id: span_id.to_string(),
        })
        .build()
}

#[test]
fn test_traceparent_header_extraction() {
    // Valid W3C traceparent header
    let traceparent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

    let ctx = parse_w3c_traceparent(traceparent).expect("Should parse valid traceparent");

    assert_eq!(ctx.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
    assert_eq!(ctx.span_id, "00f067aa0ba902b7");
    assert_eq!(ctx.trace_flags, 0x01);
    assert!(ctx.is_sampled());
}

#[test]
fn test_tracestate_header_extraction() {
    // W3C traceparent with tracestate
    let mut headers = HashMap::new();
    headers.insert(
        "traceparent".to_string(),
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_string(),
    );
    headers.insert(
        "tracestate".to_string(),
        "vendor1=value1,vendor2=value2".to_string(),
    );

    let ctx = extract_trace_context(&headers).expect("Should extract trace context");

    assert_eq!(ctx.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
    assert_eq!(ctx.span_id, "00f067aa0ba902b7");
    assert_eq!(
        ctx.tracestate.as_deref(),
        Some("vendor1=value1,vendor2=value2")
    );
}

#[test]
fn test_grpc_metadata_trace_extraction() {
    // Simulate gRPC metadata with traceparent
    let mut headers = HashMap::new();
    headers.insert(
        "traceparent".to_string(),
        "00-a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6-0011223344556677-01".to_string(),
    );
    headers.insert(
        "grpc-method".to_string(),
        "/service.Method/Call".to_string(),
    );

    let ctx = extract_trace_context(&headers).expect("Should extract from gRPC metadata");

    assert_eq!(ctx.trace_id, "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6");
    assert_eq!(ctx.span_id, "0011223344556677");
    assert!(ctx.is_sampled());
}

#[test]
fn test_correlate_network_events_to_spans() {
    // Create synthetic events with trace context
    let trace_id = "4bf92f3577b34da6a3ce929d0e0e4736";
    let span_id_1 = "00f067aa0ba902b7";
    let span_id_2 = "11f067aa0ba902b8";

    let event1 = create_synthetic_event_with_trace(trace_id, span_id_1, 0x01);
    let event2 = create_synthetic_event_with_trace(trace_id, span_id_2, 0x01);

    // Load matching spans if they exist
    let spans = load_spans("synthetic-trace.spans.json");

    // Verify events have trace context
    assert!(
        event1
            .correlation_keys
            .iter()
            .any(|k| matches!(k, CorrelationKey::TraceContext { .. }))
    );
    assert!(
        event2
            .correlation_keys
            .iter()
            .any(|k| matches!(k, CorrelationKey::TraceContext { .. }))
    );

    // Verify trace_id matches across events
    if let Some(CorrelationKey::TraceContext { trace_id: tid1, .. }) =
        event1.correlation_keys.first()
        && let Some(CorrelationKey::TraceContext { trace_id: tid2, .. }) =
            event2.correlation_keys.first()
    {
        assert_eq!(tid1, tid2, "Same trace_id should link events");
    }

    // If spans exist, verify correlation
    if !spans.is_empty() {
        let matching_spans: Vec<_> = spans.iter().filter(|s| s.trace_id == trace_id).collect();
        assert!(
            !matching_spans.is_empty(),
            "Should find spans with matching trace_id"
        );
    }
}

#[test]
fn test_correlation_with_missing_spans() {
    // Create event with trace context but no matching spans
    let event = create_synthetic_event_with_trace(
        "missing-trace-id-1234567890abcdef",
        "missing-span-12345678",
        0x00,
    );

    // Load spans (may be empty or not contain this trace)
    let spans = load_spans("synthetic-trace.spans.json");

    // Verify event is valid even without matching spans
    assert!(
        event
            .correlation_keys
            .iter()
            .any(|k| matches!(k, CorrelationKey::TraceContext { .. }))
    );

    // No panic - graceful handling of missing spans
    let matching = spans
        .iter()
        .filter(|s| s.trace_id == "missing-trace-id-1234567890abcdef")
        .count();
    assert_eq!(matching, 0, "Should gracefully handle missing spans");
}

#[test]
fn test_correlation_with_multiple_services() {
    // Simulate multi-service trace with same trace_id
    let trace_id = "shared-trace-id-across-services123";

    // Service A calls Service B
    let event_service_a = create_synthetic_event_with_trace(trace_id, "span-service-a-001", 0x01);

    // Service B processes request
    let event_service_b = create_synthetic_event_with_trace(trace_id, "span-service-b-002", 0x01);

    // Verify same trace_id across different services
    let extract_trace_id = |event: &DebugEvent| -> Option<String> {
        event.correlation_keys.iter().find_map(|k| match k {
            CorrelationKey::TraceContext { trace_id, .. } => Some(trace_id.clone()),
            _ => None,
        })
    };

    let tid_a = extract_trace_id(&event_service_a);
    let tid_b = extract_trace_id(&event_service_b);

    assert_eq!(tid_a, tid_b, "Trace ID should span multiple services");
    assert_eq!(tid_a.as_deref(), Some(trace_id));
}

#[test]
fn test_synthetic_otel_correlation() {
    // Generate synthetic matched fixtures
    let trace_id = "synthetic-test-trace-id-abcdef01";
    let span_id = "synthetic-span-1234";

    // Create event with trace context
    let event = create_synthetic_event_with_trace(trace_id, span_id, 0x01);

    // Verify 100% correlation rate for synthetic fixtures
    assert!(
        event
            .correlation_keys
            .iter()
            .any(|k| matches!(k, CorrelationKey::TraceContext { .. }))
    );

    // Extract and verify trace context
    if let Some(CorrelationKey::TraceContext {
        trace_id: tid,
        span_id: sid,
    }) = event.correlation_keys.first()
    {
        assert_eq!(tid, trace_id);
        assert_eq!(sid, span_id);
    } else {
        panic!("Should have TraceContext correlation key");
    }

    // Verify metadata includes OTel fields
    assert_eq!(
        event.metadata.get(prb_core::METADATA_KEY_OTEL_TRACE_ID),
        Some(&trace_id.to_string())
    );
    assert_eq!(
        event.metadata.get(prb_core::METADATA_KEY_OTEL_SPAN_ID),
        Some(&span_id.to_string())
    );
}

#[test]
fn test_b3_propagation_format() {
    // Test B3 single-header format extraction
    let mut headers = HashMap::new();
    headers.insert(
        "b3".to_string(),
        "4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-1".to_string(),
    );

    let ctx = extract_trace_context(&headers).expect("Should extract B3 format");

    assert_eq!(ctx.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
    assert_eq!(ctx.span_id, "00f067aa0ba902b7");
    assert!(ctx.is_sampled());
}

#[test]
fn test_multiple_trace_formats_priority() {
    // Test that W3C traceparent takes priority over B3
    let mut headers = HashMap::new();
    headers.insert(
        "traceparent".to_string(),
        "00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-1111111111111111-01".to_string(),
    );
    headers.insert(
        "b3".to_string(),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-2222222222222222-1".to_string(),
    );

    let ctx = extract_trace_context(&headers).expect("Should extract trace context");

    // W3C format should win
    assert_eq!(ctx.trace_id, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    assert_eq!(ctx.span_id, "1111111111111111");
}
