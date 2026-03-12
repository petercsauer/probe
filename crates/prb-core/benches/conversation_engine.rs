//! Benchmarks for conversation reconstruction engine.

use bytes::Bytes;
use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use prb_core::{
    ConversationEngine, CorrelationKey, DebugEvent, Direction, EventId, EventSource, NetworkAddr,
    Payload, Timestamp, TransportKind,
};
use std::collections::BTreeMap;

fn create_test_event(
    id: u64,
    transport: TransportKind,
    correlation_key: CorrelationKey,
) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(1_000_000_000 + id * 1000),
        source: EventSource {
            adapter: "test".to_string(),
            origin: "benchmark".to_string(),
            network: Some(NetworkAddr {
                src: format!("192.168.1.1:{}", 5000 + (id % 100)),
                dst: format!("192.168.1.2:{}", 6000 + (id % 100)),
            }),
        },
        transport,
        direction: if id % 2 == 0 {
            Direction::Outbound
        } else {
            Direction::Inbound
        },
        payload: Payload::Raw {
            raw: Bytes::from(vec![0u8; 100]),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![correlation_key],
        sequence: Some(id),
        warnings: vec![],
    }
}

fn create_test_events(count: usize) -> Vec<DebugEvent> {
    let mut events = Vec::new();

    // Create events with various correlation patterns
    for i in 0..count {
        let transport = match i % 3 {
            0 => TransportKind::Grpc,
            1 => TransportKind::Zmq,
            _ => TransportKind::DdsRtps,
        };

        let correlation_key = match i % 4 {
            0 => CorrelationKey::StreamId {
                id: (i / 10) as u32,
            },
            1 => CorrelationKey::Topic {
                name: format!("topic-{}", i / 20),
            },
            2 => CorrelationKey::TraceContext {
                trace_id: format!("trace-{}", i / 30),
                span_id: format!("span-{}", i),
            },
            _ => CorrelationKey::Custom {
                key: "connection".to_string(),
                value: format!("conn-{}", i / 15),
            },
        };

        events.push(create_test_event(i as u64, transport, correlation_key));
    }

    events
}

fn bench_conversation_grouping(c: &mut Criterion) {
    let mut group = c.benchmark_group("conversation_engine");

    group.bench_function("group_100_events", |b| {
        b.iter_batched(
            || create_test_events(100),
            |events| {
                let engine = ConversationEngine::new();
                black_box(engine.build_conversations(&events))
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("group_1000_events", |b| {
        b.iter_batched(
            || create_test_events(1000),
            |events| {
                let engine = ConversationEngine::new();
                black_box(engine.build_conversations(&events))
            },
            BatchSize::LargeInput,
        );
    });

    group.bench_function("group_10000_events", |b| {
        b.iter_batched(
            || create_test_events(10000),
            |events| {
                let engine = ConversationEngine::new();
                black_box(engine.build_conversations(&events))
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

criterion_group!(benches, bench_conversation_grouping);
criterion_main!(benches);
