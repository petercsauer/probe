---
segment: 1
title: "prb-core Unit Tests"
depends_on: []
risk: 3
complexity: Medium
cycle_budget: 5
status: pending
commit_message: "test(prb-core): add unit tests for engine, metrics, and conversation modules"
---

# Segment 1: prb-core Unit Tests

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Bring prb-core from ~30% to ≥90% line coverage by adding unit tests for the three 0% files and improving coverage on existing files.

**Depends on:** None

## Coverage Gaps

| File | Lines | Current | Target | Deficit |
|------|-------|---------|--------|---------|
| prb-core/src/engine.rs | 392 | 0% | 90% | ~352 |
| prb-core/src/metrics.rs | 242 | 0% | 90% | ~217 |
| prb-core/src/conversation.rs | 88 | 0% | 90% | ~79 |

## Scope

- `crates/prb-core/src/engine.rs` — ConversationEngine, reconstruction logic
- `crates/prb-core/src/metrics.rs` — ConversationMetrics computation, percentiles, error extraction
- `crates/prb-core/src/conversation.rs` — Conversation types, ConversationId, Display impls

## Key Files and Context

- `crates/prb-core/src/event.rs` — DebugEvent type (already well-tested, use as fixture reference)
- `crates/prb-core/src/trace.rs` — TraceContext (already well-tested)
- `crates/prb-grpc/src/correlation.rs` — GrpcCorrelationStrategy (use as mock reference)
- `crates/prb-zmq/src/correlation.rs` — ZmqCorrelationStrategy
- `crates/prb-dds/src/correlation.rs` — DdsCorrelationStrategy

## Implementation Approach

### conversation.rs (88 lines, 0%)
Pure data types — no mocks needed:
- Test `ConversationId::new` and `Display` impl
- Test `Conversation::new`, `add_event`, `add_metadata`
- Test serde round-trips for all types
- Test `ConversationKind` and `ConversationState` Display impls

### metrics.rs (242 lines, 0%)
Stateless computation — use synthetic DebugEvent data:
- Test `compute_metrics` with events that have known timestamps → verify duration, ttfr
- Test `extract_error` for each protocol: gRPC status codes, RST_STREAM, DDS sequence gaps, timeouts
- Test `check_dds_sequence_gaps` with sequential and gap-containing sequences
- Test `percentile` with known data sets
- Test `compute_aggregate_metrics` across multiple conversations

### engine.rs (392 lines, 0%)
Needs mock CorrelationStrategy implementations:
- Create a `MockStrategy` that returns known `CorrelationFlow` entries
- Test `ConversationEngine::new` with registered strategies
- Test `reconstruct` with events that match mock flows
- Test fallback grouping for unclaimed events (grouped by address)
- Test `ConversationSet` methods: `get`, `iter`, `by_protocol`, `event_index`
- Test empty input, single event, multiple protocols

## Alternatives Ruled Out

- Don't test via the CLI or integration harness — keep these as pure unit tests
- Don't mock DebugEvent — construct real instances with synthetic data

## Pre-Mortem Risks

- engine.rs depends on protocol correlation strategies — mock the trait, don't import real decoders
- metrics.rs error extraction checks protocol-specific metadata keys — verify key names match real decoders

## Build and Test Commands

- Build: `cargo check -p prb-core`
- Test (targeted): `cargo nextest run -p prb-core`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** `cargo nextest run -p prb-core` — all new tests pass
2. **Coverage gate:** Each of engine.rs, metrics.rs, conversation.rs ≥ 90% line coverage
3. **Regression tests:** `cargo nextest run --workspace` — no regressions
4. **Full build gate:** `cargo build --workspace`
5. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
6. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
7. **Scope verification gate:** Only test files and prb-core source modified
