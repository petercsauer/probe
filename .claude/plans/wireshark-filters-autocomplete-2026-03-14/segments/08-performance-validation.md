---
segment: 8
title: "Performance Validation Suite"
depends_on: [3]
risk: 2/10
complexity: Low
cycle_budget: 10
status: pending
commit_message: "test(tui): Add performance validation suite for filter operations"
---

# Segment 8: Performance Validation Suite

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Create comprehensive performance validation suite to verify filter operations meet targets for 100k+ events with no regressions.

**Depends on:** Segment 3 (query planner must be implemented for indexed benchmarks)

## Context: Issue 8 - No Performance Benchmarks

**Core Problem:**
- Performance already exceeds targets (9ms for 100K events vs 10ms target)
- Incremental filtering: 87µs per 1000-event batch (58x faster than 5ms target)
- No automated benchmarks to prevent regressions
- No validation that query planner actually improves performance
- No stress tests for autocomplete fuzzy matching

**Current performance (from research):**
```
Full filtering: 9ms for 100K events (target: <10ms) ✓
Incremental: 87µs per 1000 events (target: <5ms) ✓
```

**Root Cause:**
No benchmark suite. Performance verified manually during research phase only.

**Proposed Fix:**
Add comprehensive criterion-based benchmark suite:

```rust
// New file: crates/prb-tui/benches/filter_performance.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use prb_tui::{EventStore, QueryPlanner, FilterState, AutocompleteState};
use prb_core::DebugEvent;
use prb_query::parse_filter;

fn generate_test_events(count: usize) -> Vec<DebugEvent> {
    // Generate realistic mix of protocols
    (0..count).map(|i| {
        let transport = match i % 4 {
            0 => "tcp",
            1 => "udp",
            2 => "grpc",
            _ => "zmq",
        };

        let port = match i % 10 {
            0..=2 => 443,  // 30% HTTPS
            3..=4 => 53,   // 20% DNS
            5 => 80,       // 10% HTTP
            _ => 8000 + (i % 1000) as u16,  // 40% varied
        };

        DebugEvent {
            frame_number: i,
            timestamp: std::time::SystemTime::now(),
            transport: Some(TransportInfo {
                kind: transport.parse().unwrap(),
                // ... fill in other fields
            }),
            network: Some(NetworkAddr {
                src: format!("192.168.1.{}:{}", i % 254 + 1, port),
                dst: format!("10.0.0.{}:{}", i % 254 + 1, 443),
            }),
            // ... fill in other fields
        }
    }).collect()
}

fn bench_full_scan_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_scan_filter");

    for size in [1_000, 10_000, 100_000].iter() {
        let events = generate_test_events(*size);
        let mut store = EventStore::new();
        for event in events {
            store.add_event(event);
        }

        group.bench_with_input(
            BenchmarkId::new("simple_transport", size),
            size,
            |b, _| {
                b.iter(|| {
                    store.apply_filter(black_box("transport == \"tcp\""))
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("complex_and", size),
            size,
            |b, _| {
                b.iter(|| {
                    store.apply_filter(black_box("transport == \"tcp\" && tcp.port == 443"))
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("complex_or", size),
            size,
            |b, _| {
                b.iter(|| {
                    store.apply_filter(black_box("tcp.port == 443 || udp.port == 53"))
                });
            },
        );
    }

    group.finish();
}

fn bench_indexed_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("indexed_filter");

    for size in [1_000, 10_000, 100_000].iter() {
        let events = generate_test_events(*size);
        let mut store = EventStore::new();
        for event in events {
            store.add_event(event);
        }

        // Simple transport filter should use index
        group.bench_with_input(
            BenchmarkId::new("indexed_transport", size),
            size,
            |b, _| {
                b.iter(|| {
                    store.apply_filter_with_plan(black_box("transport == \"tcp\""))
                });
            },
        );

        // AND with transport should use index for first predicate
        group.bench_with_input(
            BenchmarkId::new("indexed_and_transport", size),
            size,
            |b, _| {
                b.iter(|| {
                    store.apply_filter_with_plan(black_box("transport == \"tcp\" && tcp.port == 443"))
                });
            },
        );
    }

    group.finish();
}

fn bench_incremental_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_filter");

    let mut store = EventStore::new();
    let filter_expr = parse_filter("transport == \"tcp\"").unwrap();

    group.bench_function("batch_1000", |b| {
        b.iter(|| {
            let batch = generate_test_events(1000);
            for event in batch {
                store.add_event(event);
            }
            store.apply_filter_cached(black_box(&filter_expr))
        });
    });

    group.finish();
}

fn bench_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser");

    let filters = vec![
        "transport == \"tcp\"",
        "tcp.port == 443 && udp.port == 53",
        "tcp.port in {80, 443, 8080}",
        r#"tcp.payload matches "^GET""#,
        "len(tcp.payload) > 100",
    ];

    for filter in filters {
        group.bench_with_input(
            BenchmarkId::new("parse", filter),
            &filter,
            |b, f| {
                b.iter(|| {
                    parse_filter(black_box(f))
                });
            },
        );
    }

    group.finish();
}

fn bench_autocomplete(c: &mut Criterion) {
    let mut group = c.benchmark_group("autocomplete");

    let mut autocomplete = AutocompleteState::new();

    group.bench_function("fuzzy_match_short", |b| {
        b.iter(|| {
            autocomplete.update(black_box("tcp"), 3)
        });
    });

    group.bench_function("fuzzy_match_long", |b| {
        b.iter(|| {
            autocomplete.update(black_box("tcp.payload"), 11)
        });
    });

    group.finish();
}

fn bench_syntax_highlighting(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax_highlighting");

    let filters = vec![
        "transport == \"tcp\"",
        "tcp.port == 443 && udp.port == 53",
        r#"tcp.port in {80, 443} && tcp.payload matches "^GET""#,
    ];

    for filter in filters {
        group.bench_with_input(
            BenchmarkId::new("highlight", filter),
            &filter,
            |b, f| {
                b.iter(|| {
                    highlight_filter_syntax(black_box(f))
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_full_scan_filter,
    bench_indexed_filter,
    bench_incremental_filter,
    bench_parser,
    bench_autocomplete,
    bench_syntax_highlighting
);
criterion_main!(benches);
```

**Performance targets (from research):**
1. **Full filtering (100K events)**: <10ms (current: 9ms) ✓
2. **Incremental filtering (1K batch)**: <5ms (current: 87µs) ✓
3. **Parser (complex filter)**: <100µs
4. **Autocomplete update**: <1ms
5. **Syntax highlighting**: <1ms
6. **Query planner**: Indexed queries 10-100x faster than full scan

**Validation criteria:**
```rust
// Add to benchmark report analysis
#[cfg(test)]
mod validation_tests {
    #[test]
    fn validate_performance_targets() {
        // Run criterion benchmarks and assert on results
        // This is a smoke test, not a strict gate
        // Actual benchmarks run with: cargo bench

        let results = run_benchmarks();

        // Full filtering
        assert!(results.get("full_scan_filter/simple_transport/100000").unwrap() < Duration::from_millis(10));

        // Incremental filtering
        assert!(results.get("incremental_filter/batch_1000").unwrap() < Duration::from_millis(5));

        // Parser
        assert!(results.get("parser/parse/complex").unwrap() < Duration::from_micros(100));

        // Autocomplete
        assert!(results.get("autocomplete/fuzzy_match_short").unwrap() < Duration::from_millis(1));

        // Syntax highlighting
        assert!(results.get("syntax_highlighting/highlight").unwrap() < Duration::from_millis(1));

        // Query planner speedup
        let full_scan = results.get("full_scan_filter/simple_transport/100000").unwrap();
        let indexed = results.get("indexed_filter/indexed_transport/100000").unwrap();
        assert!(indexed < &(*full_scan / 10), "Index should be 10x faster");
    }
}
```

**Pre-Mortem Risks:**
1. **Benchmark noise**: CI environment may have variable performance (run multiple iterations)
2. **Test data representativeness**: Generated events may not match real traffic patterns (use captured PCAP if available)
3. **Query planner false negatives**: May not always use index when it should (track index usage rate)
4. **Regression detection**: Small regressions may be noise (set threshold at 20% degradation)

**Alternatives Ruled Out:**
- **Manual performance testing only**: No way to catch regressions in CI
- **Microbenchmarks only**: Need end-to-end benchmarks for realistic scenarios
- **dhat/profiling tools only**: Need automated suite for CI
- **Skip benchmarks**: Performance is already good, but need to maintain it

## Scope

**Files to create:**
- `crates/prb-tui/benches/filter_performance.rs` - Comprehensive benchmark suite
- `crates/prb-tui/tests/performance_validation_test.rs` - Smoke tests for performance targets

**Files to modify:**
- `crates/prb-tui/Cargo.toml` - Add criterion to dev-dependencies
- `.github/workflows/ci.yml` - Add benchmark job (optional, may be slow)

**Unchanged files:**
- All implementation files - this is purely validation

## Implementation Approach

1. **Add criterion dependency**
   - Add `criterion = { version = "0.5", features = ["html_reports"] }` to dev-dependencies
   - Configure criterion in Cargo.toml `[[bench]]` section

2. **Create benchmark suite structure**
   - One benchmark group per component:
     - full_scan_filter
     - indexed_filter
     - incremental_filter
     - parser
     - autocomplete
     - syntax_highlighting

3. **Generate realistic test data**
   - `generate_test_events()` with varied protocols, ports, payloads
   - Match distribution seen in real traces (30% HTTPS, 20% DNS, etc.)

4. **Implement each benchmark group**
   - Parameterize over dataset sizes (1K, 10K, 100K)
   - Parameterize over filter complexity (simple, complex AND, complex OR)
   - Use `black_box()` to prevent compiler optimization

5. **Add validation tests**
   - Smoke tests that assert on approximate performance targets
   - Don't fail CI on minor regressions, but flag large ones

6. **Document how to run benchmarks**
   - Add README section: `cargo bench --bench filter_performance`
   - Explain how to interpret criterion HTML reports

7. **Verify query planner effectiveness**
   - Compare indexed vs full-scan benchmarks
   - Assert indexed queries are at least 10x faster for selective filters

## Build and Test Commands

**Build:** `cargo build --package prb-tui`

**Run benchmarks:** `cargo bench --package prb-tui --bench filter_performance`

**View reports:** `open target/criterion/report/index.html` (after running benchmarks)

**Test (targeted):** `cargo test --package prb-tui performance_validation`

**Test (regression):** `cargo test --package prb-tui`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:**
   - `test_benchmark_suite_runs` - Benchmark suite executes without panics
   - `test_validate_full_scan_target` - 100K events < 10ms (approximate)
   - `test_validate_incremental_target` - 1K batch < 5ms (approximate)
   - `test_validate_indexed_faster` - Indexed queries faster than full scan

2. **Regression tests:** All existing prb-tui tests pass

3. **Full build gate:** `cargo build --workspace` succeeds

4. **Full test suite:** `cargo test --workspace --all-targets` passes

5. **Self-review gate:**
   - All benchmark groups have representative test cases
   - Test data matches real-world traffic patterns
   - Criterion configured with appropriate sample sizes
   - Benchmark reports generated successfully

6. **Scope verification gate:**
   - Only Cargo.toml and new benchmark files modified
   - No changes to implementation code
   - Benchmarks can be run independently

**Manual verification:**
- Run `cargo bench --bench filter_performance`
- Verify HTML report shows all benchmark groups
- Check that indexed queries are faster than full scan
- Verify all targets met:
  - Full filtering (100K): <10ms
  - Incremental (1K batch): <5ms
  - Parser: <100µs
  - Autocomplete: <1ms
  - Syntax highlighting: <1ms

**Risk Factor:** 2/10 - Pure validation code, no production impact

**Estimated Complexity:** Low - Criterion setup is straightforward, test data generation is simple

**Evidence for Optimality:**
1. **Existing solutions**: criterion is standard Rust benchmarking framework (used by serde, tokio, etc.)
2. **Codebase evidence**: Performance targets already met during research phase, just need to codify
3. **CI/CD best practices**: Automated performance testing prevents regressions (from Google SRE handbook)
4. **Database benchmarking**: Query planner effectiveness measured by comparing indexed vs full-scan (standard DB practice)
