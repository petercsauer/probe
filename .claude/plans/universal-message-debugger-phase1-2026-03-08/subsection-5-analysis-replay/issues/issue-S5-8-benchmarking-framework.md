---
id: "S5-8"
title: "Benchmarking Framework Selection"
risk: 2/10
addressed_by_segments: [6]
---
# Issue S5-8: Benchmarking Framework Selection

**Core Problem:**
The parent plan recommends `criterion` for benchmarking. While criterion (v0.8.2, Feb 2026) is actively maintained, `divan` (v0.1.21) is now the preferred choice in the Rust ecosystem with features directly relevant to this project: allocation profiling (catches memory bloat in large-capture processing), thread contention insights (relevant for concurrent correlation), and a simpler API.

**Root Cause:**
The parent plan was written when criterion was the unquestioned default. The ecosystem has since shifted toward divan.

**Proposed Fix:**
Use `divan` as the primary benchmarking framework. Define standard test scenarios:

| Scenario | Events | Approx Size | Purpose |
|---|---|---|---|
| Small | 1,000 | ~100 KB | Unit benchmark for single-operation latency |
| Medium | 100,000 | ~50 MB | Integration benchmark for full pipeline |
| Large | 1,000,000 | ~500 MB | Stress test for memory and throughput |

Each event is a `DebugEvent` with: 8-byte timestamp, 4-byte transport enum, 32-byte metadata (connection_id, stream_id, topic), variable payload (avg 200 bytes for protobuf, 50 bytes for headers).

Benchmarks to implement:
- `bench_correlate_grpc`: correlate medium gRPC session, measure events/sec
- `bench_correlate_dds`: correlate medium DDS session with discovery phase, measure events/sec
- `bench_replay_stdout`: replay medium session at max speed, measure MB/s
- `bench_mcap_filter`: filter 1% of a large session, measure scan throughput
- `bench_flow_query`: compute flows from medium session, measure wall-clock time

Reference hardware: "modern laptop: 8+ cores, 16GB+ RAM, NVMe SSD." Targets: medium scenario ingest < 1s, replay at max speed < 2s, flow computation < 500ms.

**Existing Solutions Evaluated:**
- `divan` (crates.io, v0.1.21, 2.67M+ downloads, 266 reverse deps, actively maintained) -- attribute-macro benchmarking with allocation profiling. Adopted.
- `criterion` (crates.io, v0.8.2, 28M+ downloads, actively maintained) -- traditional statistical benchmarking. Viable fallback but lacks allocation profiling.

**Alternatives Considered:**
- Use criterion for broader ecosystem compatibility. Not rejected outright, but divan's allocation profiling is a differentiator for processing large captures.
- Skip formal benchmarks, use `std::time::Instant` in tests. Rejected: not statistically rigorous; noisy and non-reproducible.

**Pre-Mortem -- What Could Go Wrong:**
- divan MSRV is Rust 1.80.0. If project targets older MSRV, divan will not compile.
- Benchmark fixtures (test MCAP files with known event counts) must be generated and committed. If too large for git, need a generation script.
- Benchmark results vary between CI and developer machines. Document reference hardware and accept variance.

**Risk Factor:** 2/10

**Evidence for Optimality:**
- Existing solutions: CodSpeed (benchmark CI platform) recommends divan as "most convenient way to run Rust benchmarks."
- External evidence: divan's allocation profiling directly addresses memory efficiency concerns with large captures.

**Blast Radius:**
- Direct: `benches/` directory, `Cargo.toml` dev-dependencies
- Ripple: CI pipeline (benchmarks should run but not gate PRs)
