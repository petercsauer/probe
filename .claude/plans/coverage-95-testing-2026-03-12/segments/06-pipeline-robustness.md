---
segment: 6
title: "Pipeline Core Robustness"
depends_on: [5]
risk: 8/10
complexity: Medium
cycle_budget: 12
status: merged
commit_message: "fix(pcap-pipeline): Add error handling and warning capacity limit in hot path"
---

# Segment 6: Pipeline Core Robustness

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Increase pipeline_core.rs coverage from 21.38% to 75%+ and fix hot path safety issues.

**Depends on:** Segment 5 (normalization fixes might surface pipeline issues)

## Context: Issues Addressed

**Core Problem:** Pipeline hot path (called once per packet) has panic risks and silent error handling. Line 276 has `.unwrap()` after `.is_empty()` check - race condition or logic error could panic. Lines 222-228 silently swallow all decoder errors by converting them to fallback events, masking failures. Lines 120, 162, 179 have unbounded warning accumulation (`warnings.push()` without capacity limit) - memory exhaustion risk. Current coverage 21.38% indicates error injection and edge case handling completely untested.

**Proposed Fix:** Replace unwrap with defensive error handling, add warning capacity limit with LRU eviction, create comprehensive error injection tests (corrupt packets, invalid protocols, decoder failures), add property test for pipeline robustness, optionally add stress benchmark.

**Pre-Mortem Risks:**
- LRU cache adds allocations in hot path - mitigate by using SmallVec for small counts (<10 warnings), only LRU when approaching limit
- Error injection could surface bugs in decoders - good, that's the point of testing, fix them
- Stress test might timeout in CI - make it a benchmark (benches/), not a test (tests/), run separately

## Scope

- `crates/prb-pcap/src/pipeline_core.rs` (280 lines)
- `crates/prb-pcap/tests/pipeline_error_injection_test.rs` - New error injection tests
- `crates/prb-pcap/tests/pipeline_property_test.rs` - New property tests
- `crates/prb-pcap/benches/pipeline_stress_bench.rs` - Optional stress benchmark
- Add `lru` dependency to Cargo.toml

## Key Files and Context

**`crates/prb-pcap/src/pipeline_core.rs`** (280 lines):
- Line 276: `.unwrap()` after `.is_empty()` check - race condition or logic error could panic in hot path
- Lines 222-228: Silent error swallowing - all decoder errors converted to fallback events, masks failures
- Lines 120, 162, 179: Unbounded warning accumulation - `warnings.push()` without capacity limit, memory exhaustion risk
- Hot path: Called once per packet, must never panic

**`crates/prb-pcap/tests/pipeline_tests.rs`**:
- Existing tests focus on happy path
- Current coverage: 21.38% indicates error injection and edge case handling completely untested

## Implementation Approach

1. **Replace unwrap at line 276** with defensive error handling:
   ```rust
   // Before:
   Some(events.into_iter().next().unwrap())

   // After:
   match events.into_iter().next() {
       Some(event) => Some(event),
       None => {
           error!("Unexpected empty events after non-empty check at {}:{}", file!(), line!());
           stats.unexpected_empty_events += 1;
           None
       }
   }
   ```

2. **Add warning capacity limit** with LRU eviction:
   - Add `lru = "0.12"` to Cargo.toml dependencies
   - Change `Vec<String>` warnings to use SmallVec or LRU:
   ```rust
   const MAX_WARNINGS: usize = 100;
   if warnings.len() >= MAX_WARNINGS {
       // Use LRU cache to evict oldest warnings
       let mut lru = LruCache::new(NonZeroUsize::new(MAX_WARNINGS).unwrap());
       for w in warnings.drain(..) {
           lru.put(w.clone(), ());
       }
       warnings = lru.iter().map(|(k, _)| k.clone()).collect();
   }
   warnings.push(new_warning);
   ```

3. **Add error injection tests** in `tests/pipeline_error_injection_test.rs`:
   - Corrupt packets: invalid checksums, wrong lengths, truncated headers
   - Invalid protocol numbers: protocol 255 (unknown)
   - Decoder failures: Create mock decoder that returns errors
   - Verify errors logged but pipeline doesn't panic
   - Test error recovery: After error, next valid packet processes correctly
   - Test cases:
     - Malformed IP header
     - Truncated TCP header
     - Unknown protocol in IP header
     - Decoder throws DecodeError
     - TLS decryption failure
     - All should result in fallback event or error event, never panic

4. **Add property test** for pipeline robustness in `tests/pipeline_property_test.rs`:
   ```rust
   proptest! {
       #[test]
       fn pipeline_never_panics_with_arbitrary_packets(
           packets in prop::collection::vec(
               prop::collection::vec(any::<u8>(), 0..2000),
               0..100
           )
       ) {
           let mut pipeline = PipelineCore::new(None, DecoderRegistry::default());
           for packet_data in packets {
               // Should never panic regardless of input
               let _ = pipeline.process_packet(1, 0, &packet_data, "test");
           }
       }
   }
   ```

5. **Add stress test** (benchmark) in `benches/pipeline_stress_bench.rs`:
   ```rust
   fn benchmark_pipeline_with_errors(c: &mut Criterion) {
       c.bench_function("pipeline_100k_packets_10pct_errors", |b| {
           b.iter(|| {
               let mut pipeline = PipelineCore::new(None, DecoderRegistry::default());
               for i in 0..100_000 {
                   let packet = if i % 10 == 0 {
                       create_corrupt_packet() // 10% error rate
                   } else {
                       create_valid_packet()
                   };
                   let _ = pipeline.process_packet(1, i * 1000, &packet, "bench");
               }
           });
       });
   }
   ```

## Alternatives Ruled Out

- **Removing warnings field entirely:** Rejected - valuable for debugging protocol issues, limit capacity instead of removing
- **Panicking on warning overflow:** Rejected - hot path must never panic, dropping old warnings is safer

## Pre-Mortem Risks

- LRU cache adds allocations in hot path: Mitigate by using SmallVec for small counts (<10 warnings), only LRU when approaching limit
- Error injection could surface bugs in decoders: Good - that's the point of testing, fix them
- Stress test might timeout in CI: Make it a benchmark (benches/), not a test (tests/), run separately

## Build and Test Commands

- Build: `cargo build -p prb-pcap`
- Test (targeted): `cargo test -p prb-pcap pipeline_error_injection pipeline_property pipeline_warning_capacity`
- Test (regression): `cargo test -p prb-pcap pipeline`
- Test (full gate): `cargo nextest run -p prb-pcap`
- Benchmark (optional): `cargo bench -p prb-pcap pipeline_stress`

## Exit Criteria

1. **Targeted tests:**
   - `pipeline_error_injection` - 10 error types handled gracefully (malformed IP, truncated TCP, unknown protocol, decoder error, TLS failure, etc.)
   - `pipeline_property` - proptest passes (no panics on 100 arbitrary packet sequences)
   - `pipeline_warning_capacity` - warnings capped at 100 (LRU eviction works)

2. **Regression tests:** All pipeline tests in `tests/pipeline_tests.rs`, `tests/pipeline_error_tests.rs` pass

3. **Full build gate:** `cargo build -p prb-pcap` succeeds with zero warnings

4. **Full test gate:** `cargo nextest run -p prb-pcap` passes

5. **Self-review gate:**
   - Unwrap replaced with error handling
   - Warning capacity limit implemented
   - Error logging added for unexpected states

6. **Scope verification gate:** Only modified:
   - `pipeline_core.rs` - error handling and warning limit
   - Test files in `tests/` directory
   - Optional benchmark in `benches/` directory
   - Added lru dependency to Cargo.toml
