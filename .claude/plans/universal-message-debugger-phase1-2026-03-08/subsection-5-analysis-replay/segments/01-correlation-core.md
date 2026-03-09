---
segment: 1
title: "Correlation Engine Core + Generic Fallback + prb flows"
depends_on: []
risk: 4/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(correlation): add correlation engine with generic fallback and prb flows command"
---

# Segment 1: Correlation Engine Core + Generic Fallback + prb flows

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Establish the correlation engine architecture with generic fallback and a working `prb flows` CLI command that displays flow-grouped events.

**Depends on:** None (within this subsection). Requires Subsections 1-4 complete (DebugEvent type, MCAP storage, protocol decoders).

## Context: Issues Addressed

**S5-1: Correlation Engine Architecture and Flow State Model**

- **Core Problem:** The parent plan defines a `CorrelationStrategy` trait but does not specify how the correlation engine dispatches to strategies, iterates through stored events, or persists computed flows. Without this architecture, individual protocol strategies have no framework to plug into.
- **Proposed Fix:** Build a `CorrelationEngine` that: (1) reads events from MCAP storage via `MessageStream`, (2) identifies each event's transport type from `DebugEvent` metadata, (3) dispatches to the appropriate `CorrelationStrategy` implementation, (4) maintains a `FlowSet` mapping flow IDs to ordered event lists, (5) supports a generic fallback for unrecognized transports. Flow state is computed on-demand for Phase 1.
- **Pre-Mortem risks:** Memory pressure with very large sessions; dispatch overhead if checking each event against all strategies sequentially (use O(1) transport enum match); flow explosion if correlation keys are too fine-grained.

## Scope

- `prb-correlation` crate: `CorrelationEngine`, `FlowSet`, `Flow`, `GenericCorrelationStrategy`
- `prb-cli` crate: `prb flows` subcommand
- Shared output formatting infrastructure (tabled + JSON)

## Key Files and Context

- `crates/prb-correlation/src/lib.rs` -- crate root, re-exports
- `crates/prb-correlation/src/engine.rs` -- `CorrelationEngine` struct with strategy dispatch
- `crates/prb-correlation/src/flow.rs` -- `FlowSet`, `Flow`, `FlowId` types
- `crates/prb-correlation/src/generic.rs` -- `GenericCorrelationStrategy` (IP-tuple + timestamp proximity)
- `crates/prb-cli/src/commands/flows.rs` -- `prb flows` command implementation
- `crates/prb-core/src/traits.rs` -- `CorrelationStrategy` trait (defined in Subsection 1, consumed here)
- `crates/prb-core/src/event.rs` -- `DebugEvent` type (defined in Subsection 1)
- `crates/prb-storage/src/lib.rs` -- MCAP read API (defined in Subsection 2)

The `CorrelationStrategy` trait from prb-core:
```rust
pub trait CorrelationStrategy: Send + Sync {
    fn name(&self) -> &str;
    fn matches(&self, event: &DebugEvent) -> bool;
    fn correlation_key(&self, event: &DebugEvent) -> Option<CorrelationKey>;
}
```

The `CorrelationEngine` dispatches to the first matching strategy for each event. `GenericCorrelationStrategy` uses `(src_addr, dst_addr, timestamp_bucket)` as the correlation key, where `timestamp_bucket` groups events within a configurable window (default 100ms).

A `Flow` contains: `id: FlowId`, `transport: TransportKind`, `correlation_key: CorrelationKey`, `event_count: usize`, `first_timestamp: Timestamp`, `last_timestamp: Timestamp`, `metadata: HashMap<String, String>` (protocol-specific info like method name, topic).

`FlowSet` stores flows in a `BTreeMap<FlowId, Flow>` for ordered iteration by first timestamp.

## Implementation Approach

1. Create `prb-correlation` crate with engine, flow types, and generic strategy.
2. Engine reads DebugEvents from MCAP via storage API, iterates in timestamp order, assigns each event to a flow via strategy dispatch.
3. `prb flows` subcommand: loads MCAP session, runs correlation engine, displays flows in table format.
4. Output infrastructure: shared formatting module with `format_table()` and `format_json()` using `tabled` and `serde_json`.
5. Add `tabled` to `prb-cli` dependencies.

## Alternatives Ruled Out

- Precomputing flows during ingest (adds complexity to Subsections 1-3 which are already built)
- SQLite-based flow index (heavyweight dependency for Phase 1)
- Single global strategy without dispatch (fails for multi-protocol sessions)

## Pre-Mortem Risks

- Memory pressure with very large sessions. Test with 1M events to verify.
- Generic strategy timestamp bucket may be too coarse (groups unrelated events) or too fine (one event per flow). Make configurable.
- DebugEvent may not carry all needed metadata fields. Verify against Subsection 4 output.

## Build and Test Commands

- Build: `cargo build -p prb-correlation -p prb-cli`
- Test (targeted): `cargo nextest run -p prb-correlation`
- Test (regression): `cargo nextest run -p prb-core -p prb-storage -p prb-cli`
- Test (full gate): `cargo nextest run --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_engine_dispatches_to_matching_strategy`: CorrelationEngine with two mock strategies correctly dispatches based on transport type.
   - `test_generic_strategy_groups_by_ip_tuple`: Events with same src/dst addr grouped; different addrs produce separate flows.
   - `test_generic_strategy_timestamp_bucket`: Events >100ms apart (same addrs) produce separate flows.
   - `test_flow_set_ordering`: Flows returned sorted by first_timestamp.
   - `test_prb_flows_json_output`: `prb flows session.mcap --format json` produces valid NDJSON with expected fields.
   - `test_prb_flows_table_output`: `prb flows session.mcap` produces formatted table with columns: Flow ID, Transport, Events, Duration, Key.
2. **Regression tests:** All existing workspace tests pass (`cargo nextest run --workspace`).
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo nextest run --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changed files within `crates/prb-correlation/`, `crates/prb-cli/src/commands/flows.rs`, and `crates/prb-cli/Cargo.toml`. Out-of-scope changes documented.
