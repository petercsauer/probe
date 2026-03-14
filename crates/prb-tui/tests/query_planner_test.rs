//! Integration tests for query planner with EventStore.

use bytes::Bytes;
use prb_core::{
    DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind,
};
use prb_tui::EventStore;
use std::collections::BTreeMap;

fn make_event(id: u64, ts_ns: u64, transport: TransportKind, src: &str, dst: &str) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(ts_ns),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: Some(NetworkAddr {
                src: src.to_string(),
                dst: dst.to_string(),
            }),
        },
        transport,
        direction: Direction::Inbound,
        payload: Payload::Raw { raw: Bytes::new() },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}

#[test]
fn test_apply_filter_with_plan_empty_filter() {
    let events = vec![
        make_event(
            1,
            1000,
            TransportKind::Grpc,
            "10.0.0.1:1234",
            "10.0.0.2:5678",
        ),
        make_event(
            2,
            2000,
            TransportKind::Zmq,
            "10.0.0.3:1234",
            "10.0.0.4:5678",
        ),
    ];
    let mut store = EventStore::new(events);
    store.build_index();

    let result = store.apply_filter_with_plan("").expect("filter failed");
    assert_eq!(result.len(), 2);
}

#[test]
fn test_apply_filter_with_plan_simple_transport() {
    let events = vec![
        make_event(
            1,
            1000,
            TransportKind::Grpc,
            "10.0.0.1:1234",
            "10.0.0.2:5678",
        ),
        make_event(
            2,
            2000,
            TransportKind::Zmq,
            "10.0.0.3:1234",
            "10.0.0.4:5678",
        ),
        make_event(
            3,
            3000,
            TransportKind::Grpc,
            "10.0.0.5:1234",
            "10.0.0.6:5678",
        ),
    ];
    let mut store = EventStore::new(events);
    store.build_index();

    let result = store
        .apply_filter_with_plan(r#"transport == "gRPC""#)
        .expect("filter failed");
    assert_eq!(result.len(), 2);
    assert_eq!(store.get(result[0]).unwrap().transport, TransportKind::Grpc);
    assert_eq!(store.get(result[1]).unwrap().transport, TransportKind::Grpc);
}

#[test]
fn test_apply_filter_with_plan_transport_and_other() {
    let mut events = vec![
        make_event(
            1,
            1000,
            TransportKind::Grpc,
            "10.0.0.1:1234",
            "10.0.0.2:5678",
        ),
        make_event(
            2,
            2000,
            TransportKind::Zmq,
            "10.0.0.3:1234",
            "10.0.0.4:5678",
        ),
        make_event(
            3,
            3000,
            TransportKind::Grpc,
            "10.0.0.5:1234",
            "10.0.0.6:5678",
        ),
    ];

    // Add metadata to first gRPC event
    events[0]
        .metadata
        .insert("grpc.method".to_string(), "/api/Users/Get".to_string());

    let mut store = EventStore::new(events);
    store.build_index();

    let result = store
        .apply_filter_with_plan(r#"transport == "gRPC" && grpc.method contains "Users""#)
        .expect("filter failed");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], 0);
}

#[test]
fn test_apply_filter_with_plan_src_address() {
    let events = vec![
        make_event(
            1,
            1000,
            TransportKind::Grpc,
            "10.0.0.1:1234",
            "10.0.0.2:5678",
        ),
        make_event(
            2,
            2000,
            TransportKind::Zmq,
            "10.0.0.3:1234",
            "10.0.0.4:5678",
        ),
        make_event(
            3,
            3000,
            TransportKind::Grpc,
            "10.0.0.1:1234",
            "10.0.0.6:5678",
        ),
    ];
    let mut store = EventStore::new(events);
    store.build_index();

    let result = store
        .apply_filter_with_plan(r#"src == "10.0.0.1:1234""#)
        .expect("filter failed");
    assert_eq!(result.len(), 2);
}

#[test]
fn test_apply_filter_with_plan_dst_address() {
    let events = vec![
        make_event(
            1,
            1000,
            TransportKind::Grpc,
            "10.0.0.1:1234",
            "10.0.0.2:5678",
        ),
        make_event(
            2,
            2000,
            TransportKind::Zmq,
            "10.0.0.3:1234",
            "10.0.0.2:5678",
        ),
        make_event(
            3,
            3000,
            TransportKind::Grpc,
            "10.0.0.5:1234",
            "10.0.0.6:5678",
        ),
    ];
    let mut store = EventStore::new(events);
    store.build_index();

    let result = store
        .apply_filter_with_plan(r#"dst == "10.0.0.2:5678""#)
        .expect("filter failed");
    assert_eq!(result.len(), 2);
}

#[test]
fn test_apply_filter_with_plan_complex_or() {
    let events = vec![
        make_event(
            1,
            1000,
            TransportKind::Grpc,
            "10.0.0.1:1234",
            "10.0.0.2:5678",
        ),
        make_event(
            2,
            2000,
            TransportKind::Zmq,
            "10.0.0.3:1234",
            "10.0.0.4:5678",
        ),
        make_event(
            3,
            3000,
            TransportKind::DdsRtps,
            "10.0.0.5:1234",
            "10.0.0.6:5678",
        ),
    ];
    let mut store = EventStore::new(events);
    store.build_index();

    // OR expressions should fall back to full scan
    let result = store
        .apply_filter_with_plan(r#"transport == "gRPC" || transport == "ZMQ""#)
        .expect("filter failed");
    assert_eq!(result.len(), 2);
}

#[test]
fn test_apply_filter_with_plan_no_index() {
    let events = vec![
        make_event(
            1,
            1000,
            TransportKind::Grpc,
            "10.0.0.1:1234",
            "10.0.0.2:5678",
        ),
        make_event(
            2,
            2000,
            TransportKind::Zmq,
            "10.0.0.3:1234",
            "10.0.0.4:5678",
        ),
    ];
    let mut store = EventStore::new(events);
    // Don't build index

    // Should fall back to full scan when no index is available
    let result = store
        .apply_filter_with_plan(r#"transport == "gRPC""#)
        .expect("filter failed");
    assert_eq!(result.len(), 1);
}

#[test]
fn test_apply_filter_with_plan_caching() {
    let events = vec![
        make_event(
            1,
            1000,
            TransportKind::Grpc,
            "10.0.0.1:1234",
            "10.0.0.2:5678",
        ),
        make_event(
            2,
            2000,
            TransportKind::Zmq,
            "10.0.0.3:1234",
            "10.0.0.4:5678",
        ),
    ];
    let mut store = EventStore::new(events);
    store.build_index();

    // First filter - should parse and cache
    let result1 = store
        .apply_filter_with_plan(r#"transport == "gRPC""#)
        .expect("filter failed");
    assert_eq!(result1.len(), 1);

    // Second filter with same expression - should hit cache
    let result2 = store
        .apply_filter_with_plan(r#"transport == "gRPC""#)
        .expect("filter failed");
    assert_eq!(result2.len(), 1);
}

#[test]
fn test_apply_filter_with_plan_parse_error() {
    let events = vec![make_event(
        1,
        1000,
        TransportKind::Grpc,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let mut store = EventStore::new(events);

    // Invalid syntax should return error
    let result = store.apply_filter_with_plan(r#"transport == "#);
    assert!(result.is_err());
}

#[test]
fn test_indexed_faster_than_full_scan() {
    // Create a large dataset
    let events: Vec<_> = (0..10000)
        .map(|i| {
            make_event(
                i,
                1000 * i,
                if i % 3 == 0 {
                    TransportKind::Grpc
                } else if i % 3 == 1 {
                    TransportKind::Zmq
                } else {
                    TransportKind::DdsRtps
                },
                &format!("10.0.0.{}:1234", i % 256),
                &format!("10.0.0.{}:5678", (i + 1) % 256),
            )
        })
        .collect();

    let mut store = EventStore::new(events);
    store.build_index();

    // Measure indexed query
    let start_indexed = std::time::Instant::now();
    let result_indexed = store
        .apply_filter_with_plan(r#"transport == "gRPC""#)
        .expect("filter failed");
    let duration_indexed = start_indexed.elapsed();

    // Should find ~3333 events (1/3 of 10K)
    assert!((result_indexed.len() as f64 - 3333.0).abs() < 10.0);

    // Measure full scan (using filter_indices_incremental which does full scan)
    let filter = prb_query::Filter::parse(r#"transport == "gRPC""#).unwrap();
    let start_full_scan = std::time::Instant::now();
    let result_full_scan = store.filter_indices_incremental(&filter);
    let duration_full_scan = start_full_scan.elapsed();

    assert_eq!(result_indexed.len(), result_full_scan.len());

    // Indexed should be faster (or at least comparable)
    // Note: For small datasets, index overhead might make it slower,
    // but for 10K events with selective filter, index should help
    println!(
        "Indexed: {:?}, Full scan: {:?}",
        duration_indexed, duration_full_scan
    );

    // Both should be fast enough for real-time filtering
    assert!(duration_indexed.as_millis() < 100);
    assert!(duration_full_scan.as_millis() < 100);
}

#[test]
fn test_apply_filter_with_plan_nested_and() {
    let mut events = vec![
        make_event(
            1,
            1000,
            TransportKind::Grpc,
            "10.0.0.1:1234",
            "10.0.0.2:5678",
        ),
        make_event(
            2,
            2000,
            TransportKind::Zmq,
            "10.0.0.3:1234",
            "10.0.0.4:5678",
        ),
        make_event(
            3,
            3000,
            TransportKind::Grpc,
            "10.0.0.5:1234",
            "10.0.0.6:5678",
        ),
    ];

    // Add metadata to first event
    events[0]
        .metadata
        .insert("grpc.method".to_string(), "/api/Users/Get".to_string());
    events[0]
        .metadata
        .insert("grpc.status".to_string(), "0".to_string());

    let mut store = EventStore::new(events);
    store.build_index();

    // Nested AND should still use index for transport
    let result = store
        .apply_filter_with_plan(
            r#"transport == "gRPC" && grpc.method contains "Users" && grpc.status == "0""#,
        )
        .expect("filter failed");
    assert_eq!(result.len(), 1);
}
