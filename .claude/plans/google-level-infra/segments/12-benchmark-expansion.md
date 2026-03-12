---
segment: 12
title: Benchmark Expansion
depends_on: []
risk: 3
complexity: Medium
cycle_budget: 5
estimated_lines: ~600 new lines
---

# Segment 12: Benchmark Expansion

## Context

Expand the benchmark suite to track performance regressions across critical code paths. Current benchmarks exist for detection and pipeline throughput, but many performance-critical areas lack coverage.

## Current State

Existing benchmarks:
- `prb-detect/benches/detection.rs` - Protocol detection performance
- `prb-pcap/benches/pipeline_throughput.rs` - Pipeline throughput
- `prb-tui/benches/large_file_perf.rs` - TUI performance with large files

## Goal

Add comprehensive benchmarks for all performance-critical components with CI tracking for regression detection.

## Exit Criteria

1. [ ] Benchmarks added for prb-grpc, prb-zmq, prb-dds decoders
2. [ ] Benchmarks added for prb-core conversation engine
3. [ ] Benchmarks added for prb-storage read/write
4. [ ] Benchmarks added for prb-query evaluation
5. [ ] All benchmarks run successfully with `cargo bench`
6. [ ] CI benchmark job configured (already in S03)
7. [ ] Benchmark results tracked over time
8. [ ] Manual test: Run benchmarks, verify performance is reasonable

## Implementation Plan

### Benchmark 1: gRPC Decoder

Create `/Users/psauer/probe/crates/prb-grpc/benches/grpc_decode.rs`:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use prb_core::{DebugEvent, EventSource};
use prb_grpc::GrpcDecoder;

fn grpc_message_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("grpc_decode");

    // Test data: typical gRPC request
    let grpc_request = create_test_grpc_request();

    group.throughput(Throughput::Bytes(grpc_request.len() as u64));
    group.bench_function("decode_request", |b| {
        b.iter(|| {
            let decoder = GrpcDecoder::new();
            decoder.decode(black_box(&grpc_request))
        });
    });

    // Test data: gRPC response
    let grpc_response = create_test_grpc_response();

    group.throughput(Throughput::Bytes(grpc_response.len() as u64));
    group.bench_function("decode_response", |b| {
        b.iter(|| {
            let decoder = GrpcDecoder::new();
            decoder.decode(black_box(&grpc_response))
        });
    });

    group.finish();
}

fn create_test_grpc_request() -> Vec<u8> {
    // Create realistic gRPC request bytes
    vec![/* ... */]
}

fn create_test_grpc_response() -> Vec<u8> {
    // Create realistic gRPC response bytes
    vec![/* ... */]
}

criterion_group!(benches, grpc_message_decode);
criterion_main!(benches);
```

Add to `prb-grpc/Cargo.toml`:
```toml
[[bench]]
name = "grpc_decode"
harness = false
```

### Benchmark 2: ZMTP Decoder

Create `/Users/psauer/probe/crates/prb-zmq/benches/zmtp_decode.rs`:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use prb_zmq::ZmqDecoder;

fn zmtp_frame_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("zmtp_decode");

    let zmtp_frame = create_test_zmtp_frame();

    group.throughput(Throughput::Bytes(zmtp_frame.len() as u64));
    group.bench_function("decode_frame", |b| {
        b.iter(|| {
            let decoder = ZmqDecoder::new();
            decoder.decode(black_box(&zmtp_frame))
        });
    });

    group.finish();
}

fn create_test_zmtp_frame() -> Vec<u8> {
    vec![/* ... */]
}

criterion_group!(benches, zmtp_frame_decode);
criterion_main!(benches);
```

Add to `prb-zmq/Cargo.toml`:
```toml
[[bench]]
name = "zmtp_decode"
harness = false
```

### Benchmark 3: DDS/RTPS Decoder

Create `/Users/psauer/probe/crates/prb-dds/benches/rtps_decode.rs`:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use prb_dds::DdsDecoder;

fn rtps_packet_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("rtps_decode");

    let rtps_packet = create_test_rtps_packet();

    group.throughput(Throughput::Bytes(rtps_packet.len() as u64));
    group.bench_function("decode_packet", |b| {
        b.iter(|| {
            let decoder = DdsDecoder::new();
            decoder.decode(black_box(&rtps_packet))
        });
    });

    group.finish();
}

fn create_test_rtps_packet() -> Vec<u8> {
    vec![/* ... */]
}

criterion_group!(benches, rtps_packet_decode);
criterion_main!(benches);
```

Add to `prb-dds/Cargo.toml`:
```toml
[[bench]]
name = "rtps_decode"
harness = false
```

### Benchmark 4: Conversation Engine

Create `/Users/psauer/probe/crates/prb-core/benches/conversation_engine.rs`:

```rust
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use prb_core::{ConversationEngine, DebugEvent};

fn conversation_grouping(c: &mut Criterion) {
    let mut group = c.benchmark_group("conversation_engine");

    group.bench_function("group_100_events", |b| {
        b.iter_batched(
            || create_test_events(100),
            |events| {
                let mut engine = ConversationEngine::new();
                for event in events {
                    engine.process_event(black_box(event));
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("group_1000_events", |b| {
        b.iter_batched(
            || create_test_events(1000),
            |events| {
                let mut engine = ConversationEngine::new();
                for event in events {
                    engine.process_event(black_box(event));
                }
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

fn create_test_events(count: usize) -> Vec<DebugEvent> {
    (0..count).map(|i| create_test_event(i)).collect()
}

fn create_test_event(id: usize) -> DebugEvent {
    // Create realistic test event
    todo!()
}

criterion_group!(benches, conversation_grouping);
criterion_main!(benches);
```

Add to `prb-core/Cargo.toml`:
```toml
[[bench]]
name = "conversation_engine"
harness = false

[dev-dependencies]
criterion = { workspace = true }
```

### Benchmark 5: Storage Performance

Create `/Users/psauer/probe/crates/prb-storage/benches/storage_perf.rs`:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use prb_storage::{SessionReader, SessionWriter};
use tempfile::TempDir;

fn storage_write_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage");

    let events = create_test_events(1000);

    group.throughput(Throughput::Elements(1000));
    group.bench_function("write_1000_events", |b| {
        b.iter_with_setup(
            || TempDir::new().unwrap(),
            |tmp_dir| {
                let path = tmp_dir.path().join("test.mcap");
                let mut writer = SessionWriter::new(&path).unwrap();
                for event in &events {
                    writer.write_event(black_box(event)).unwrap();
                }
            },
        );
    });

    group.bench_function("read_1000_events", |b| {
        let tmp_dir = TempDir::new().unwrap();
        let path = tmp_dir.path().join("test.mcap");
        // Pre-create file with events
        let mut writer = SessionWriter::new(&path).unwrap();
        for event in &events {
            writer.write_event(event).unwrap();
        }
        drop(writer);

        b.iter(|| {
            let reader = SessionReader::new(black_box(&path)).unwrap();
            let mut count = 0;
            for event in reader {
                black_box(event);
                count += 1;
            }
            assert_eq!(count, 1000);
        });
    });

    group.finish();
}

criterion_group!(benches, storage_write_read);
criterion_main!(benches);
```

Add to `prb-storage/Cargo.toml`:
```toml
[[bench]]
name = "storage_perf"
harness = false
```

### Benchmark 6: Query Evaluation

Create `/Users/psauer/probe/crates/prb-query/benches/query_eval.rs`:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use prb_query::{Filter, parse_filter};
use prb_core::DebugEvent;

fn query_evaluation(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_eval");

    let events = create_test_events(100);
    let simple_filter = parse_filter("transport == \"gRPC\"").unwrap();
    let complex_filter = parse_filter(
        "transport == \"gRPC\" && grpc.method contains \"User\" && direction == \"request\""
    ).unwrap();

    group.throughput(Throughput::Elements(100));
    group.bench_function("simple_filter", |b| {
        b.iter(|| {
            events.iter().filter(|e| simple_filter.matches(black_box(e))).count()
        });
    });

    group.bench_function("complex_filter", |b| {
        b.iter(|| {
            events.iter().filter(|e| complex_filter.matches(black_box(e))).count()
        });
    });

    group.finish();
}

criterion_group!(benches, query_evaluation);
criterion_main!(benches);
```

Add to `prb-query/Cargo.toml`:
```toml
[[bench]]
name = "query_eval"
harness = false
```

## Files to Create

1. `prb-grpc/benches/grpc_decode.rs` (~100 lines)
2. `prb-zmq/benches/zmtp_decode.rs` (~80 lines)
3. `prb-dds/benches/rtps_decode.rs` (~80 lines)
4. `prb-core/benches/conversation_engine.rs` (~100 lines)
5. `prb-storage/benches/storage_perf.rs` (~120 lines)
6. `prb-query/benches/query_eval.rs` (~80 lines)
7. Updates to 6 Cargo.toml files (~10 lines each)

Total: ~600 new lines

## Test Plan

1. Create all benchmark files
2. Update Cargo.toml files with [[bench]] entries
3. Run each benchmark individually:
   ```bash
   cargo bench -p prb-grpc
   cargo bench -p prb-zmq
   cargo bench -p prb-dds
   cargo bench -p prb-core
   cargo bench -p prb-storage
   cargo bench -p prb-query
   ```
4. Run all benchmarks:
   ```bash
   cargo bench --workspace
   ```
5. Verify benchmark output shows performance metrics
6. Check that CI benchmark job runs (push to main branch)
7. Commit: "perf: Add comprehensive benchmark suite"

## Blocked By

None - benchmarks are independent additions.

## Blocks

None - performance tracking doesn't block functionality.

## Success Metrics

- 6 new benchmark files created
- All benchmarks compile and run
- Reasonable performance numbers (document baseline)
- CI tracks benchmarks over time
- No performance regressions detected

## Notes

- Benchmarks use criterion for statistical rigor
- Test data should be realistic (based on real protocol samples)
- Consider adding micro-benchmarks for hot paths
- Benchmark results are stored in target/criterion/
- CI benchmark job only runs on main branch (doesn't block PRs)
- Use `cargo bench -- --save-baseline <name>` to track baselines
