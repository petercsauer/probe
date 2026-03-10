---
segment: 4
title: "prb-pcap Pipeline Tests"
depends_on: []
risk: 4
complexity: Medium
cycle_budget: 6
status: pending
commit_message: "test(prb-pcap): add unit tests for normalize, streaming, keylog, pipeline"
---

# Segment 4: prb-pcap Pipeline Tests

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Bring prb-pcap from ~82% to ≥90% line coverage by testing pipeline components, normalize edge cases, TLS keylog, and streaming.

**Depends on:** None

## Coverage Gaps

| File | Lines | Current | Target | Deficit |
|------|-------|---------|--------|---------|
| prb-pcap/src/normalize.rs | 576 | 73% | 90% | ~97 |
| prb-pcap/src/parallel/streaming.rs | 614 | 80% | 90% | ~61 |
| prb-pcap/src/tls/keylog.rs | 196 | 60% | 90% | ~59 |
| prb-pcap/src/pipeline.rs | 140 | 72% | 90% | ~25 |
| prb-pcap/src/pipeline_core.rs | 304 | 84% | 90% | ~17 |
| prb-pcap/src/reader.rs | 211 | 77% | 90% | ~26 |
| prb-pcap/src/parallel/orchestrator.rs | 265 | 74% | 90% | ~43 |
| prb-pcap/src/tls/decrypt.rs | 344 | 83% | 90% | ~25 |
| prb-pcap/src/tls/mod.rs | 108 | 80% | 90% | ~11 |
| prb-pcap/src/parallel/shard.rs | 383 | 84% | 90% | ~21 |
| prb-pcap/src/parallel/normalize.rs | 362 | 88% | 90% | ~7 |

## Scope

All files in `crates/prb-pcap/src/` listed above.

## Implementation Approach

### normalize.rs (73% → 90%)
- Test IPv6 packet normalization (likely uncovered path)
- Test VLAN-tagged frames
- Test truncated packets, malformed headers
- Test fragment reassembly edge cases

### streaming.rs (80% → 90%)
- Test micro-batch accumulation and flush on timeout
- Test backpressure behavior when channel is full
- Test graceful shutdown via stop signal
- Test stats collection during streaming

### keylog.rs (60% → 90%)
- Test NSS keylog file parsing for all line formats (CLIENT_RANDOM, CLIENT_HANDSHAKE_TRAFFIC_SECRET, etc.)
- Test malformed lines (skip gracefully)
- Test TLS 1.2 vs 1.3 key extraction
- Test empty file, file with only comments

### pipeline.rs + pipeline_core.rs (72-84% → 90%)
- Test pipeline with TLS keylog enabled
- Test pipeline with protocol detection enabled
- Test error propagation through pipeline stages

### reader.rs (77% → 90%)
- Test pcapng format reading (SHB, IDB, EPB blocks)
- Test pcap legacy format
- Test corrupted/truncated file handling

### orchestrator.rs (74% → 90%)
- Test parallel orchestration with varying thread counts
- Test with empty input
- Test stats aggregation across shards

### tls/ files (80-83% → 90%)
- Test TLS 1.2 and 1.3 decryption paths
- Test session resumption
- Test invalid/missing keys gracefully

## Build and Test Commands

- Build: `cargo check -p prb-pcap`
- Test (targeted): `cargo nextest run -p prb-pcap`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** `cargo nextest run -p prb-pcap` — all new tests pass
2. **Coverage gate:** Every file in prb-pcap ≥ 88% line coverage
3. **Regression tests:** `cargo nextest run --workspace` — no regressions
4. **Full build gate:** `cargo build --workspace`
5. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
6. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
7. **Scope verification gate:** Only prb-pcap test and source files modified
