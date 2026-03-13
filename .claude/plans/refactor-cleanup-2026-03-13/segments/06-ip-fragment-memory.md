---
segment: 6
title: "Fix IP Fragment Memory Leak"
depends_on: [1]
risk: 7/10
complexity: High
cycle_budget: 20
status: pending
commit_message: "fix(pcap): Replace Box::leak with proper lifetime management in IP fragmentation"
---

# Segment 6: Fix IP Fragment Memory Leak

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Eliminate intentional memory leak in IP fragmentation reassembly by implementing proper lifetime management using Arc and reference counting.

**Depends on:** Segment 1 (test utilities for comprehensive edge-case testing)

## Context: Issues Addressed

### Issue 06: IP Fragment Memory Leak (Critical Bug)

**Core Problem:** `Box::leak` is used intentionally in `/Users/psauer/probe/crates/prb-pcap/src/normalize.rs:366` to extend lifetime of fragmented packet buffers. This is a documented memory leak that grows unbounded with the number of fragmented packets processed.

**Evidence:**
```rust
// crates/prb-pcap/src/normalize.rs:366
let leaked: &'static [u8] = Box::leak(payload.into_boxed_slice());
self.fragments.insert(key, FragmentEntry {
    data: leaked,  // Leaked memory, never freed
    offset,
    more_fragments,
});
```

**Root Cause:**
1. `FragmentEntry` struct holds `&'static [u8]`, requiring static lifetime
2. Fragmented payloads come from pcap packets with limited lifetime
3. Original design used `Box::leak` as "quick fix" to satisfy lifetime constraints
4. Memory is never reclaimed - even after reassembly completes

**Proposed Fix:**
Replace `&'static [u8]` with `Arc<[u8]>` for reference-counted shared ownership:
1. Change `FragmentEntry.data` from `&'static [u8]` to `Arc<[u8]>`
2. Use `Arc::from(payload)` instead of `Box::leak`
3. Memory automatically freed when last Arc is dropped (after reassembly)
4. No lifetime constraints - Arc owns the data

**Pre-Mortem Risks:**
- **Performance impact**: Arc has atomic reference counting overhead
  - Mitigation: Benchmark fragmentation hot path, verify <5% regression
  - Context: IP fragmentation is rare in modern networks (mostly IPv6 which avoids fragmentation)
- **Clone semantics**: Accidental over-cloning could cause memory spike
  - Mitigation: Use `Arc::clone` explicitly, add comments about ref-count semantics
  - Test: Memory profiling test with 1000 fragmented packets
- **Lifetime bugs**: Might introduce use-after-free if Arc is dropped too early
  - Mitigation: Comprehensive unit tests for reassembly edge cases
  - Test: Property tests with random fragment ordering/drops

## Scope

**Subsystem:** IP fragmentation reassembly (packet normalization)

**Crates affected:**
- `crates/prb-pcap/` (normalize.rs, fragment handling)

**Files to modify:**
- `/Users/psauer/probe/crates/prb-pcap/src/normalize.rs:366` (Box::leak → Arc)
- `/Users/psauer/probe/crates/prb-pcap/src/normalize.rs:50-80` (FragmentEntry struct definition)
- `/Users/psauer/probe/crates/prb-pcap/src/normalize.rs:200-250` (reassembly logic)

## Key Files and Context

### Current Implementation (normalize.rs)

**FragmentEntry struct** (lines 50-80):
```rust
struct FragmentEntry {
    data: &'static [u8],    // Leaked memory
    offset: u16,
    more_fragments: bool,
}
```

**Fragment insertion** (line 366):
```rust
let leaked: &'static [u8] = Box::leak(payload.into_boxed_slice());
self.fragments.insert(key, FragmentEntry {
    data: leaked,
    offset,
    more_fragments,
});
```

**Reassembly logic** (lines 200-250):
```rust
fn reassemble(&mut self, key: FragmentKey) -> Option<Vec<u8>> {
    let entries = self.fragments.remove(&key)?;
    let mut result = Vec::new();
    for entry in entries {
        result.extend_from_slice(entry.data);  // Copies from leaked memory
        // entry.data is never freed - memory leak
    }
    Some(result)
}
```

### Architecture Context
- IP fragmentation is handled in the normalization layer (before TCP reassembly)
- Fragments are keyed by (src_ip, dst_ip, fragment_id) tuple
- Maximum fragment buffer: 64KB per fragmented packet (IPv4 limit)
- Fragment timeout: 60 seconds (per RFC 791)

### Performance Considerations
- Hot path: `/Users/psauer/probe/crates/prb-pcap/src/pipeline_core.rs:112-163` (process_packet)
- IP fragmentation is a cold path (rarely hit in production traces)
- Benchmark target: <5% regression on fragmented packet processing
- Memory target: Zero leaked bytes after reassembly

### Project Conventions
- ADR 0002: "Never panic on malformed input" - handle fragment errors gracefully
- CONTRIBUTING.md line 157: "No .unwrap() in library code" - Arc::clone is infallible but document assumptions
- Memory safety: Use Arc for shared ownership, not raw pointers or static lifetimes

## Implementation Approach

### Step 1: Update FragmentEntry struct
```rust
// crates/prb-pcap/src/normalize.rs:50-80
use std::sync::Arc;

struct FragmentEntry {
    data: Arc<[u8]>,       // Reference-counted, automatically freed
    offset: u16,
    more_fragments: bool,
}
```

### Step 2: Replace Box::leak with Arc
```rust
// crates/prb-pcap/src/normalize.rs:366
// BEFORE:
// let leaked: &'static [u8] = Box::leak(payload.into_boxed_slice());

// AFTER:
let data: Arc<[u8]> = Arc::from(payload);  // No leak, ref-counted
self.fragments.insert(key, FragmentEntry {
    data,  // Arc owns the data
    offset,
    more_fragments,
});
```

### Step 3: Update reassembly logic
```rust
// crates/prb-pcap/src/normalize.rs:200-250
fn reassemble(&mut self, key: FragmentKey) -> Option<Vec<u8>> {
    let entries = self.fragments.remove(&key)?;
    let total_size: usize = entries.iter().map(|e| e.data.len()).sum();
    let mut result = Vec::with_capacity(total_size);

    for entry in entries {
        result.extend_from_slice(&entry.data);
        // entry.data (Arc) is dropped here, ref-count decremented
        // If this was the last reference, memory is freed
    }

    Some(result)
}
```

### Step 4: Add fragment timeout mechanism
Currently fragments live forever. Add timeout to prevent memory buildup:
```rust
struct FragmentEntry {
    data: Arc<[u8]>,
    offset: u16,
    more_fragments: bool,
    received_at: Instant,  // NEW: Track when fragment arrived
}

impl FragmentCache {
    fn expire_old_fragments(&mut self, now: Instant) {
        const FRAGMENT_TIMEOUT: Duration = Duration::from_secs(60);

        self.fragments.retain(|_key, entries| {
            entries.iter().any(|e| now.duration_since(e.received_at) < FRAGMENT_TIMEOUT)
        });
    }
}
```

### Step 5: Add tests for memory safety

**Test 1: Memory is freed after reassembly**
```rust
#[test]
fn test_fragments_freed_after_reassembly() {
    use std::sync::Weak;

    let mut cache = FragmentCache::new();
    let payload = vec![1, 2, 3, 4];
    let arc_data = Arc::from(payload.as_slice());
    let weak = Arc::downgrade(&arc_data);  // Weak reference for testing

    // Insert fragment
    cache.insert(key, FragmentEntry {
        data: arc_data,
        offset: 0,
        more_fragments: false,
    });

    // Reassemble
    let _result = cache.reassemble(key);

    // Arc should be dropped, weak reference should be invalid
    assert!(weak.upgrade().is_none(), "Memory should be freed after reassembly");
}
```

**Test 2: Partial reassembly doesn't leak**
```rust
#[test]
fn test_partial_fragments_freed_on_timeout() {
    let mut cache = FragmentCache::new();

    // Insert first fragment
    cache.insert_fragment(key, payload1, 0, true);

    // Expire without receiving remaining fragments
    cache.expire_old_fragments(Instant::now() + Duration::from_secs(61));

    // Memory should be freed
    assert_eq!(cache.fragments.len(), 0);
}
```

### Step 6: Benchmark performance impact
```rust
// benches/ip_fragmentation_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_fragment_reassembly(c: &mut Criterion) {
    c.bench_function("reassemble_100_fragments", |b| {
        b.iter(|| {
            let mut cache = FragmentCache::new();
            for i in 0..100 {
                cache.insert_fragment(
                    key,
                    black_box(vec![i as u8; 1024]),
                    i * 1024,
                    i < 99,
                );
            }
            black_box(cache.reassemble(key));
        });
    });
}

criterion_group!(benches, bench_fragment_reassembly);
criterion_main!(benches);
```

## Alternatives Ruled Out

1. **Keep Box::leak, add manual cleanup**
   - Rejected: Error-prone, easy to forget cleanup, doesn't solve root cause
   - Why: Memory management is exactly what Arc is designed for

2. **Use Rc instead of Arc**
   - Rejected: Rc is not thread-safe, future work might parallelize packet processing
   - Why: Arc has minimal overhead (atomic ops are fast), future-proofs design

3. **Use Bytes crate (already in project)**
   - Evaluated: Bytes is optimized for zero-copy I/O, but requires ownership transfer
   - Rejected: Fragments need shared ownership (multiple references during reassembly), Arc<[u8]> is simpler

4. **Rewrite with lifetime annotations**
   - Rejected: Fragments outlive the original pcap packet, static lifetime is incorrect, complex lifetime annotations don't solve the problem
   - Why: Arc is the idiomatic Rust solution for shared ownership

## Pre-Mortem Risks

1. **Performance regression on hot path**
   - **Watch for**: Atomic reference count operations causing slowdown
   - **Test**: Run `cargo bench --bench ip_fragmentation_bench`, verify <5% regression
   - **Mitigation**: IP fragmentation is cold path (rare in modern networks), acceptable tradeoff

2. **Accidental Arc over-cloning**
   - **Watch for**: `arc.clone()` in hot loops causing memory spike
   - **Test**: Memory profiler (cargo-instruments on macOS) with 10k fragmented packets
   - **Mitigation**: Use `Arc::clone(&arc)` explicitly with comments about ref-count semantics

3. **Use-after-free if Arc dropped prematurely**
   - **Watch for**: Fragment data accessed after reassembly removes it from cache
   - **Test**: Comprehensive unit tests for all reassembly paths (success, timeout, partial)
   - **Mitigation**: Rust's ownership prevents use-after-free at compile time

4. **Timeout mechanism adds complexity**
   - **Watch for**: Off-by-one errors in timeout calculation, timezone issues
   - **Test**: Property test with random fragment arrival times
   - **Mitigation**: Use `Instant` (monotonic) not `SystemTime` (can go backwards)

5. **Breaking change to FragmentEntry struct**
   - **Watch for**: Other code depending on `&'static [u8]` lifetime
   - **Test**: `cargo check --workspace` must pass
   - **Mitigation**: FragmentEntry is private to normalize.rs, no external dependencies

## Build and Test Commands

**Build:**
```bash
cargo build --package prb-pcap
```

**Test (targeted):**
```bash
# Unit tests for fragmentation logic
cargo test --package prb-pcap --lib normalize::tests

# Specific tests added in this segment
cargo test --package prb-pcap --lib test_fragments_freed_after_reassembly
cargo test --package prb-pcap --lib test_partial_fragments_freed_on_timeout
```

**Test (regression):**
```bash
# All pcap tests (TCP reassembly, IP parsing, etc.)
cargo test --package prb-pcap

# Integration tests using fragmented pcap files
cargo test --package prb-pcap --test pipeline_tests
```

**Test (full gate):**
```bash
cargo test --workspace --all-targets
```

**Benchmark (performance validation):**
```bash
# Run fragmentation benchmark
cargo bench --package prb-pcap --bench ip_fragmentation_bench

# Compare with baseline (should be <5% regression)
```

**Memory profiling (optional but recommended):**
```bash
# macOS only - requires Instruments.app
cargo instruments --package prb-pcap --test normalize_tests --template Leaks
```

## Exit Criteria

1. **Targeted tests:**
   - `cargo test --package prb-pcap --lib normalize`: All fragmentation tests pass
   - New test `test_fragments_freed_after_reassembly`: Verifies Arc is dropped (uses Weak reference)
   - New test `test_partial_fragments_freed_on_timeout`: Verifies timeout logic
   - Property test with random fragment ordering (using prb-test-utils + proptest)

2. **Regression tests:**
   - `cargo test --package prb-pcap`: All pcap tests pass (no behavior changes)
   - `cargo test --package prb-integration-tests`: End-to-end tests with fragmented pcap files pass
   - No test timing regressions (IP fragmentation is cold path)

3. **Full build gate:**
   - `cargo build --workspace`: Clean build with no warnings
   - `cargo clippy --workspace --all-targets -- -D warnings`: No clippy warnings (Arc usage is idiomatic)
   - `cargo doc --package prb-pcap`: Documentation builds (add doc comment explaining Arc choice)

4. **Full test suite gate:**
   - `cargo test --workspace --all-targets`: All tests pass
   - Coverage remains ≥80% (add tests for timeout logic)

5. **Performance gate:**
   - `cargo bench --package prb-pcap --bench ip_fragmentation_bench`: <5% regression vs baseline
   - If regression >5%, profile with cargo-flamegraph and optimize Arc usage

6. **Self-review gate:**
   - No Box::leak calls remain in normalize.rs (grep confirms)
   - No TODO or FIXME comments added
   - No commented-out code
   - Memory leak eliminated (verified by Weak reference test)
   - Doc comments added explaining Arc choice and lifetime rationale

7. **Scope verification gate:**
   - **Modified files:**
     - `crates/prb-pcap/src/normalize.rs` (FragmentEntry struct, Box::leak → Arc, timeout logic)
     - `crates/prb-pcap/Cargo.toml` (no changes needed, std::sync::Arc is stdlib)
   - **New files:**
     - `crates/prb-pcap/benches/ip_fragmentation_bench.rs` (performance validation)
     - `crates/prb-pcap/tests/fragment_memory_test.rs` (memory safety tests)
   - No other files modified (fragmentation is isolated to normalize.rs)
