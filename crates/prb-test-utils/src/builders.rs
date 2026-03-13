//! Builder factories with test-friendly defaults for `DebugEvent`.

use bytes::Bytes;
use prb_core::{
    DebugEvent, DebugEventBuilder, Direction, EventSource, NetworkAddr, Payload, Timestamp,
};

/// Returns a pre-configured builder with test-friendly defaults.
///
/// Defaults:
/// - timestamp: 1_000_000_000 nanos (1970-01-01 00:00:01)
/// - source.adapter: "test"
/// - source.origin: "test"
/// - network: 10.0.0.1:1234 → 10.0.0.2:5678
/// - direction: Inbound
/// - payload: Raw(b"test")
///
/// # Example
/// ```
/// use prb_test_utils::event_builder;
/// use prb_core::TransportKind;
///
/// let evt = event_builder()
///     .transport(TransportKind::Grpc)
///     .build();
/// ```
pub fn event_builder() -> DebugEventBuilder {
    DebugEvent::builder()
        .timestamp(Timestamp::from_nanos(1_000_000_000))
        .source(EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: Some(NetworkAddr {
                src: "10.0.0.1:1234".to_string(),
                dst: "10.0.0.2:5678".to_string(),
            }),
        })
        .direction(Direction::Inbound)
        .payload(Payload::Raw {
            raw: Bytes::from(b"test".to_vec()),
        })
}

/// Returns a builder with custom network addresses.
///
/// # Example
/// ```
/// use prb_test_utils::event_builder_with_network;
/// use prb_core::TransportKind;
///
/// let evt = event_builder_with_network("192.168.1.1:8080", "192.168.1.2:9090")
///     .transport(TransportKind::Grpc)
///     .build();
/// ```
pub fn event_builder_with_network(src: &str, dst: &str) -> DebugEventBuilder {
    DebugEvent::builder()
        .timestamp(Timestamp::from_nanos(1_000_000_000))
        .source(EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: Some(NetworkAddr {
                src: src.to_string(),
                dst: dst.to_string(),
            }),
        })
        .direction(Direction::Inbound)
        .payload(Payload::Raw {
            raw: Bytes::from(b"test".to_vec()),
        })
}
