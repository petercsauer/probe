//! gRPC conversation correlation strategy.
//!
//! Groups gRPC events by connection (src/dst) and H2 stream ID.

use indexmap::IndexMap;
use prb_core::{
    CoreError, CorrelationStrategy, DebugEvent, Flow, METADATA_KEY_GRPC_METHOD,
    METADATA_KEY_H2_STREAM_ID, TransportKind,
};
use std::collections::BTreeMap;

/// gRPC correlation strategy.
///
/// Groups events by (network.src, network.dst, `h2.stream_id`).
pub struct GrpcCorrelationStrategy;

impl CorrelationStrategy for GrpcCorrelationStrategy {
    fn transport(&self) -> TransportKind {
        TransportKind::Grpc
    }

    fn correlate<'a>(&self, events: &'a [DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError> {
        // Group events by (src, dst, stream_id)
        let mut groups: IndexMap<String, Vec<&'a DebugEvent>> = IndexMap::new();

        for event in events {
            let key = grouping_key(event);
            groups.entry(key).or_default().push(event);
        }

        // Convert groups to flows
        let mut flows = Vec::new();
        for (key, mut group) in groups {
            // Sort by timestamp
            group.sort_by_key(|e| e.timestamp);

            // Extract metadata
            let mut metadata = BTreeMap::new();

            // Extract gRPC method from first event with it
            if let Some(method) = group
                .iter()
                .find_map(|e| e.metadata.get(METADATA_KEY_GRPC_METHOD))
            {
                metadata.insert("grpc.method".to_string(), method.clone());
            }

            // Extract authority
            if let Some(authority) = group.iter().find_map(|e| e.metadata.get("h2.authority")) {
                metadata.insert("grpc.authority".to_string(), authority.clone());
            }

            // Extract gRPC status from trailers
            if let Some(status) = group.iter().find_map(|e| e.metadata.get("grpc.status")) {
                metadata.insert("grpc.status".to_string(), status.clone());
            }

            // Extract gRPC message from trailers
            if let Some(message) = group.iter().find_map(|e| e.metadata.get("grpc.message")) {
                metadata.insert("grpc.message".to_string(), message.clone());
            }

            // Add connection info
            if let Some(event) = group.first()
                && let Some(ref network) = event.source.network
            {
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
}

/// Generate grouping key for a gRPC event.
fn grouping_key(event: &DebugEvent) -> String {
    let stream_id = event
        .metadata
        .get(METADATA_KEY_H2_STREAM_ID)
        .map_or("?", std::string::String::as_str);

    if let Some(ref network) = event.source.network {
        format!("grpc:{}->{}/ s{}", network.src, network.dst, stream_id)
    } else {
        format!("grpc:unknown/s{stream_id}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prb_core::{
        DebugEvent, Direction, EventSource, NetworkAddr, Payload, Timestamp, TransportKind,
    };

    fn make_grpc_event(
        src: &str,
        dst: &str,
        stream_id: u32,
        direction: Direction,
        method: Option<&str>,
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
            .transport(TransportKind::Grpc)
            .direction(direction)
            .payload(Payload::Raw {
                raw: bytes::Bytes::from("test"),
            })
            .timestamp(Timestamp::from_nanos(1000))
            .metadata("h2.stream_id", stream_id.to_string());

        if let Some(m) = method {
            builder = builder.metadata("grpc.method", m);
        }

        builder.build()
    }

    #[test]
    fn test_single_unary_conversation() {
        let events = vec![
            make_grpc_event(
                "10.0.0.1:50051",
                "10.0.0.2:8080",
                3,
                Direction::Outbound,
                Some("POST /api.v1.Users/Get"),
            ),
            make_grpc_event(
                "10.0.0.1:50051",
                "10.0.0.2:8080",
                3,
                Direction::Inbound,
                None,
            ),
        ];

        let strategy = GrpcCorrelationStrategy;
        let flows = strategy.correlate(&events).unwrap();

        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].events.len(), 2);
        assert_eq!(
            flows[0].metadata.get("grpc.method"),
            Some(&"POST /api.v1.Users/Get".to_string())
        );
    }

    #[test]
    fn test_two_interleaved_streams() {
        let events = vec![
            make_grpc_event(
                "10.0.0.1:50051",
                "10.0.0.2:8080",
                3,
                Direction::Outbound,
                Some("POST /api.v1.Users/Get"),
            ),
            make_grpc_event(
                "10.0.0.1:50051",
                "10.0.0.2:8080",
                5,
                Direction::Outbound,
                Some("POST /api.v1.Orders/List"),
            ),
            make_grpc_event(
                "10.0.0.1:50051",
                "10.0.0.2:8080",
                3,
                Direction::Inbound,
                None,
            ),
            make_grpc_event(
                "10.0.0.1:50051",
                "10.0.0.2:8080",
                5,
                Direction::Inbound,
                None,
            ),
        ];

        let strategy = GrpcCorrelationStrategy;
        let flows = strategy.correlate(&events).unwrap();

        assert_eq!(flows.len(), 2);
        assert_eq!(flows[0].events.len(), 2);
        assert_eq!(flows[1].events.len(), 2);
    }

    #[test]
    fn test_different_connections_same_stream_id() {
        let events = vec![
            make_grpc_event(
                "10.0.0.1:50051",
                "10.0.0.2:8080",
                3,
                Direction::Outbound,
                Some("POST /api.v1.Users/Get"),
            ),
            make_grpc_event(
                "10.0.0.3:60000",
                "10.0.0.2:8080",
                3,
                Direction::Outbound,
                Some("POST /api.v1.Orders/List"),
            ),
        ];

        let strategy = GrpcCorrelationStrategy;
        let flows = strategy.correlate(&events).unwrap();

        // Different connections → separate flows
        assert_eq!(flows.len(), 2);
    }
}
