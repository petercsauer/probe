//! Benchmarks for query filter evaluation performance.

use bytes::Bytes;
use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use prb_core::{
    DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind,
};
use prb_query::Filter;
use std::collections::BTreeMap;

fn create_test_event(id: u64, transport: TransportKind, method: &str) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(1_000_000_000 + id * 1000),
        source: EventSource {
            adapter: "pcap".to_string(),
            origin: "test.pcap".to_string(),
            network: Some(NetworkAddr {
                src: "192.168.1.1:5000".to_string(),
                dst: "192.168.1.2:6000".to_string(),
            }),
        },
        transport,
        direction: if id.is_multiple_of(2) {
            Direction::Outbound
        } else {
            Direction::Inbound
        },
        payload: Payload::Raw {
            raw: Bytes::from(vec![0u8; 100]),
        },
        metadata: {
            let mut m = BTreeMap::new();
            m.insert("grpc.method".to_string(), method.to_string());
            m
        },
        correlation_keys: vec![],
        sequence: Some(id),
        warnings: vec![],
    }
}

fn create_test_events(count: usize) -> Vec<DebugEvent> {
    let mut events = Vec::new();
    let methods = [
        "/api.UserService/GetUser",
        "/api.UserService/ListUsers",
        "/api.OrderService/CreateOrder",
        "/api.OrderService/GetOrder",
    ];

    for i in 0..count {
        let transport = if i % 3 == 0 {
            TransportKind::Grpc
        } else if i % 3 == 1 {
            TransportKind::Zmq
        } else {
            TransportKind::DdsRtps
        };
        let method = methods[i % methods.len()];
        events.push(create_test_event(i as u64, transport, method));
    }

    events
}

fn bench_query_evaluation(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_eval");

    let events = create_test_events(1000);

    // Simple filter: single equality check
    let simple_filter = Filter::parse(r#"transport == "gRPC""#).unwrap();

    group.throughput(Throughput::Elements(1000));
    group.bench_function("simple_filter", |b| {
        b.iter(|| {
            black_box(&events)
                .iter()
                .filter(|e| simple_filter.matches(e))
                .count()
        });
    });

    // Complex filter: multiple conditions with string operations
    let complex_filter = Filter::parse(
        r#"transport == "gRPC" && grpc.method contains "User" && direction == "request""#,
    )
    .unwrap();

    group.bench_function("complex_filter", |b| {
        b.iter(|| {
            black_box(&events)
                .iter()
                .filter(|e| complex_filter.matches(e))
                .count()
        });
    });

    // Very complex filter: nested conditions
    let very_complex_filter = Filter::parse(
        r#"(transport == "gRPC" && grpc.method contains "User") || (transport == "ZMQ" && direction == "request")"#,
    )
    .unwrap();

    group.bench_function("very_complex_filter", |b| {
        b.iter(|| {
            black_box(&events)
                .iter()
                .filter(|e| very_complex_filter.matches(e))
                .count()
        });
    });

    group.finish();
}

criterion_group!(benches, bench_query_evaluation);
criterion_main!(benches);
