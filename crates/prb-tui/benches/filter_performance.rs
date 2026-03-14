//! Comprehensive performance benchmarks for filter operations.
//!
//! This benchmark suite validates that filter operations meet performance
//! targets for 100k+ events with no regressions.
//!
//! Performance targets (from research):
//! - Full filtering (100K events): <10ms
//! - Incremental filtering (1K batch): <5ms
//! - Parser (complex filter): <100µs
//! - Autocomplete update: <1ms
//! - Query planner: Indexed queries 10-100x faster than full scan

use bytes::Bytes;
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use prb_core::{
    DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind,
};
use prb_query::Filter;
use prb_tui::{EventStore, autocomplete::AutocompleteState};
use std::collections::BTreeMap;

/// Generate realistic test events with varied protocols and ports.
fn generate_test_events(count: usize) -> Vec<DebugEvent> {
    (0..count)
        .map(|i| {
            // Protocol distribution: 30% TCP, 30% UDP, 20% gRPC, 20% ZMQ
            let transport = match i % 10 {
                0..=2 => TransportKind::RawTcp,
                3..=5 => TransportKind::RawUdp,
                6..=7 => TransportKind::Grpc,
                _ => TransportKind::Zmq,
            };

            // Port distribution: 30% HTTPS (443), 20% DNS (53), 10% HTTP (80), 40% varied
            let src_port = match i % 10 {
                0..=2 => 443,                  // 30% HTTPS
                3..=4 => 53,                   // 20% DNS
                5 => 80,                       // 10% HTTP
                _ => 8000 + (i % 1000) as u16, // 40% varied
            };

            let dst_port = match i % 10 {
                0..=4 => 443,                  // 50% to HTTPS
                5..=7 => 53,                   // 30% to DNS
                _ => 9000 + (i % 1000) as u16, // 20% varied
            };

            DebugEvent {
                id: EventId::from_raw(i as u64),
                timestamp: Timestamp::from_nanos((1000 * i) as u64),
                source: EventSource {
                    adapter: "bench".into(),
                    origin: "test".into(),
                    network: Some(NetworkAddr {
                        src: format!("192.168.{}.{}:{}", (i / 256) % 256, i % 256, src_port),
                        dst: format!("10.0.{}.{}:{}", (i / 256) % 256, i % 256, dst_port),
                    }),
                },
                transport,
                direction: if i % 2 == 0 {
                    Direction::Inbound
                } else {
                    Direction::Outbound
                },
                payload: Payload::Raw {
                    raw: Bytes::from(vec![0u8; 100 + (i % 900)]),
                },
                metadata: BTreeMap::new(),
                correlation_keys: vec![],
                sequence: Some(i as u64),
                warnings: vec![],
            }
        })
        .collect()
}

/// Benchmark full scan filtering with different filter complexities.
fn bench_full_scan_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_scan_filter");

    for size in [1_000, 10_000, 100_000].iter() {
        let events = generate_test_events(*size);
        let store = EventStore::new(events);

        // Simple transport filter
        let filter_simple = Filter::parse(r#"transport == "TCP""#).unwrap();
        group.bench_with_input(BenchmarkId::new("simple_transport", size), size, |b, _| {
            b.iter(|| {
                let _filtered = store.filter_indices(black_box(&filter_simple));
            });
        });

        // Complex AND filter
        let filter_and = Filter::parse(r#"transport == "TCP" && direction == "inbound""#).unwrap();
        group.bench_with_input(BenchmarkId::new("complex_and", size), size, |b, _| {
            b.iter(|| {
                let _filtered = store.filter_indices(black_box(&filter_and));
            });
        });

        // Complex OR filter
        let filter_or = Filter::parse(r#"transport == "TCP" || transport == "UDP""#).unwrap();
        group.bench_with_input(BenchmarkId::new("complex_or", size), size, |b, _| {
            b.iter(|| {
                let _filtered = store.filter_indices(black_box(&filter_or));
            });
        });
    }

    group.finish();
}

/// Benchmark indexed filtering using query planner.
fn bench_indexed_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("indexed_filter");

    for size in [1_000, 10_000, 100_000].iter() {
        let events = generate_test_events(*size);
        let mut store = EventStore::new(events);
        // Build index for indexed queries
        store.build_index();

        // Simple transport filter should use index
        group.bench_with_input(BenchmarkId::new("indexed_transport", size), size, |b, _| {
            b.iter(|| {
                let _filtered = store
                    .apply_filter_with_plan(black_box(r#"transport == "TCP""#))
                    .unwrap();
            });
        });

        // AND with transport should use index for first predicate
        group.bench_with_input(
            BenchmarkId::new("indexed_and_transport", size),
            size,
            |b, _| {
                b.iter(|| {
                    let _filtered = store
                        .apply_filter_with_plan(black_box(
                            r#"transport == "TCP" && direction == "inbound""#,
                        ))
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark incremental filtering for streaming use cases.
fn bench_incremental_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_filter");

    let filter = Filter::parse(r#"transport == "TCP""#).unwrap();

    group.bench_function("batch_1000", |b| {
        b.iter(|| {
            let mut store = EventStore::empty();
            let batch = generate_test_events(1000);
            store.push_batch(batch);
            let _filtered = store.filter_indices_incremental(black_box(&filter));
        });
    });

    group.finish();
}

/// Benchmark parser performance on various filter expressions.
fn bench_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser");

    let filters = vec![
        ("simple", r#"transport == "TCP""#),
        ("and", r#"transport == "TCP" && direction == "inbound""#),
        ("or", r#"transport == "TCP" || transport == "UDP""#),
        (
            "complex",
            r#"(transport == "TCP" || transport == "UDP") && direction == "inbound""#,
        ),
        ("field_access", r#"sequence > 1000"#),
    ];

    for (name, filter_str) in filters {
        group.bench_with_input(BenchmarkId::new("parse", name), &filter_str, |b, f| {
            b.iter(|| {
                let _filter = Filter::parse(black_box(f));
            });
        });
    }

    group.finish();
}

/// Benchmark autocomplete fuzzy matching.
fn bench_autocomplete(c: &mut Criterion) {
    let mut group = c.benchmark_group("autocomplete");

    let mut autocomplete = AutocompleteState::new();

    group.bench_function("fuzzy_match_short", |b| {
        b.iter(|| {
            autocomplete.update(black_box("tcp"), 3);
        });
    });

    group.bench_function("fuzzy_match_medium", |b| {
        b.iter(|| {
            autocomplete.update(black_box("trans"), 5);
        });
    });

    group.bench_function("fuzzy_match_long", |b| {
        b.iter(|| {
            autocomplete.update(black_box("direction"), 9);
        });
    });

    group.finish();
}

/// Benchmark query planner effectiveness by comparing indexed vs full scan.
fn bench_query_planner_speedup(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_planner_speedup");

    let events = generate_test_events(100_000);

    // Full scan benchmark
    let store_full = EventStore::new(events.clone());
    let filter = Filter::parse(r#"transport == "TCP""#).unwrap();

    group.bench_function("full_scan_100k", |b| {
        b.iter(|| {
            let _filtered = store_full.filter_indices(black_box(&filter));
        });
    });

    // Indexed benchmark
    let mut store_indexed = EventStore::new(events);
    store_indexed.build_index();

    group.bench_function("indexed_100k", |b| {
        b.iter(|| {
            let _filtered = store_indexed
                .apply_filter_with_plan(black_box(r#"transport == "TCP""#))
                .unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_full_scan_filter,
    bench_indexed_filter,
    bench_incremental_filter,
    bench_parser,
    bench_autocomplete,
    bench_query_planner_speedup
);
criterion_main!(benches);
