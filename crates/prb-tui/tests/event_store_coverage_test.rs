//! Additional tests for `event_store.rs` to improve coverage

use bytes::Bytes;
use prb_core::{
    DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind,
};
use prb_query::Filter;
use prb_tui::event_store::EventStore;
use std::collections::BTreeMap;

fn make_event(id: u64, ts: u64, transport: TransportKind) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(ts),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: Some(NetworkAddr {
                src: "10.0.0.1:1234".to_string(),
                dst: "10.0.0.2:5678".to_string(),
            }),
        },
        transport,
        direction: Direction::Inbound,
        payload: Payload::Raw {
            raw: Bytes::from(vec![1, 2, 3]),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}

#[test]
fn test_event_store_get_valid_index() {
    let events = vec![
        make_event(1, 1_000_000_000, TransportKind::Grpc),
        make_event(2, 2_000_000_000, TransportKind::Zmq),
    ];
    let store = EventStore::new(events);

    let event = store.get(0);
    assert!(event.is_some());
    assert_eq!(event.unwrap().id.as_u64(), 1);

    let event2 = store.get(1);
    assert!(event2.is_some());
    assert_eq!(event2.unwrap().id.as_u64(), 2);
}

#[test]
fn test_event_store_get_invalid_index() {
    let events = vec![make_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);

    let event = store.get(99);
    assert!(event.is_none());
}

#[test]
fn test_event_store_len() {
    let events = vec![
        make_event(1, 1_000_000_000, TransportKind::Grpc),
        make_event(2, 2_000_000_000, TransportKind::Zmq),
        make_event(3, 3_000_000_000, TransportKind::DdsRtps),
    ];
    let store = EventStore::new(events);

    assert_eq!(store.len(), 3);
}

#[test]
fn test_event_store_is_empty() {
    let empty_store = EventStore::new(vec![]);
    assert!(empty_store.is_empty());

    let non_empty_store = EventStore::new(vec![make_event(1, 1_000_000_000, TransportKind::Grpc)]);
    assert!(!non_empty_store.is_empty());
}

#[test]
fn test_event_store_all_indices() {
    let events = vec![
        make_event(1, 1_000_000_000, TransportKind::Grpc),
        make_event(2, 2_000_000_000, TransportKind::Zmq),
        make_event(3, 3_000_000_000, TransportKind::DdsRtps),
    ];
    let store = EventStore::new(events);

    let indices = store.all_indices();
    assert_eq!(indices, vec![0, 1, 2]);
}

#[test]
fn test_event_store_filter_indices_no_match() {
    let events = vec![
        make_event(1, 1_000_000_000, TransportKind::Grpc),
        make_event(2, 2_000_000_000, TransportKind::Grpc),
    ];
    let store = EventStore::new(events);

    let filter = Filter::parse(r#"transport == "ZMQ""#).unwrap();
    let indices = store.filter_indices(&filter);
    assert_eq!(indices.len(), 0);
}

#[test]
fn test_event_store_filter_indices_all_match() {
    let events = vec![
        make_event(1, 1_000_000_000, TransportKind::Grpc),
        make_event(2, 2_000_000_000, TransportKind::Grpc),
    ];
    let store = EventStore::new(events);

    let filter = Filter::parse(r#"transport == "gRPC""#).unwrap();
    let indices = store.filter_indices(&filter);
    assert_eq!(indices.len(), 2);
}

#[test]
fn test_event_store_time_range_empty() {
    let store = EventStore::new(vec![]);
    let range = store.time_range();
    assert!(range.is_none());
}

#[test]
fn test_event_store_time_range_single_event() {
    let events = vec![make_event(1, 5_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);

    let range = store.time_range();
    assert!(range.is_some());
    let (start, end) = range.unwrap();
    assert_eq!(start.as_nanos(), 5_000_000_000);
    assert_eq!(end.as_nanos(), 5_000_000_000);
}

#[test]
fn test_event_store_time_range_multiple_events() {
    let events = vec![
        make_event(1, 1_000_000_000, TransportKind::Grpc),
        make_event(2, 5_000_000_000, TransportKind::Zmq),
        make_event(3, 3_000_000_000, TransportKind::DdsRtps),
    ];
    let store = EventStore::new(events);

    let range = store.time_range();
    assert!(range.is_some());
    let (start, end) = range.unwrap();
    // Should be sorted, so first is earliest, last is latest
    assert_eq!(start.as_nanos(), 1_000_000_000);
    assert_eq!(end.as_nanos(), 5_000_000_000);
}

#[test]
fn test_event_store_protocol_counts() {
    let events = vec![
        make_event(1, 1_000_000_000, TransportKind::Grpc),
        make_event(2, 2_000_000_000, TransportKind::Grpc),
        make_event(3, 3_000_000_000, TransportKind::Zmq),
        make_event(4, 4_000_000_000, TransportKind::DdsRtps),
        make_event(5, 5_000_000_000, TransportKind::Grpc),
    ];
    let store = EventStore::new(events);

    let counts = store.protocol_counts(&store.all_indices());

    // Should have 3 gRPC, 1 ZMQ, 1 DDS
    assert_eq!(
        counts
            .iter()
            .find(|(k, _)| *k == TransportKind::Grpc)
            .map(|(_, v)| *v),
        Some(3)
    );
    assert_eq!(
        counts
            .iter()
            .find(|(k, _)| *k == TransportKind::Zmq)
            .map(|(_, v)| *v),
        Some(1)
    );
    assert_eq!(
        counts
            .iter()
            .find(|(k, _)| *k == TransportKind::DdsRtps)
            .map(|(_, v)| *v),
        Some(1)
    );
}

#[test]
fn test_event_store_protocol_counts_filtered() {
    let events = vec![
        make_event(1, 1_000_000_000, TransportKind::Grpc),
        make_event(2, 2_000_000_000, TransportKind::Zmq),
        make_event(3, 3_000_000_000, TransportKind::Grpc),
    ];
    let store = EventStore::new(events);

    let filter = Filter::parse(r#"transport == "gRPC""#).unwrap();
    let filtered_indices = store.filter_indices(&filter);
    let counts = store.protocol_counts(&filtered_indices);

    // Should only have gRPC counts
    assert_eq!(
        counts
            .iter()
            .find(|(k, _)| *k == TransportKind::Grpc)
            .map(|(_, v)| *v),
        Some(2)
    );
    assert_eq!(counts.iter().find(|(k, _)| *k == TransportKind::Zmq), None);
}

#[test]
fn test_event_store_time_buckets_empty() {
    let store = EventStore::new(vec![]);
    let buckets = store.time_buckets(&[], 10);
    // With empty data, still returns buckets filled with zeros
    assert_eq!(buckets.len(), 10);
    assert_eq!(buckets.iter().sum::<u64>(), 0);
}

#[test]
fn test_event_store_time_buckets_single_bucket() {
    let events = vec![
        make_event(1, 1_000_000_000, TransportKind::Grpc),
        make_event(2, 2_000_000_000, TransportKind::Zmq),
    ];
    let store = EventStore::new(events);
    let indices = store.all_indices();

    let buckets = store.time_buckets(&indices, 1);
    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0], 2); // Both events in one bucket
}

#[test]
fn test_event_store_time_buckets_multiple() {
    let events = vec![
        make_event(1, 0, TransportKind::Grpc),
        make_event(2, 5_000_000_000, TransportKind::Zmq),
        make_event(3, 10_000_000_000, TransportKind::DdsRtps),
    ];
    let store = EventStore::new(events);
    let indices = store.all_indices();

    let buckets = store.time_buckets(&indices, 10);
    assert_eq!(buckets.len(), 10);
    // Events should be distributed across buckets
    let total: u64 = buckets.iter().sum();
    assert_eq!(total, 3);
}

#[test]
fn test_event_store_time_buckets_zero() {
    let events = vec![make_event(1, 1_000_000_000, TransportKind::Grpc)];
    let store = EventStore::new(events);
    let indices = store.all_indices();

    let buckets = store.time_buckets(&indices, 0);
    assert_eq!(buckets.len(), 0);
}

#[test]
fn test_event_store_sorting() {
    // Events inserted out of order
    let events = vec![
        make_event(3, 3_000_000_000, TransportKind::Grpc),
        make_event(1, 1_000_000_000, TransportKind::Zmq),
        make_event(2, 2_000_000_000, TransportKind::DdsRtps),
    ];
    let store = EventStore::new(events);

    // Should be sorted by timestamp
    assert_eq!(store.get(0).unwrap().id.as_u64(), 1);
    assert_eq!(store.get(1).unwrap().id.as_u64(), 2);
    assert_eq!(store.get(2).unwrap().id.as_u64(), 3);
}
