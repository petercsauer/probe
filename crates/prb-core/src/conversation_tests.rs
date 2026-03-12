//! Tests for conversation types.

use super::*;

#[test]
fn test_conversation_id_new_and_display() {
    let id = ConversationId::new("grpc:192.168.1.1:8080->192.168.1.2:9090/stream123");
    assert_eq!(id.as_str(), "grpc:192.168.1.1:8080->192.168.1.2:9090/stream123");
    assert_eq!(id.to_string(), "grpc:192.168.1.1:8080->192.168.1.2:9090/stream123");
}

#[test]
fn test_conversation_id_serde_roundtrip() {
    let id = ConversationId::new("test-id-123");
    let json = serde_json::to_string(&id).expect("failed to serialize");
    let deserialized: ConversationId = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(id, deserialized);
}

#[test]
fn test_conversation_kind_display() {
    assert_eq!(ConversationKind::UnaryRpc.to_string(), "unary-rpc");
    assert_eq!(ConversationKind::ServerStreaming.to_string(), "server-streaming");
    assert_eq!(ConversationKind::ClientStreaming.to_string(), "client-streaming");
    assert_eq!(ConversationKind::BidirectionalStreaming.to_string(), "bidirectional-streaming");
    assert_eq!(ConversationKind::RequestReply.to_string(), "request-reply");
    assert_eq!(ConversationKind::PubSubChannel.to_string(), "pub-sub");
    assert_eq!(ConversationKind::Pipeline.to_string(), "pipeline");
    assert_eq!(ConversationKind::TopicExchange.to_string(), "topic-exchange");
    assert_eq!(ConversationKind::TcpStream.to_string(), "tcp-stream");
    assert_eq!(ConversationKind::Unknown.to_string(), "unknown");
}

#[test]
fn test_conversation_kind_serde_roundtrip() {
    let kinds = vec![
        ConversationKind::UnaryRpc,
        ConversationKind::ServerStreaming,
        ConversationKind::ClientStreaming,
        ConversationKind::BidirectionalStreaming,
        ConversationKind::RequestReply,
        ConversationKind::PubSubChannel,
        ConversationKind::Pipeline,
        ConversationKind::TopicExchange,
        ConversationKind::TcpStream,
        ConversationKind::Unknown,
    ];

    for kind in kinds {
        let json = serde_json::to_string(&kind).expect("failed to serialize");
        let deserialized: ConversationKind = serde_json::from_str(&json).expect("failed to deserialize");
        assert_eq!(kind, deserialized);
    }
}

#[test]
fn test_conversation_state_display() {
    assert_eq!(ConversationState::Active.to_string(), "active");
    assert_eq!(ConversationState::Complete.to_string(), "complete");
    assert_eq!(ConversationState::Error.to_string(), "error");
    assert_eq!(ConversationState::Timeout.to_string(), "timeout");
    assert_eq!(ConversationState::Incomplete.to_string(), "incomplete");
}

#[test]
fn test_conversation_state_serde_roundtrip() {
    let states = vec![
        ConversationState::Active,
        ConversationState::Complete,
        ConversationState::Error,
        ConversationState::Timeout,
        ConversationState::Incomplete,
    ];

    for state in states {
        let json = serde_json::to_string(&state).expect("failed to serialize");
        let deserialized: ConversationState = serde_json::from_str(&json).expect("failed to deserialize");
        assert_eq!(state, deserialized);
    }
}

#[test]
fn test_conversation_error_new() {
    let error = ConversationError::new("grpc-status", "Internal server error");
    assert_eq!(error.kind, "grpc-status");
    assert_eq!(error.message, "Internal server error");
    assert_eq!(error.code, None);
}

#[test]
fn test_conversation_error_with_code() {
    let error = ConversationError::new("grpc-status", "Not found")
        .with_code("5");
    assert_eq!(error.kind, "grpc-status");
    assert_eq!(error.message, "Not found");
    assert_eq!(error.code, Some("5".to_string()));
}

#[test]
fn test_conversation_error_serde_roundtrip() {
    let error = ConversationError::new("timeout", "Connection timeout")
        .with_code("408");
    let json = serde_json::to_string(&error).expect("failed to serialize");
    let deserialized: ConversationError = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(error.kind, deserialized.kind);
    assert_eq!(error.message, deserialized.message);
    assert_eq!(error.code, deserialized.code);
}

#[test]
fn test_conversation_metrics_default() {
    let metrics = ConversationMetrics::default();
    assert_eq!(metrics.start_time, None);
    assert_eq!(metrics.end_time, None);
    assert_eq!(metrics.duration_ns, 0);
    assert_eq!(metrics.time_to_first_response_ns, None);
    assert_eq!(metrics.request_count, 0);
    assert_eq!(metrics.response_count, 0);
    assert_eq!(metrics.total_bytes, 0);
    assert!(metrics.error.is_none());
}

#[test]
fn test_conversation_metrics_serde_roundtrip() {
    let metrics = ConversationMetrics {
        start_time: Some(Timestamp::from_nanos(1000000000)),
        end_time: Some(Timestamp::from_nanos(2000000000)),
        duration_ns: 1000000000,
        time_to_first_response_ns: Some(500000000),
        request_count: 5,
        response_count: 3,
        total_bytes: 4096,
        error: Some(ConversationError::new("timeout", "No response")),
    };

    let json = serde_json::to_string(&metrics).expect("failed to serialize");
    let deserialized: ConversationMetrics = serde_json::from_str(&json).expect("failed to deserialize");

    assert_eq!(metrics.start_time, deserialized.start_time);
    assert_eq!(metrics.end_time, deserialized.end_time);
    assert_eq!(metrics.duration_ns, deserialized.duration_ns);
    assert_eq!(metrics.time_to_first_response_ns, deserialized.time_to_first_response_ns);
    assert_eq!(metrics.request_count, deserialized.request_count);
    assert_eq!(metrics.response_count, deserialized.response_count);
    assert_eq!(metrics.total_bytes, deserialized.total_bytes);
}

#[test]
fn test_conversation_new() {
    let id = ConversationId::new("test-conv-1");
    let conv = Conversation::new(
        id.clone(),
        ConversationKind::UnaryRpc,
        TransportKind::Grpc,
        ConversationState::Complete,
    );

    assert_eq!(conv.id, id);
    assert_eq!(conv.kind, ConversationKind::UnaryRpc);
    assert_eq!(conv.protocol, TransportKind::Grpc);
    assert_eq!(conv.state, ConversationState::Complete);
    assert!(conv.event_ids.is_empty());
    assert_eq!(conv.metrics.duration_ns, 0);
    assert!(conv.metadata.is_empty());
    assert_eq!(conv.summary, "");
}

#[test]
fn test_conversation_add_event() {
    let mut conv = Conversation::new(
        ConversationId::new("test"),
        ConversationKind::UnaryRpc,
        TransportKind::Grpc,
        ConversationState::Active,
    );

    let event_id = EventId::next();
    conv.add_event(event_id);

    assert_eq!(conv.event_ids.len(), 1);
    assert_eq!(conv.event_ids[0], event_id);
}

#[test]
fn test_conversation_add_multiple_events() {
    let mut conv = Conversation::new(
        ConversationId::new("test"),
        ConversationKind::ServerStreaming,
        TransportKind::Grpc,
        ConversationState::Active,
    );

    let ids: Vec<EventId> = (0..5).map(|_| EventId::next()).collect();
    for id in &ids {
        conv.add_event(*id);
    }

    assert_eq!(conv.event_ids.len(), 5);
    assert_eq!(conv.event_ids, ids);
}

#[test]
fn test_conversation_add_metadata() {
    let mut conv = Conversation::new(
        ConversationId::new("test"),
        ConversationKind::UnaryRpc,
        TransportKind::Grpc,
        ConversationState::Complete,
    );

    conv.add_metadata("grpc.method", "/api.v1.Users/GetUser");
    conv.add_metadata("grpc.status", "0");

    assert_eq!(conv.metadata.len(), 2);
    assert_eq!(conv.metadata.get("grpc.method"), Some(&"/api.v1.Users/GetUser".to_string()));
    assert_eq!(conv.metadata.get("grpc.status"), Some(&"0".to_string()));
}

#[test]
fn test_conversation_set_summary() {
    let mut conv = Conversation::new(
        ConversationId::new("test"),
        ConversationKind::UnaryRpc,
        TransportKind::Grpc,
        ConversationState::Complete,
    );

    conv.set_summary("GET /users/123 → OK (45ms)");
    assert_eq!(conv.summary, "GET /users/123 → OK (45ms)");
}

#[test]
fn test_conversation_set_metrics() {
    let mut conv = Conversation::new(
        ConversationId::new("test"),
        ConversationKind::UnaryRpc,
        TransportKind::Grpc,
        ConversationState::Complete,
    );

    let metrics = ConversationMetrics {
        duration_ns: 45000000,
        request_count: 1,
        response_count: 1,
        ..Default::default()
    };

    conv.set_metrics(metrics.clone());
    assert_eq!(conv.metrics.duration_ns, 45000000);
    assert_eq!(conv.metrics.request_count, 1);
    assert_eq!(conv.metrics.response_count, 1);
}

#[test]
fn test_conversation_serde_roundtrip() {
    let mut conv = Conversation::new(
        ConversationId::new("test-conv"),
        ConversationKind::BidirectionalStreaming,
        TransportKind::Grpc,
        ConversationState::Complete,
    );

    conv.add_event(EventId::next());
    conv.add_event(EventId::next());
    conv.add_metadata("grpc.method", "/test.Service/Method");
    conv.set_summary("Test conversation");

    let json = serde_json::to_string(&conv).expect("failed to serialize");
    let deserialized: Conversation = serde_json::from_str(&json).expect("failed to deserialize");

    assert_eq!(conv.id, deserialized.id);
    assert_eq!(conv.kind, deserialized.kind);
    assert_eq!(conv.protocol, deserialized.protocol);
    assert_eq!(conv.state, deserialized.state);
    assert_eq!(conv.event_ids, deserialized.event_ids);
    assert_eq!(conv.metadata, deserialized.metadata);
    assert_eq!(conv.summary, deserialized.summary);
}
