---
segment: 2
title: "Column Layout & Smart Display"
depends_on: []
risk: 3
complexity: Medium
cycle_budget: 5
status: pending
commit_message: "feat(prb-tui): adaptive column layout with smart source/dest fallback"
---

# Segment 2: Column Layout & Smart Display

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Make the event list show useful source/destination info in all cases, and adaptively collapse empty columns to maximize screen real estate.

**Depends on:** None

## Current State

- Source column shows `event.source.network.src` or `"-"`
- Destination shows `event.source.network.dst` or `"-"`
- Fixture files lack network address data, so all show `"-"`
- Column widths are fixed regardless of content
- `pad_to_width()` handles Unicode correctly (Phase 0 fix)

## Scope

- `fixtures/multi_transport.json` — Add network addresses
- `fixtures/grpc_sample.json` — Add network addresses
- `fixtures/sample.json` — Add network addresses
- `crates/prb-tui/src/panes/event_list.rs` — Smart fallback display, adaptive column widths

## Implementation

### 2.1 Update Fixture Files

Add realistic network addresses to all fixture JSON files:

```json
{
  "transport": "grpc",
  "source": {
    "origin": "pcap-file",
    "network": { "src": "10.0.0.1:50051", "dst": "10.0.0.2:8080" }
  }
}
```

Verify the JSON schema matches the `EventSource` deserialization in `prb-core`.

### 2.2 Smart Fallback Display

When `event.source.network` is `None`, display `event.source.origin` (the adapter name like "pcap-file", "fixture", "live:en0") instead of bare "-". In event_list.rs:

```rust
fn format_source(event: &DebugEvent) -> String {
    event.source.network.as_ref()
        .map(|n| n.src.to_string())
        .unwrap_or_else(|| {
            event.source.origin.clone()
        })
}

fn format_dest(event: &DebugEvent) -> String {
    event.source.network.as_ref()
        .map(|n| n.dst.to_string())
        .unwrap_or_else(|| String::from("-"))
}
```

### 2.3 Adaptive Column Layout

Detect at render time which columns have meaningful data. If ALL visible rows lack network info, collapse Source/Dest and give the space to Summary:

```rust
fn compute_column_widths(events: &[&DebugEvent], total_width: u16) -> ColumnWidths {
    let has_network = events.iter().any(|e| e.source.network.is_some());

    if has_network {
        // Full layout: #(6) Time(12) Src(18) Dst(18) Proto(10) Dir(3) Summary(fill)
        ColumnWidths { id: 6, time: 12, src: 18, dst: 18, proto: 10, dir: 3, summary: fill }
    } else {
        // Collapsed: #(6) Time(12) Origin(20) Proto(10) Dir(3) Summary(fill)
        ColumnWidths { id: 6, time: 12, src: 20, dst: 0, proto: 10, dir: 3, summary: fill }
    }
}
```

Use `Constraint::Min` + `Constraint::Fill` instead of fixed widths for the Summary column. Store the column config in `EventListPane` and recompute when the visible window changes.

### 2.4 Header Adaptation

The header row should also adapt to show "Origin" instead of "Source" / "Destination" when network info is absent.

## Key Files and Context

- `crates/prb-core/src/event.rs` — `EventSource { origin: String, network: Option<NetworkAddr> }`, `NetworkAddr { src, dst }`
- `crates/prb-tui/src/panes/event_list.rs` — Column rendering, `format_header()`, `render()` method
- `crates/prb-fixture/src/lib.rs` — `JsonFixtureAdapter` that reads fixture JSON files
- `fixtures/` — JSON fixture files

## Build and Test Commands

- Build: `cargo check -p prb-tui`
- Test (targeted): `cargo nextest run -p prb-tui && cargo nextest run -p prb-fixture`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Fixture files:** All fixture JSON files have realistic network addresses that deserialize correctly
2. **Targeted tests:** `cargo nextest run -p prb-tui -p prb-fixture` — all pass
3. **Smart fallback:** When network is None, Source shows origin name instead of "-"
4. **Adaptive columns:** When no events have network info, Dest column collapses, Summary gets extra width
5. **Header adapts:** Shows "Origin" instead of "Source"/"Dest" when appropriate
6. **Regression tests:** `cargo nextest run --workspace` — no regressions
7. **Full build gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
