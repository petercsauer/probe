//! ZMQ conversation correlation strategy.
//!
//! Groups ZMQ events by socket pattern (PUB/SUB, REQ/REP, PUSH/PULL).

use indexmap::IndexMap;
use prb_core::{
    CoreError, CorrelationStrategy, DebugEvent, Direction, Flow, METADATA_KEY_ZMQ_TOPIC,
    TransportKind,
};
use std::collections::BTreeMap;

/// ZMQ correlation strategy.
///
/// Grouping strategy depends on socket type:
/// - PUB/SUB: group by topic
/// - REQ/REP: group by connection_id + temporal pairing
/// - PUSH/PULL: group by connection_id
/// - DEALER/ROUTER: group by identity or connection_id
pub struct ZmqCorrelationStrategy;

impl CorrelationStrategy for ZmqCorrelationStrategy {
    fn transport(&self) -> TransportKind {
        TransportKind::Zmq
    }

    fn correlate<'a>(&self, events: &'a [DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError> {
        // Partition by socket type
        let mut pub_sub_events = Vec::new();
        let mut req_rep_events = Vec::new();
        let mut push_pull_events = Vec::new();
        let mut other_events = Vec::new();

        for event in events {
            match event.metadata.get("zmq.socket_type").map(|s| s.as_str()) {
                Some("PUB") | Some("SUB") => pub_sub_events.push(event),
                Some("REQ") | Some("REP") | Some("DEALER") | Some("ROUTER") => {
                    req_rep_events.push(event)
                }
                Some("PUSH") | Some("PULL") => push_pull_events.push(event),
                _ => other_events.push(event),
            }
        }

        let mut flows = Vec::new();

        // Handle PUB/SUB: group by topic
        if !pub_sub_events.is_empty() {
            flows.extend(correlate_pub_sub(&pub_sub_events)?);
        }

        // Handle REQ/REP: group by connection and pair temporally
        if !req_rep_events.is_empty() {
            flows.extend(correlate_req_rep(&req_rep_events)?);
        }

        // Handle PUSH/PULL: group by connection
        if !push_pull_events.is_empty() {
            flows.extend(correlate_push_pull(&push_pull_events)?);
        }

        // Fallback: group by connection
        if !other_events.is_empty() {
            flows.extend(correlate_fallback(&other_events)?);
        }

        Ok(flows)
    }
}

/// Correlate PUB/SUB events by topic.
fn correlate_pub_sub<'a>(events: &[&'a DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError> {
    let mut groups: IndexMap<String, Vec<&'a DebugEvent>> = IndexMap::new();

    for event in events {
        let topic = event
            .metadata
            .get(METADATA_KEY_ZMQ_TOPIC)
            .map(|s| s.as_str())
            .unwrap_or("(no-topic)");

        let key = format!("zmq:pubsub:{}", topic);
        groups.entry(key).or_default().push(event);
    }

    let mut flows = Vec::new();
    for (key, mut group) in groups {
        group.sort_by_key(|e| e.timestamp);

        let mut metadata = BTreeMap::new();
        if let Some(topic) = group
            .iter()
            .find_map(|e| e.metadata.get(METADATA_KEY_ZMQ_TOPIC))
        {
            metadata.insert("zmq.topic".to_string(), topic.clone());
        }
        if let Some(socket_type) = group.iter().find_map(|e| e.metadata.get("zmq.socket_type")) {
            metadata.insert("zmq.socket_type".to_string(), socket_type.clone());
        }

        let mut flow = Flow::new(key);
        for event in group {
            flow = flow.add_event(event);
        }
        for (k, v) in metadata {
            flow = flow.add_metadata(k, v);
        }

        flows.push(flow);
    }

    Ok(flows)
}

/// Correlate REQ/REP events by connection and pair temporally.
fn correlate_req_rep<'a>(events: &[&'a DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError> {
    // Group by connection first
    let mut by_connection: IndexMap<String, Vec<&'a DebugEvent>> = IndexMap::new();

    for event in events {
        let conn_key = connection_key(event);
        by_connection.entry(conn_key).or_default().push(event);
    }

    let mut flows = Vec::new();

    // For each connection, pair OUT/IN events temporally
    for (conn_key, mut conn_events) in by_connection {
        conn_events.sort_by_key(|e| e.timestamp);

        let mut pair_idx = 0;
        let mut i = 0;

        while i < conn_events.len() {
            let event = conn_events[i];

            if event.direction == Direction::Outbound {
                // Start a new pair
                let mut pair = vec![event];
                let pair_key = format!("{}:rr{}", conn_key, pair_idx);
                pair_idx += 1;

                // Look for matching inbound
                let mut j = i + 1;
                while j < conn_events.len() {
                    if conn_events[j].direction == Direction::Inbound {
                        pair.push(conn_events[j]);
                        i = j; // Skip to the inbound event
                        break;
                    }
                    j += 1;
                }

                // Create flow for this pair
                let mut metadata = BTreeMap::new();
                if let Some(socket_type) =
                    pair.iter().find_map(|e| e.metadata.get("zmq.socket_type"))
                {
                    metadata.insert("zmq.socket_type".to_string(), socket_type.clone());
                }
                if let Some(ref network) = pair[0].source.network {
                    metadata.insert(
                        "connection".to_string(),
                        format!("{} → {}", network.src, network.dst),
                    );
                }

                let mut flow = Flow::new(pair_key);
                for e in pair {
                    flow = flow.add_event(e);
                }
                for (k, v) in metadata {
                    flow = flow.add_metadata(k, v);
                }

                flows.push(flow);
            }

            i += 1;
        }
    }

    Ok(flows)
}

/// Correlate PUSH/PULL events by connection.
fn correlate_push_pull<'a>(events: &[&'a DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError> {
    let mut groups: IndexMap<String, Vec<&'a DebugEvent>> = IndexMap::new();

    for event in events {
        let key = format!("zmq:pushpull:{}", connection_key(event));
        groups.entry(key).or_default().push(event);
    }

    let mut flows = Vec::new();
    for (key, mut group) in groups {
        group.sort_by_key(|e| e.timestamp);

        let mut metadata = BTreeMap::new();
        if let Some(socket_type) = group.iter().find_map(|e| e.metadata.get("zmq.socket_type")) {
            metadata.insert("zmq.socket_type".to_string(), socket_type.clone());
        }
        if let Some(ref network) = group[0].source.network {
            metadata.insert(
                "connection".to_string(),
                format!("{} → {}", network.src, network.dst),
            );
        }

        let mut flow = Flow::new(key);
        for event in group {
            flow = flow.add_event(event);
        }
        for (k, v) in metadata {
            flow = flow.add_metadata(k, v);
        }

        flows.push(flow);
    }

    Ok(flows)
}

/// Fallback correlation for unknown socket types.
fn correlate_fallback<'a>(events: &[&'a DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError> {
    let mut groups: IndexMap<String, Vec<&'a DebugEvent>> = IndexMap::new();

    for event in events {
        let key = format!("zmq:unknown:{}", connection_key(event));
        groups.entry(key).or_default().push(event);
    }

    let mut flows = Vec::new();
    for (key, mut group) in groups {
        group.sort_by_key(|e| e.timestamp);

        let mut flow = Flow::new(key);
        for event in group {
            flow = flow.add_event(event);
        }

        flows.push(flow);
    }

    Ok(flows)
}

/// Generate connection key from event.
fn connection_key(event: &DebugEvent) -> String {
    if let Some(ref network) = event.source.network {
        format!("{}:{}", network.src, network.dst)
    } else {
        event.source.origin.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prb_core::{DebugEvent, Direction, EventSource, NetworkAddr, Payload, Timestamp};

    fn make_zmq_event(
        src: &str,
        dst: &str,
        direction: Direction,
        socket_type: Option<&str>,
        topic: Option<&str>,
    ) -> DebugEvent {
        let mut builder = DebugEvent::builder()
            .source(EventSource {
                adapter: "test".to_string(),
                origin: "test.pcap".to_string(),
                network: Some(NetworkAddr {
                    src: src.to_string(),
                    dst: dst.to_string(),
                }),
            })
            .transport(TransportKind::Zmq)
            .direction(direction)
            .payload(Payload::Raw {
                raw: bytes::Bytes::from("test"),
            })
            .timestamp(Timestamp::from_nanos(1000));

        if let Some(st) = socket_type {
            builder = builder.metadata("zmq.socket_type", st);
        }
        if let Some(t) = topic {
            builder = builder.metadata("zmq.topic", t);
        }

        builder.build()
    }

    #[test]
    fn test_pub_sub_same_topic() {
        let events = vec![
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Outbound,
                Some("PUB"),
                Some("market.data"),
            ),
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Outbound,
                Some("PUB"),
                Some("market.data"),
            ),
        ];

        let strategy = ZmqCorrelationStrategy;
        let flows = strategy.correlate(&events).unwrap();

        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].events.len(), 2);
        assert_eq!(
            flows[0].metadata.get("zmq.topic"),
            Some(&"market.data".to_string())
        );
    }

    #[test]
    fn test_pub_sub_different_topics() {
        let events = vec![
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Outbound,
                Some("PUB"),
                Some("topic1"),
            ),
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Outbound,
                Some("PUB"),
                Some("topic2"),
            ),
        ];

        let strategy = ZmqCorrelationStrategy;
        let flows = strategy.correlate(&events).unwrap();

        assert_eq!(flows.len(), 2);
    }

    #[test]
    fn test_req_rep_pairing() {
        let events = vec![
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Outbound,
                Some("REQ"),
                None,
            ),
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Inbound,
                Some("REP"),
                None,
            ),
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Outbound,
                Some("REQ"),
                None,
            ),
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Inbound,
                Some("REP"),
                None,
            ),
        ];

        let strategy = ZmqCorrelationStrategy;
        let flows = strategy.correlate(&events).unwrap();

        // Two REQ/REP pairs
        assert_eq!(flows.len(), 2);
        assert_eq!(flows[0].events.len(), 2);
        assert_eq!(flows[1].events.len(), 2);
    }

    #[test]
    fn test_req_without_rep() {
        let events = vec![
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Outbound,
                Some("REQ"),
                None,
            ),
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Outbound,
                Some("REQ"),
                None,
            ),
        ];

        let strategy = ZmqCorrelationStrategy;
        let flows = strategy.correlate(&events).unwrap();

        // Two unpaired requests (timeout scenarios)
        assert_eq!(flows.len(), 2);
        assert_eq!(flows[0].events.len(), 1);
        assert_eq!(flows[1].events.len(), 1);
    }

    #[test]
    fn test_dealer_router_pattern() {
        let events = vec![
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Outbound,
                Some("DEALER"),
                None,
            ),
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Inbound,
                Some("ROUTER"),
                None,
            ),
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Outbound,
                Some("DEALER"),
                None,
            ),
        ];

        let strategy = ZmqCorrelationStrategy;
        let flows = strategy.correlate(&events).unwrap();

        // DEALER/ROUTER should be correlated by connection
        assert!(!flows.is_empty());
    }

    #[test]
    fn test_push_pull_pattern() {
        let events = vec![
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Outbound,
                Some("PUSH"),
                None,
            ),
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Inbound,
                Some("PULL"),
                None,
            ),
        ];

        let strategy = ZmqCorrelationStrategy;
        let flows = strategy.correlate(&events).unwrap();

        assert!(!flows.is_empty());
    }

    #[test]
    fn test_pub_sub_multiple_topics() {
        let events = vec![
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Outbound,
                Some("PUB"),
                Some("topic.a"),
            ),
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Outbound,
                Some("PUB"),
                Some("topic.b"),
            ),
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Outbound,
                Some("PUB"),
                Some("topic.a"),
            ),
        ];

        let strategy = ZmqCorrelationStrategy;
        let flows = strategy.correlate(&events).unwrap();

        // Should have 2 flows (one per topic)
        assert_eq!(flows.len(), 2);
        let topic_a_flow = flows
            .iter()
            .find(|f| f.metadata.get("zmq.topic") == Some(&"topic.a".to_string()));
        assert!(topic_a_flow.is_some());
        assert_eq!(topic_a_flow.unwrap().events.len(), 2);
    }

    #[test]
    fn test_fallback_unknown_socket_type() {
        let events = vec![
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Outbound,
                None, // No socket type
                None,
            ),
            make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                Direction::Inbound,
                None,
                None,
            ),
        ];

        let strategy = ZmqCorrelationStrategy;
        let flows = strategy.correlate(&events).unwrap();

        // Should use fallback correlation
        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].events.len(), 2);
    }

    #[test]
    fn test_conversation_state_transitions() {
        // Test REQ -> REP -> REQ -> REP sequence
        let mut events = vec![];
        for i in 0..4 {
            let socket_type = if i % 2 == 0 { "REQ" } else { "REP" };
            let direction = if i % 2 == 0 {
                Direction::Outbound
            } else {
                Direction::Inbound
            };
            events.push(make_zmq_event(
                "10.0.0.1:5555",
                "10.0.0.2:5556",
                direction,
                Some(socket_type),
                None,
            ));
        }

        let strategy = ZmqCorrelationStrategy;
        let flows = strategy.correlate(&events).unwrap();

        // Should have 2 REQ/REP conversation pairs
        assert_eq!(flows.len(), 2);
    }
}
