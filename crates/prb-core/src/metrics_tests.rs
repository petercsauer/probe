//! Tests for metrics computation.

use super::*;
use bytes::Bytes;

/// Helper to create a test `DebugEvent`.
fn create_test_event(timestamp_ns: u64, direction: Direction, payload_size: usize) -> DebugEvent {
    DebugEvent::builder()
        .id(EventId::next())
        .timestamp(Timestamp::from_nanos(timestamp_ns))
        .source(EventSource {
            adapter: "test".to_string(),
            origin: "test-origin".to_string(),
            network: Some(NetworkAddr {
                src: "192.168.1.1:8080".to_string(),
                dst: "192.168.1.2:9090".to_string(),
            }),
        })
        .transport(TransportKind::Grpc)
        .direction(direction)
        .payload(Payload::Raw {
            raw: Bytes::from(vec![0u8; payload_size]),
        })
        .build()
}

#[test]
fn test_compute_metrics_empty_events() {
    let events: Vec<&DebugEvent> = vec![];
    let result = compute_metrics(&events).expect("should succeed");

    assert_eq!(result.start_time, None);
    assert_eq!(result.end_time, None);
    assert_eq!(result.duration_ns, 0);
    assert_eq!(result.time_to_first_response_ns, None);
    assert_eq!(result.request_count, 0);
    assert_eq!(result.response_count, 0);
    assert_eq!(result.total_bytes, 0);
    assert!(result.error.is_none());
}

#[test]
fn test_compute_metrics_single_event() {
    let event = create_test_event(1000000000, Direction::Outbound, 100);
    let events = vec![&event];

    let result = compute_metrics(&events).expect("should succeed");

    assert_eq!(result.start_time, Some(Timestamp::from_nanos(1000000000)));
    assert_eq!(result.end_time, Some(Timestamp::from_nanos(1000000000)));
    assert_eq!(result.duration_ns, 0); // Same timestamp
    assert_eq!(result.time_to_first_response_ns, None); // No inbound
    assert_eq!(result.request_count, 1);
    assert_eq!(result.response_count, 0);
    assert_eq!(result.total_bytes, 100);
}

#[test]
fn test_compute_metrics_request_response() {
    let req = create_test_event(1000000000, Direction::Outbound, 200);
    let resp = create_test_event(1050000000, Direction::Inbound, 500);
    let events = vec![&req, &resp];

    let result = compute_metrics(&events).expect("should succeed");

    assert_eq!(result.start_time, Some(Timestamp::from_nanos(1000000000)));
    assert_eq!(result.end_time, Some(Timestamp::from_nanos(1050000000)));
    assert_eq!(result.duration_ns, 50000000); // 50ms
    assert_eq!(result.time_to_first_response_ns, Some(50000000)); // 50ms
    assert_eq!(result.request_count, 1);
    assert_eq!(result.response_count, 1);
    assert_eq!(result.total_bytes, 700);
}

#[test]
fn test_compute_metrics_multiple_requests_responses() {
    let events_data = [
        (1000000000, Direction::Outbound, 100),
        (1010000000, Direction::Outbound, 150),
        (1020000000, Direction::Inbound, 200),
        (1030000000, Direction::Inbound, 250),
        (1040000000, Direction::Outbound, 300),
    ];

    let events: Vec<DebugEvent> = events_data
        .iter()
        .map(|(ts, dir, size)| create_test_event(*ts, *dir, *size))
        .collect();
    let event_refs: Vec<&DebugEvent> = events.iter().collect();

    let result = compute_metrics(&event_refs).expect("should succeed");

    assert_eq!(result.duration_ns, 40000000); // 1040 - 1000
    assert_eq!(result.time_to_first_response_ns, Some(20000000)); // First inbound - first outbound
    assert_eq!(result.request_count, 3);
    assert_eq!(result.response_count, 2);
    assert_eq!(result.total_bytes, 1000);
}

#[test]
fn test_extract_error_grpc_status_ok() {
    let mut event = create_test_event(1000000000, Direction::Inbound, 100);
    event
        .metadata
        .insert("grpc.status".to_string(), "0".to_string());
    let events = vec![&event];

    let result = compute_metrics(&events).expect("should succeed");
    assert!(result.error.is_none());
}

#[test]
fn test_extract_error_grpc_status_error() {
    let mut event = create_test_event(1000000000, Direction::Inbound, 100);
    event
        .metadata
        .insert("grpc.status".to_string(), "14".to_string());
    event.metadata.insert(
        "grpc.message".to_string(),
        "Service unavailable".to_string(),
    );
    let events = vec![&event];

    let result = compute_metrics(&events).expect("should succeed");
    assert!(result.error.is_some());

    let error = result.error.unwrap();
    assert_eq!(error.kind, "grpc-status");
    assert_eq!(error.code, Some("14".to_string()));
    assert_eq!(error.message, "Service unavailable");
}

#[test]
fn test_extract_error_grpc_status_without_message() {
    let mut event = create_test_event(1000000000, Direction::Inbound, 100);
    event
        .metadata
        .insert("grpc.status".to_string(), "5".to_string());
    let events = vec![&event];

    let result = compute_metrics(&events).expect("should succeed");
    assert!(result.error.is_some());

    let error = result.error.unwrap();
    assert_eq!(error.kind, "grpc-status");
    assert_eq!(error.code, Some("5".to_string()));
    assert!(error.message.contains("gRPC error status 5"));
}

#[test]
fn test_extract_error_rst_stream() {
    let mut event = create_test_event(1000000000, Direction::Inbound, 0);
    event
        .metadata
        .insert("h2.frame_type".to_string(), "RST_STREAM".to_string());
    let events = vec![&event];

    let result = compute_metrics(&events).expect("should succeed");
    assert!(result.error.is_some());

    let error = result.error.unwrap();
    assert_eq!(error.kind, "rst-stream");
    assert_eq!(error.message, "HTTP/2 stream reset");
}

#[test]
fn test_extract_error_timeout_no_response() {
    let req = create_test_event(1000000000, Direction::Outbound, 100);
    let events = vec![&req];

    let result = compute_metrics(&events).expect("should succeed");
    assert!(result.error.is_some());

    let error = result.error.unwrap();
    assert_eq!(error.kind, "timeout");
    assert_eq!(error.message, "No response received");
}

#[test]
fn test_extract_error_dds_sequence_gaps() {
    let mut events_data = [
        create_test_event(1000000000, Direction::Inbound, 100),
        create_test_event(1010000000, Direction::Inbound, 100),
        create_test_event(1020000000, Direction::Inbound, 100),
    ];

    // Set sequence numbers with gaps: 1, 2, 5 (missing 3, 4)
    events_data[0].sequence = Some(1);
    events_data[1].sequence = Some(2);
    events_data[2].sequence = Some(5);

    let event_refs: Vec<&DebugEvent> = events_data.iter().collect();
    let result = compute_metrics(&event_refs).expect("should succeed");

    assert!(result.error.is_some());
    let error = result.error.unwrap();
    assert_eq!(error.kind, "sequence-gap");
    assert!(error.message.contains("2 missing sequence numbers"));
}

#[test]
fn test_check_dds_sequence_gaps_none() {
    let mut events_data = [
        create_test_event(1000000000, Direction::Inbound, 100),
        create_test_event(1010000000, Direction::Inbound, 100),
        create_test_event(1020000000, Direction::Inbound, 100),
    ];

    // Sequential: 1, 2, 3
    events_data[0].sequence = Some(1);
    events_data[1].sequence = Some(2);
    events_data[2].sequence = Some(3);

    let event_refs: Vec<&DebugEvent> = events_data.iter().collect();
    let result = compute_metrics(&event_refs).expect("should succeed");

    assert!(result.error.is_none());
}

#[test]
fn test_check_dds_sequence_gaps_empty() {
    let events: Vec<&DebugEvent> = vec![];
    let result = compute_metrics(&events).expect("should succeed");
    assert!(result.error.is_none());
}

#[test]
fn test_percentile_empty() {
    let values: Vec<u64> = vec![];
    let result = metrics::percentile(&values, 0.50);
    assert_eq!(result, 0);
}

#[test]
fn test_percentile_single_value() {
    let values = vec![42];
    assert_eq!(metrics::percentile(&values, 0.50), 42);
    assert_eq!(metrics::percentile(&values, 0.95), 42);
    assert_eq!(metrics::percentile(&values, 0.99), 42);
}

#[test]
fn test_percentile_multiple_values() {
    let values = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];

    assert_eq!(metrics::percentile(&values, 0.0), 10);
    // With percentile rounding: p50 at index (0.5 * 9).round() = 5, which is 60
    assert_eq!(metrics::percentile(&values, 0.50), 60); // Median
    assert_eq!(metrics::percentile(&values, 0.95), 100);
    assert_eq!(metrics::percentile(&values, 1.0), 100);
}

#[test]
fn test_compute_aggregate_metrics_empty() {
    let conversations: Vec<&Conversation> = vec![];
    let result = compute_aggregate_metrics(&conversations);

    assert_eq!(result.total_conversations, 0);
    assert_eq!(result.error_rate, 0.0);
    assert_eq!(result.latency_p50_ns, 0);
    assert_eq!(result.latency_p95_ns, 0);
    assert_eq!(result.latency_p99_ns, 0);
    assert_eq!(result.total_bytes, 0);
    assert_eq!(result.conversations_per_second, 0.0);
}

#[test]
fn test_compute_aggregate_metrics_single_conversation() {
    let mut conv = Conversation::new(
        ConversationId::new("test"),
        ConversationKind::UnaryRpc,
        TransportKind::Grpc,
        ConversationState::Complete,
    );

    let metrics = ConversationMetrics {
        start_time: Some(Timestamp::from_nanos(1000000000)),
        end_time: Some(Timestamp::from_nanos(1050000000)),
        duration_ns: 50000000,
        total_bytes: 1024,
        ..Default::default()
    };
    conv.set_metrics(metrics);

    let conversations = vec![&conv];
    let result = compute_aggregate_metrics(&conversations);

    assert_eq!(result.total_conversations, 1);
    assert_eq!(result.error_rate, 0.0);
    assert_eq!(result.latency_p50_ns, 50000000);
    assert_eq!(result.total_bytes, 1024);
}

#[test]
fn test_compute_aggregate_metrics_with_errors() {
    let mut conv1 = Conversation::new(
        ConversationId::new("test1"),
        ConversationKind::UnaryRpc,
        TransportKind::Grpc,
        ConversationState::Complete,
    );
    conv1.metrics.duration_ns = 100000000;

    let mut conv2 = Conversation::new(
        ConversationId::new("test2"),
        ConversationKind::UnaryRpc,
        TransportKind::Grpc,
        ConversationState::Error,
    );
    conv2.metrics.duration_ns = 200000000;

    let mut conv3 = Conversation::new(
        ConversationId::new("test3"),
        ConversationKind::UnaryRpc,
        TransportKind::Grpc,
        ConversationState::Complete,
    );
    conv3.metrics.duration_ns = 150000000;

    let conversations = vec![&conv1, &conv2, &conv3];
    let result = compute_aggregate_metrics(&conversations);

    assert_eq!(result.total_conversations, 3);
    assert!((result.error_rate - 0.3333).abs() < 0.001); // 1/3 ≈ 0.333
}

#[test]
fn test_compute_aggregate_metrics_percentiles() {
    let mut conversations: Vec<Conversation> = Vec::new();

    // Create 10 conversations with durations from 10ms to 100ms
    for i in 1..=10 {
        let mut conv = Conversation::new(
            ConversationId::new(format!("test{i}")),
            ConversationKind::UnaryRpc,
            TransportKind::Grpc,
            ConversationState::Complete,
        );
        conv.metrics.duration_ns = (i * 10_000_000) as u64; // i * 10ms
        conversations.push(conv);
    }

    let conv_refs: Vec<&Conversation> = conversations.iter().collect();
    let result = compute_aggregate_metrics(&conv_refs);

    assert_eq!(result.total_conversations, 10);
    // With percentile rounding: p50 at index (0.5 * 9).round() = 5, which is 60ms
    assert_eq!(result.latency_p50_ns, 60_000_000); // 60ms
    assert_eq!(result.latency_p95_ns, 100_000_000); // 100ms
}

#[test]
fn test_compute_aggregate_metrics_conversations_per_second() {
    let mut conversations: Vec<Conversation> = Vec::new();

    // 10 conversations over 5 seconds (1B ns to 6B ns)
    for i in 0..10 {
        let mut conv = Conversation::new(
            ConversationId::new(format!("test{i}")),
            ConversationKind::UnaryRpc,
            TransportKind::Grpc,
            ConversationState::Complete,
        );
        conv.metrics.start_time = Some(Timestamp::from_nanos(1_000_000_000 + (i * 500_000_000)));
        conv.metrics.end_time = Some(Timestamp::from_nanos(
            1_000_000_000 + (i * 500_000_000) + 100_000_000,
        ));
        conv.metrics.duration_ns = 100_000_000;
        conversations.push(conv);
    }

    let conv_refs: Vec<&Conversation> = conversations.iter().collect();
    let result = compute_aggregate_metrics(&conv_refs);

    // Time span is from 1s to ~5.6s = 4.6s
    // 10 conversations / 4.6s ≈ 2.17 conv/s
    assert!(result.conversations_per_second > 2.0 && result.conversations_per_second < 3.0);
}

#[test]
fn test_compute_aggregate_metrics_total_bytes() {
    let mut conversations: Vec<Conversation> = Vec::new();

    for i in 1..=5 {
        let mut conv = Conversation::new(
            ConversationId::new(format!("test{i}")),
            ConversationKind::UnaryRpc,
            TransportKind::Grpc,
            ConversationState::Complete,
        );
        conv.metrics.total_bytes = (i * 1024) as u64;
        conversations.push(conv);
    }

    let conv_refs: Vec<&Conversation> = conversations.iter().collect();
    let result = compute_aggregate_metrics(&conv_refs);

    // Sum: 1024 + 2048 + 3072 + 4096 + 5120 = 15360
    assert_eq!(result.total_bytes, 15360);
}
