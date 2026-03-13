//! Preset event factories for common test scenarios.

use prb_core::{DebugEvent, Direction, EventId, TransportKind};

use crate::event_builder;

/// Creates a minimal test event with default values.
///
/// # Example
/// ```
/// use prb_test_utils::event;
/// use prb_core::Direction;
///
/// let evt = event();
/// assert_eq!(evt.direction, Direction::Inbound);
/// ```
pub fn event() -> DebugEvent {
    event_builder()
        .transport(TransportKind::Grpc)
        .build()
}

/// Creates a gRPC test event with the given ID.
///
/// # Example
/// ```
/// use prb_test_utils::grpc_event;
/// use prb_core::{EventId, TransportKind};
///
/// let evt = grpc_event(42);
/// assert_eq!(evt.id, EventId::from_raw(42));
/// assert_eq!(evt.transport, TransportKind::Grpc);
/// ```
pub fn grpc_event(id: u64) -> DebugEvent {
    event_builder()
        .id(EventId::from_raw(id))
        .transport(TransportKind::Grpc)
        .build()
}

/// Creates a ZMQ test event with the given ID.
///
/// # Example
/// ```
/// use prb_test_utils::zmq_event;
/// use prb_core::{Direction, EventId, TransportKind};
///
/// let evt = zmq_event(42);
/// assert_eq!(evt.id, EventId::from_raw(42));
/// assert_eq!(evt.transport, TransportKind::Zmq);
/// assert_eq!(evt.direction, Direction::Outbound);
/// ```
pub fn zmq_event(id: u64) -> DebugEvent {
    event_builder()
        .id(EventId::from_raw(id))
        .transport(TransportKind::Zmq)
        .direction(Direction::Outbound)
        .build()
}

/// Creates a raw TCP test event with the given ID.
///
/// # Example
/// ```
/// use prb_test_utils::tcp_event;
/// use prb_core::{EventId, TransportKind};
///
/// let evt = tcp_event(42);
/// assert_eq!(evt.id, EventId::from_raw(42));
/// assert_eq!(evt.transport, TransportKind::RawTcp);
/// ```
pub fn tcp_event(id: u64) -> DebugEvent {
    event_builder()
        .id(EventId::from_raw(id))
        .transport(TransportKind::RawTcp)
        .build()
}

/// Creates a DDS RTPS test event with the given ID.
///
/// # Example
/// ```
/// use prb_test_utils::dds_event;
/// use prb_core::{EventId, TransportKind};
///
/// let evt = dds_event(42);
/// assert_eq!(evt.id, EventId::from_raw(42));
/// assert_eq!(evt.transport, TransportKind::DdsRtps);
/// ```
pub fn dds_event(id: u64) -> DebugEvent {
    event_builder()
        .id(EventId::from_raw(id))
        .transport(TransportKind::DdsRtps)
        .build()
}
