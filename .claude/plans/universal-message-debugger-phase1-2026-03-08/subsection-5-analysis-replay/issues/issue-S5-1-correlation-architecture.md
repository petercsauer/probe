---
id: "S5-1"
title: "Correlation Engine Architecture and Flow State Model"
risk: 4/10
addressed_by_segments: [1]
---
# Issue S5-1: Correlation Engine Architecture and Flow State Model

**Core Problem:**
The parent plan defines a `CorrelationStrategy` trait in Subsection 1's core crate but does not specify how the correlation engine dispatches to strategies, iterates through stored events, or persists computed flows. Without this architecture, individual protocol strategies have no framework to plug into.

**Root Cause:**
The parent plan treats correlation as a collection of per-protocol strategies without defining the orchestrating engine that ties them together.

**Proposed Fix:**
Build a `CorrelationEngine` that: (1) reads events from MCAP storage via `MessageStream`, (2) identifies each event's transport type from `DebugEvent` metadata, (3) dispatches to the appropriate `CorrelationStrategy` implementation, (4) maintains a `FlowSet` mapping flow IDs to ordered event lists, (5) supports a generic fallback for unrecognized transports.

Flow state is computed on-demand for Phase 1. Precomputed flow indexes deferred to Phase 2 optimization. Rationale: on-demand computation is simpler, and MCAP's memory-mapped sequential read is fast enough for sessions up to 1M events.

API sketch:
```rust
pub struct CorrelationEngine {
    strategies: Vec<Box<dyn CorrelationStrategy>>,
    fallback: GenericCorrelationStrategy,
}

impl CorrelationEngine {
    pub fn correlate(&self, events: &[DebugEvent]) -> FlowSet {
        let mut flows = FlowSet::new();
        for event in events {
            let strategy = self.strategy_for(event);
            let flow_id = strategy.assign_flow(event, &mut flows);
            flows.add_event(flow_id, event);
        }
        flows
    }
}
```

**Existing Solutions Evaluated:**
- `retina` (Stanford, Rust, 100Gbps network analysis) -- provides flow tracking for live capture. Not usable for offline MCAP-based analysis. Rejected: live-only, heavyweight.
- No existing Rust library provides generic multi-protocol message correlation over stored events. Custom implementation required.

**Alternatives Considered:**
- Precompute flows during ingest and store in MCAP metadata. Rejected for Phase 1: adds complexity to the ingest pipeline and Subsections 1-3 are already built without flow-aware ingest. Suitable for Phase 2.
- Use SQLite for flow indexing. Rejected: adds a dependency and persistence format beyond MCAP. Overkill for Phase 1.

**Pre-Mortem -- What Could Go Wrong:**
- Memory: loading all events for correlation may fail for very large sessions. Mitigation: streaming correlation that processes events in MCAP order without full materialization.
- Dispatch overhead: checking each event against all strategies sequentially. Mitigation: O(1) dispatch via transport type enum match, not iteration.
- Flow explosion: if correlation keys are too fine-grained, every event becomes its own "flow." Need minimum flow size or grouping thresholds.

**Risk Factor:** 4/10

**Evidence for Optimality:**
- External evidence: Wireshark's "Follow Stream" feature uses this exact pattern -- dispatch to protocol-specific dissector, track conversation by protocol-specific key.
- External evidence: Zeek (formerly Bro) IDS uses per-protocol "analyzers" dispatched by transport type, the same architecture pattern.

**Blast Radius:**
- Direct: `prb-correlation` crate (new), `prb-cli` (new `flows` subcommand)
- Ripple: replay engine consumes flows for filtered replay
