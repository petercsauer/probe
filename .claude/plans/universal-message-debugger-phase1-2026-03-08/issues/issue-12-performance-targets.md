---
id: "12"
title: "Performance Targets Lack Methodology"
risk: 2/10
addressed_by_subsections: [5]
---

# Issue 12: Performance Targets Lack Methodology

**Core Problem:**
The plan states targets (100k events/sec ingest, 1M event sessions, <500ms session load) without specifying hardware baseline, measurement methodology, event size, or compression settings.

**Root Cause:**
Performance targets were stated aspirationally rather than derived from use cases or benchmarked against reference implementations.

**Proposed Fix:**
Define a benchmarking framework in Subsection 5:
- Use `criterion` for micro-benchmarks (decode latency, storage throughput).
- Define standard test scenarios: small (1K events, 100KB), medium (100K events, 50MB), large (1M events, 500MB).
- Specify a reference hardware class (e.g., "modern laptop: 8+ cores, 16GB RAM, NVMe SSD").
- Measure wall-clock time, peak RSS, and throughput.
- Targets become: "on reference hardware, the medium scenario completes ingest in <1s and loads in <500ms."

**Existing Solutions Evaluated:**
- `criterion` (crates.io, 28M+ downloads) -- standard Rust benchmarking library. Adopted.
- `divan` (crates.io) -- newer, attribute-macro-based benchmarking. Simpler API but less ecosystem adoption.

**Recommendation:** Use `criterion` for compatibility and ecosystem support.

**Alternatives Considered:**
- Skip formal benchmarks; use ad-hoc timing. Rejected: benchmarks without methodology are not reproducible.

**Pre-Mortem -- What Could Go Wrong:**
- Benchmark results vary wildly between CI and developer machines.
- Criterion's statistical model may be overkill for this project's needs.

**Risk Factor:** 2/10

**Evidence for Optimality:**
- External evidence: `criterion` is the standard Rust benchmarking library recommended by the Rust Performance Book.
- Existing solutions: MCAP's own benchmarks (visible in foxglove/mcap repo) use similar methodology with defined event sizes and hardware specs.

**Blast Radius:**
- Direct: benchmark harness (new crate or test directory)
- Ripple: CI pipeline (benchmarks should run but not gate PRs)
