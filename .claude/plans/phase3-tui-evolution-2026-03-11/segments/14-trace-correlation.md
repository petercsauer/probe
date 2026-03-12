---
segment: 14
title: "Trace Correlation View"
depends_on: [9]
risk: 5
complexity: Medium
cycle_budget: 7
status: pending
commit_message: "feat(prb-tui): trace correlation view — group by OTel trace/span ID, distributed trace tree"
---

# Segment 14: Trace Correlation View

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Wire `prb-core`'s trace context extraction into the TUI, enabling events to be grouped by OpenTelemetry trace/span ID in a distributed trace tree view.

**Depends on:** S09 (Conversation View — view switching pattern, event grouping)

## Current State

- `prb-core` has `extract_trace_context(event)` returning `Option<TraceContext>`
- Parsers exist for W3C traceparent, B3 single/multi, Jaeger uber-trace-id
- CLI has `prb inspect --group-by-trace` that does this grouping
- `prb-export` has `merge_traces_with_packets()` for OTLP+packet correlation
- None of this is in the TUI

## Scope

- `crates/prb-tui/src/panes/trace_view.rs` — **New file.** Trace tree view
- `crates/prb-tui/src/app.rs` — `t` key toggle, trace extraction on load
- `crates/prb-cli/src/commands/tui.rs` — `--otlp` flag for OTLP trace merge

## Implementation

### 14.1 Extract Trace Context on Load

After loading events, extract trace contexts:

```rust
fn extract_traces(events: &[DebugEvent]) -> HashMap<String, Vec<(usize, TraceContext)>> {
    let mut traces: HashMap<String, Vec<_>> = HashMap::new();
    for (idx, event) in events.iter().enumerate() {
        if let Some(ctx) = extract_trace_context(event) {
            traces.entry(ctx.trace_id.clone()).or_default().push((idx, ctx));
        }
    }
    traces
}
```

### 14.2 Trace Tree View

Press `t` to toggle trace correlation mode. Events with matching trace IDs grouped into a tree:

```
Trace: abc123def456
 ├─ Span: /api.v1.Users/Get (50ms)
 │  ├─ Event #1: gRPC request  → 10.0.0.2:8080
 │  └─ Event #4: gRPC response ← 10.0.0.2:8080
 └─ Span: /api.v1.Auth/Verify (30ms)
    ├─ Event #2: gRPC request  → 10.0.0.3:8080
    └─ Event #3: gRPC response ← 10.0.0.3:8080
```

Use `tui-tree-widget` for the trace tree. Selecting a node in the tree selects the corresponding event.

### 14.3 OTLP Trace Merge

Add `--otlp` flag to `prb tui`:
```bash
prb tui capture.pcap --otlp traces.json
```

Use `prb_export::merge_traces_with_packets()` to correlate application-level spans with wire-level packets. Show unified view with both span metadata and packet details.

### 14.4 Trace List

When multiple traces exist, show a trace list:

| Trace ID | Spans | Events | Duration | Status |
|----------|-------|--------|----------|--------|
| abc123.. | 3 | 8 | 120ms | OK |
| def456.. | 2 | 4 | 45ms | ERROR |

Select a trace to see its span tree.

## Key Files and Context

- `crates/prb-core/src/trace.rs` — `TraceContext`, `extract_trace_context()`, parsers
- `crates/prb-export/src/merge.rs` — `merge_traces_with_packets()`, `MergedEvent`
- `crates/prb-export/src/otlp_import.rs` — `parse_otlp_json()`

## Exit Criteria

1. **Trace extraction:** Trace contexts extracted from events on load
2. **Trace tree:** `t` toggles trace view with span tree
3. **Event linking:** Selecting span tree node selects corresponding event
4. **OTLP merge:** `--otlp` flag loads and merges OTLP traces with packets
5. **Tests pass:** `cargo nextest run -p prb-tui`
6. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
