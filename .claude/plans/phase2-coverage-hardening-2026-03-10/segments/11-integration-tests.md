---
segment: 11
title: "Cross-Crate Integration Tests"
depends_on: [1, 2, 4, 5, 6, 9, 10]
risk: 4
complexity: Medium
cycle_budget: 5
status: pending
commit_message: "test(workspace): add cross-crate integration tests for pipeline and decode paths"
---

# Segment 11: Cross-Crate Integration Tests

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Add integration tests that exercise cross-crate boundaries, catching coverage gaps in glue code and error paths that unit tests miss.

**Depends on:** Segments 1, 2, 4, 5, 6, 9, 10 (unit tests must pass first)

## Scope

Integration tests in `tests/` directory at workspace root, exercising:
- PCAP → normalize → reassemble → decode → DebugEvent pipeline
- DebugEvent → conversation reconstruction → metrics
- DebugEvent → export (CSV, HAR, OTLP)
- DebugEvent → query filter → filtered results
- Schema load → schema-backed decode
- Protocol detection → decoder dispatch

## Implementation Approach

### Pipeline integration (pcap → events)
- Create a test that reads a fixture .pcap file through the full pipeline
- Verify events have correct protocol, source/dest, payloads
- Test with TLS keylog (decrypt → decode)
- Test parallel pipeline produces same results as serial

### Conversation reconstruction integration
- Feed a set of correlated events through ConversationEngine
- Verify conversations are correctly grouped
- Verify metrics (duration, TTFR) are reasonable
- Test with mixed protocols (gRPC + ZMQ in same capture)

### Export integration
- Load events → export to CSV → verify CSV structure
- Load events → export to HAR → verify HAR JSON schema
- Load events → export to OTLP → import back → verify round-trip

### Query filter integration
- Parse filter expressions → evaluate against real DebugEvents
- Test complex filters: `proto == "grpc" && latency > 100ms`
- Test filter with conversation-level predicates

### Protocol detection integration
- Feed raw TCP stream through detector → verify correct protocol identified
- Test detection with encrypted stream (should return Unknown)
- Test detection cascade: user override > port mapping > magic bytes > heuristic

## Build and Test Commands

- Build: `cargo check --workspace`
- Test (targeted): `cargo nextest run --workspace --test '*integration*'` or `cargo nextest run --workspace -E 'test(integration)'`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** At least 15 new integration tests covering cross-crate paths
2. **Coverage improvement:** Workspace total improves by ≥2 percentage points from integration tests alone
3. **Regression tests:** `cargo nextest run --workspace` — no regressions
4. **Full build gate:** `cargo build --workspace`
5. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
6. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
7. **Scope verification gate:** Integration test files in tests/ directory, no production code changes
