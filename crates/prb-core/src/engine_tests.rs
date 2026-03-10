//! Tests for conversation reconstruction engine.

use super::*;
use bytes::Bytes;
use std::collections::BTreeMap;

/// Mock correlation strategy for testing.
struct MockGrpcStrategy;

impl CorrelationStrategy for MockGrpcStrategy {
    fn transport(&self) -> TransportKind {
        TransportKind::Grpc
    }

    fn correlate<'a>(&self, events: &'a [DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError> {
        // Group by stream ID from metadata
        let mut flows_map: BTreeMap<String, Vec<&'a DebugEvent>> = BTreeMap::new();

        for event in events {
            if let Some(stream_id) = event.metadata.get("h2.stream_id") {
                flows_map
                    .entry(stream_id.clone())
                    .or_default()
                    .push(event);
            }
        }

        let mut flows = Vec::new();
        for (stream_id, events) in flows_map {
            let mut flow = Flow::new(format!("grpc:stream:{}", stream_id));
            for event in &events {
                flow = flow.add_event(event);
            }
            // Copy grpc metadata from events to flow
            for event in &events {
                if let Some(method) = event.metadata.get("grpc.method") {
                    flow = flow.add_metadata("grpc.method", method.clone());
                }
                if let Some(status) = event.metadata.get("grpc.status") {
                    flow = flow.add_metadata("grpc.status", status.clone());
                }
            }
            flows.push(flow);
        }

        Ok(flows)
    }
}

/// Mock ZMQ strategy for testing.
struct MockZmqStrategy;

impl CorrelationStrategy for MockZmqStrategy {
    fn transport(&self) -> TransportKind {
        TransportKind::Zmq
    }

    fn correlate<'a>(&self, events: &'a [DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError> {
        // Group by topic
        let mut flows_map: BTreeMap<String, Vec<&'a DebugEvent>> = BTreeMap::new();

        for event in events {
            if let Some(topic) = event.metadata.get("zmq.topic") {
                flows_map
                    .entry(topic.clone())
                    .or_default()
                    .push(event);
            }
        }

        let mut flows = Vec::new();
        for (topic, events) in flows_map {
            let mut flow = Flow::new(format!("zmq:topic:{}", topic));
            for event in &events {
                flow = flow.add_event(event);
            }
            flow = flow.add_metadata("zmq.topic", topic);
            flows.push(flow);
        }

        Ok(flows)
    }
}

/// Helper to create a test DebugEvent.
fn create_test_event(
    transport: TransportKind,
    direction: Direction,
    timestamp_ns: u64,
) -> DebugEvent {
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
        .transport(transport)
        .direction(direction)
        .payload(Payload::Raw {
            raw: Bytes::from_static(b"test payload"),
        })
        .build()
}

#[test]
fn test_conversation_engine_new() {
    let engine = ConversationEngine::new();
    assert_eq!(engine.strategies.len(), 0);
}

#[test]
fn test_conversation_engine_register() {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));
    assert_eq!(engine.strategies.len(), 1);

    engine.register(Box::new(MockZmqStrategy));
    assert_eq!(engine.strategies.len(), 2);
}

#[test]
fn test_conversation_engine_build_conversations_empty() {
    let engine = ConversationEngine::new();
    let events: Vec<DebugEvent> = vec![];

    let result = engine.build_conversations(&events).expect("should succeed");
    assert_eq!(result.conversations.len(), 0);
}

#[test]
fn test_conversation_engine_build_conversations_single_protocol() {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));

    let mut event1 = create_test_event(TransportKind::Grpc, Direction::Outbound, 1000000000);
    event1.metadata.insert("h2.stream_id".to_string(), "1".to_string());
    event1.metadata.insert("grpc.method".to_string(), "/api.v1.Users/Get".to_string());

    let mut event2 = create_test_event(TransportKind::Grpc, Direction::Inbound, 1050000000);
    event2.metadata.insert("h2.stream_id".to_string(), "1".to_string());
    event2.metadata.insert("grpc.status".to_string(), "0".to_string());

    let events = vec![event1, event2];
    let result = engine.build_conversations(&events).expect("should succeed");

    assert_eq!(result.conversations.len(), 1);
    let conv = &result.conversations[0];
    assert_eq!(conv.protocol, TransportKind::Grpc);
    assert_eq!(conv.event_ids.len(), 2);
    assert_eq!(conv.metadata.get("grpc.method"), Some(&"/api.v1.Users/Get".to_string()));
}

#[test]
fn test_conversation_engine_build_conversations_multiple_protocols() {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));
    engine.register(Box::new(MockZmqStrategy));

    let mut grpc_event = create_test_event(TransportKind::Grpc, Direction::Outbound, 1000000000);
    grpc_event.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let mut zmq_event = create_test_event(TransportKind::Zmq, Direction::Outbound, 2000000000);
    zmq_event.metadata.insert("zmq.topic".to_string(), "market.data".to_string());

    let events = vec![grpc_event, zmq_event];
    let result = engine.build_conversations(&events).expect("should succeed");

    assert_eq!(result.conversations.len(), 2);

    // Verify both protocols are present
    let protocols: Vec<TransportKind> = result.conversations.iter().map(|c| c.protocol).collect();
    assert!(protocols.contains(&TransportKind::Grpc));
    assert!(protocols.contains(&TransportKind::Zmq));
}

#[test]
fn test_conversation_engine_build_conversations_multiple_streams() {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));

    // Create events for two different streams
    let mut event1 = create_test_event(TransportKind::Grpc, Direction::Outbound, 1000000000);
    event1.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let mut event2 = create_test_event(TransportKind::Grpc, Direction::Inbound, 1050000000);
    event2.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let mut event3 = create_test_event(TransportKind::Grpc, Direction::Outbound, 2000000000);
    event3.metadata.insert("h2.stream_id".to_string(), "3".to_string());

    let mut event4 = create_test_event(TransportKind::Grpc, Direction::Inbound, 2050000000);
    event4.metadata.insert("h2.stream_id".to_string(), "3".to_string());

    let events = vec![event1, event2, event3, event4];
    let result = engine.build_conversations(&events).expect("should succeed");

    assert_eq!(result.conversations.len(), 2);
    assert_eq!(result.conversations[0].event_ids.len(), 2);
    assert_eq!(result.conversations[1].event_ids.len(), 2);
}

#[test]
fn test_conversation_engine_fallback_unclaimed_events() {
    let engine = ConversationEngine::new(); // No strategies registered

    // Create events that won't be claimed by any strategy
    let event1 = create_test_event(TransportKind::RawTcp, Direction::Outbound, 1000000000);
    let event2 = create_test_event(TransportKind::RawTcp, Direction::Inbound, 1050000000);

    let events = vec![event1, event2];
    let result = engine.build_conversations(&events).expect("should succeed");

    // Should create fallback conversation grouped by address
    assert_eq!(result.conversations.len(), 1);
    assert_eq!(result.conversations[0].kind, ConversationKind::TcpStream);
    assert_eq!(result.conversations[0].state, ConversationState::Incomplete);
}

#[test]
fn test_conversation_engine_mixed_claimed_and_unclaimed() {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));

    let mut grpc_event = create_test_event(TransportKind::Grpc, Direction::Outbound, 1000000000);
    grpc_event.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let tcp_event = create_test_event(TransportKind::RawTcp, Direction::Outbound, 2000000000);

    let events = vec![grpc_event, tcp_event];
    let result = engine.build_conversations(&events).expect("should succeed");

    // Should have one gRPC conversation and one fallback TCP conversation
    assert_eq!(result.conversations.len(), 2);
}

#[test]
fn test_conversation_set_for_event() {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));

    let mut event = create_test_event(TransportKind::Grpc, Direction::Outbound, 1000000000);
    event.metadata.insert("h2.stream_id".to_string(), "1".to_string());
    let event_id = event.id;

    let events = vec![event];
    let set = engine.build_conversations(&events).expect("should succeed");

    let conv = set.for_event(event_id);
    assert!(conv.is_some());
    assert_eq!(conv.unwrap().event_ids[0], event_id);
}

#[test]
fn test_conversation_set_for_event_not_found() {
    let engine = ConversationEngine::new();
    let events: Vec<DebugEvent> = vec![];
    let set = engine.build_conversations(&events).expect("should succeed");

    let conv = set.for_event(EventId::next());
    assert!(conv.is_none());
}

#[test]
fn test_conversation_set_sorted_by_time() {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));

    // Create events with different timestamps (not in order)
    let mut event1 = create_test_event(TransportKind::Grpc, Direction::Outbound, 3000000000);
    event1.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let mut event2 = create_test_event(TransportKind::Grpc, Direction::Outbound, 1000000000);
    event2.metadata.insert("h2.stream_id".to_string(), "2".to_string());

    let mut event3 = create_test_event(TransportKind::Grpc, Direction::Outbound, 2000000000);
    event3.metadata.insert("h2.stream_id".to_string(), "3".to_string());

    let events = vec![event1, event2, event3];
    let set = engine.build_conversations(&events).expect("should succeed");

    let sorted = set.sorted_by_time();
    assert_eq!(sorted.len(), 3);

    // Should be sorted by start time
    assert!(sorted[0].metrics.start_time <= sorted[1].metrics.start_time);
    assert!(sorted[1].metrics.start_time <= sorted[2].metrics.start_time);
}

#[test]
fn test_conversation_set_by_protocol() {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));
    engine.register(Box::new(MockZmqStrategy));

    let mut grpc1 = create_test_event(TransportKind::Grpc, Direction::Outbound, 1000000000);
    grpc1.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let mut grpc2 = create_test_event(TransportKind::Grpc, Direction::Outbound, 2000000000);
    grpc2.metadata.insert("h2.stream_id".to_string(), "2".to_string());

    let mut zmq = create_test_event(TransportKind::Zmq, Direction::Outbound, 3000000000);
    zmq.metadata.insert("zmq.topic".to_string(), "test".to_string());

    let events = vec![grpc1, grpc2, zmq];
    let set = engine.build_conversations(&events).expect("should succeed");

    let grpc_convs = set.by_protocol(TransportKind::Grpc);
    assert_eq!(grpc_convs.len(), 2);

    let zmq_convs = set.by_protocol(TransportKind::Zmq);
    assert_eq!(zmq_convs.len(), 1);

    let dds_convs = set.by_protocol(TransportKind::DdsRtps);
    assert_eq!(dds_convs.len(), 0);
}

#[test]
fn test_conversation_set_stats() {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));

    let mut event1 = create_test_event(TransportKind::Grpc, Direction::Outbound, 1000000000);
    event1.metadata.insert("h2.stream_id".to_string(), "1".to_string());
    event1.metadata.insert("grpc.status".to_string(), "0".to_string());

    let mut event2 = create_test_event(TransportKind::Grpc, Direction::Outbound, 2000000000);
    event2.metadata.insert("h2.stream_id".to_string(), "2".to_string());
    event2.metadata.insert("grpc.status".to_string(), "14".to_string()); // Error

    let events = vec![event1, event2];
    let set = engine.build_conversations(&events).expect("should succeed");

    let stats = set.stats();
    assert_eq!(stats.total, 2);
    assert_eq!(stats.by_protocol.get(&TransportKind::Grpc), Some(&2));
}

#[test]
fn test_conversation_set_stats_multiple_protocols() {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));
    engine.register(Box::new(MockZmqStrategy));

    let mut grpc = create_test_event(TransportKind::Grpc, Direction::Outbound, 1000000000);
    grpc.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let mut zmq = create_test_event(TransportKind::Zmq, Direction::Outbound, 2000000000);
    zmq.metadata.insert("zmq.topic".to_string(), "test".to_string());

    let events = vec![grpc, zmq];
    let set = engine.build_conversations(&events).expect("should succeed");

    let stats = set.stats();
    assert_eq!(stats.total, 2);
    assert_eq!(stats.by_protocol.get(&TransportKind::Grpc), Some(&1));
    assert_eq!(stats.by_protocol.get(&TransportKind::Zmq), Some(&1));
}

#[test]
fn test_conversation_engine_default() {
    let engine = ConversationEngine::default();
    assert_eq!(engine.strategies.len(), 0);
}

#[test]
fn test_conversation_for_event() {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));

    let mut event = create_test_event(TransportKind::Grpc, Direction::Outbound, 1000000000);
    event.metadata.insert("h2.stream_id".to_string(), "1".to_string());
    let event_id = event.id;

    let events = vec![event];
    let set = engine.build_conversations(&events).expect("should succeed");

    let conv = engine.conversation_for_event(&set, event_id);
    assert!(conv.is_some());
}

#[test]
fn test_flow_to_conversation_grpc() {
    let mut event1 = create_test_event(TransportKind::Grpc, Direction::Outbound, 1000000000);
    event1.metadata.insert("grpc.method".to_string(), "/api.v1.Users/Get".to_string());

    let mut event2 = create_test_event(TransportKind::Grpc, Direction::Inbound, 1050000000);
    event2.metadata.insert("grpc.status".to_string(), "0".to_string());

    let events = vec![event1, event2];
    let event_refs: Vec<&DebugEvent> = events.iter().collect();

    let mut flow = Flow::new("grpc:test");
    for e in &event_refs {
        flow = flow.add_event(e);
    }
    flow = flow.add_metadata("grpc.method", "/api.v1.Users/Get");

    let conv = engine::flow_to_conversation(flow, TransportKind::Grpc).expect("should succeed");

    assert_eq!(conv.protocol, TransportKind::Grpc);
    assert_eq!(conv.kind, ConversationKind::UnaryRpc); // 1 outbound, 1 inbound
    assert_eq!(conv.state, ConversationState::Complete);
    assert_eq!(conv.event_ids.len(), 2);
}

#[test]
fn test_classify_grpc_conversation_kinds() {
    // Unary: 1 request, 1 response
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));

    let mut req = create_test_event(TransportKind::Grpc, Direction::Outbound, 1000000000);
    req.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let mut resp = create_test_event(TransportKind::Grpc, Direction::Inbound, 1050000000);
    resp.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let events = vec![req, resp];
    let set = engine.build_conversations(&events).expect("should succeed");
    assert_eq!(set.conversations[0].kind, ConversationKind::UnaryRpc);
}

#[test]
fn test_classify_grpc_server_streaming() {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));

    let mut req = create_test_event(TransportKind::Grpc, Direction::Outbound, 1000000000);
    req.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    // Multiple responses
    let mut resp1 = create_test_event(TransportKind::Grpc, Direction::Inbound, 1050000000);
    resp1.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let mut resp2 = create_test_event(TransportKind::Grpc, Direction::Inbound, 1100000000);
    resp2.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let events = vec![req, resp1, resp2];
    let set = engine.build_conversations(&events).expect("should succeed");
    assert_eq!(set.conversations[0].kind, ConversationKind::ServerStreaming);
}

#[test]
fn test_classify_grpc_client_streaming() {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));

    // Multiple requests
    let mut req1 = create_test_event(TransportKind::Grpc, Direction::Outbound, 1000000000);
    req1.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let mut req2 = create_test_event(TransportKind::Grpc, Direction::Outbound, 1050000000);
    req2.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let mut resp = create_test_event(TransportKind::Grpc, Direction::Inbound, 1100000000);
    resp.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let events = vec![req1, req2, resp];
    let set = engine.build_conversations(&events).expect("should succeed");
    assert_eq!(set.conversations[0].kind, ConversationKind::ClientStreaming);
}

#[test]
fn test_classify_grpc_bidirectional_streaming() {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));

    // Multiple requests and multiple responses
    let mut req1 = create_test_event(TransportKind::Grpc, Direction::Outbound, 1000000000);
    req1.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let mut req2 = create_test_event(TransportKind::Grpc, Direction::Outbound, 1050000000);
    req2.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let mut resp1 = create_test_event(TransportKind::Grpc, Direction::Inbound, 1100000000);
    resp1.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let mut resp2 = create_test_event(TransportKind::Grpc, Direction::Inbound, 1150000000);
    resp2.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let events = vec![req1, req2, resp1, resp2];
    let set = engine.build_conversations(&events).expect("should succeed");
    assert_eq!(set.conversations[0].kind, ConversationKind::BidirectionalStreaming);
}

#[test]
fn test_conversation_state_timeout() {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));

    // Only outbound, no response
    let mut req = create_test_event(TransportKind::Grpc, Direction::Outbound, 1000000000);
    req.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let events = vec![req];
    let set = engine.build_conversations(&events).expect("should succeed");
    assert_eq!(set.conversations[0].state, ConversationState::Timeout);
}

#[test]
fn test_conversation_state_incomplete() {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));

    // Only inbound, no request
    let mut resp = create_test_event(TransportKind::Grpc, Direction::Inbound, 1000000000);
    resp.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let events = vec![resp];
    let set = engine.build_conversations(&events).expect("should succeed");
    assert_eq!(set.conversations[0].state, ConversationState::Incomplete);
}

#[test]
fn test_conversation_state_error() {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(MockGrpcStrategy));

    let mut req = create_test_event(TransportKind::Grpc, Direction::Outbound, 1000000000);
    req.metadata.insert("h2.stream_id".to_string(), "1".to_string());

    let mut resp = create_test_event(TransportKind::Grpc, Direction::Inbound, 1050000000);
    resp.metadata.insert("h2.stream_id".to_string(), "1".to_string());
    resp.metadata.insert("grpc.status".to_string(), "14".to_string()); // Error status

    let events = vec![req, resp];
    let set = engine.build_conversations(&events).expect("should succeed");
    assert_eq!(set.conversations[0].state, ConversationState::Error);
}
