---
segment: 6
title: "Benchmark Framework"
depends_on: []
risk: 2/10
complexity: Low
cycle_budget: 10
status: pending
commit_message: "feat(bench): add divan benchmark framework with synthetic fixtures and initial benchmarks"
---

# Segment 6: Benchmark Framework

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Set up the divan-based benchmarking framework with standard test scenarios, fixture generation, and initial benchmarks for correlation and replay.

**Depends on:** None (can run in parallel with any segment, including Segment 1). Benchmarks will initially use synthetic data; real-protocol benchmarks can be added after Segments 2-5 land.

## Context: Issues Addressed

**S5-8: Benchmarking Framework Selection**

- **Core Problem:** The parent plan recommended `criterion`. `divan` is now preferred with allocation profiling (catches memory bloat in large-capture processing), thread contention insights (relevant for concurrent correlation), and simpler API.
- **Proposed Fix:** Use `divan` as primary benchmarking framework. Standard scenarios: Small (1K events, ~100KB), Medium (100K events, ~50MB), Large (1M events, ~500MB). Benchmarks: `bench_correlate_grpc`, `bench_correlate_dds`, `bench_replay_stdout`, `bench_mcap_filter`, `bench_flow_query`. Reference hardware: modern laptop, 8+ cores, 16GB+ RAM, NVMe SSD. Targets: medium ingest < 1s, replay at max speed < 2s, flow computation < 500ms.
- **Pre-Mortem risks:** divan MSRV 1.80.0 may conflict with project MSRV; fixture generation for Large scenario is slow (cache to temp path); benchmark results vary between machines.

## Scope

- `crates/prb-bench/` -- benchmark crate (or `benches/` directory at workspace root)
- Fixture generation script for synthetic MCAP sessions
- Initial benchmark functions

## Key Files and Context

- `crates/prb-bench/Cargo.toml` -- benchmark crate with `divan` dependency. Uses `[[bench]]` targets with `harness = false`.
- `crates/prb-bench/benches/correlation.rs` -- correlation benchmarks
- `crates/prb-bench/benches/replay.rs` -- replay benchmarks
- `crates/prb-bench/src/fixtures.rs` -- synthetic MCAP session generator

divan (v0.1.21+, crates.io):
- MSRV: Rust 1.80.0
- Attribute-macro API: `#[divan::bench]` on functions
- Allocation profiling: `#[divan::bench(allocs)]` measures allocations per iteration
- Generic benchmarks: `#[divan::bench(types = [SmallSession, MediumSession])]`
- Thread contention: `#[divan::bench(threads = [1, 4, 8])]`

Standard test scenarios:

| Scenario | Events | Approx Size | DebugEvent composition |
|---|---|---|---|
| Small | 1,000 | ~100 KB | 8B timestamp + 4B transport + 32B metadata + ~200B avg payload |
| Medium | 100,000 | ~50 MB | Same structure, realistic transport mix (60% gRPC, 25% ZMQ, 15% DDS) |
| Large | 1,000,000 | ~500 MB | Same structure, stress test |

Reference hardware: modern laptop, 8+ cores, 16GB+ RAM, NVMe SSD.

Performance targets:
- Medium scenario correlation: < 500ms wall-clock
- Medium scenario replay at max speed: < 2s wall-clock
- Medium scenario flow computation: < 500ms
- Large scenario correlation: < 5s wall-clock

## Implementation Approach

1. Create `crates/prb-bench/` as a library crate with `[[bench]]` targets.
2. Add `divan` as a dev-dependency. Configure `harness = false` for bench targets.
3. Implement `fixtures.rs`: generate synthetic MCAP files with configurable event count, transport mix, and payload size. Use `mcap::Writer` to create valid MCAP sessions in a tempdir. Cache generated fixtures to avoid regeneration on every bench run.
4. Implement `correlation.rs` benchmarks:
   - `bench_correlate_generic_small`: correlate small synthetic session with generic strategy
   - `bench_correlate_generic_medium`: correlate medium session, measure events/sec
   - `bench_correlate_generic_medium_allocs`: same with allocation profiling
5. Implement `replay.rs` benchmarks:
   - `bench_replay_max_speed_medium`: replay medium session at max speed to `/dev/null`, measure throughput
   - `bench_mcap_filter_medium`: filter 10% of medium session, measure scan throughput
6. Add a `bench` profile to workspace `Cargo.toml` with `opt-level = 3`.
7. Document reference hardware and how to run benchmarks in a `BENCHMARKS.md` or inline doc comments.

## Alternatives Ruled Out

- `criterion` instead of divan (lacks allocation profiling which is directly relevant)
- `std::time::Instant` in unit tests (not statistically rigorous, noisy)
- Committing large pre-built fixture files to git (use generation script instead)

## Pre-Mortem Risks

- divan MSRV 1.80.0 may conflict if project targets older Rust. Verify workspace MSRV.
- Fixture generation for Large scenario (1M events, ~500MB) is slow. Cache to a well-known temp path.
- Benchmark results vary between machines. Accept variance; document reference hardware.
- Benchmarks must not accidentally run in CI test suite (separate `cargo bench` vs `cargo test`).

## Build and Test Commands

- Build: `cargo build -p prb-bench`
- Test (targeted): `cargo bench -p prb-bench` (runs all benchmarks)
- Test (regression): `cargo nextest run --workspace` (benchmarks excluded from test suite)
- Test (full gate): `cargo nextest run --workspace && cargo bench -p prb-bench --no-run` (verify bench compilation)

## Exit Criteria

1. **Targeted tests:**
   - `bench_correlate_generic_medium` runs and produces a throughput measurement (events/sec) without panic.
   - `bench_correlate_generic_medium_allocs` runs and reports allocation count per iteration.
   - `bench_replay_max_speed_medium` runs and produces throughput (MB/s).
   - `bench_mcap_filter_medium` runs and produces scan throughput.
   - Fixture generator creates valid MCAP files readable by `mcap::MessageStream`.
   - All benchmarks compile with `cargo bench --no-run`.
2. **Regression tests:** All existing workspace tests pass (`cargo nextest run --workspace`).
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo nextest run --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changes within `crates/prb-bench/` and workspace `Cargo.toml` (bench profile). No changes to production crates.
