---
segment: 17
title: "Timeline Enhancements"
depends_on: [9]
risk: 4
complexity: Medium
cycle_budget: 5
status: pending
commit_message: "feat(prb-tui): interactive timeline with cursor navigation, multi-protocol sparklines, latency heatmap"
---

# Segment 17: Timeline Enhancements

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Transform the timeline from a passive sparkline into an interactive navigation tool with cursor, multi-protocol stacking, and latency heatmap mode.

**Depends on:** S09 (Conversation View — conversations needed for heatmap)

## Scope

- `crates/prb-tui/src/panes/timeline.rs` — Major rewrite of the timeline pane

## Implementation

### 17.1 Interactive Timeline

Make the sparkline navigable when focused:
- Left/Right arrows move a cursor across time buckets
- Enter jumps event list to events in that time bucket
- Show tooltip with bucket time range and event count
- Cursor rendered as a highlighted column in the sparkline

### 17.2 Multi-Protocol Sparklines

Show stacked per-protocol sparklines (one row per active protocol):

```
gRPC: ▁▂▃▅▇█▇▅▃▂▁  (green)
ZMQ:  ▁▁▂▃▂▁▁▂▃▅▇  (yellow)
TCP:  ▂▂▂▂▂▂▂▂▂▂▂  (blue)
      14:00 --- 14:05
```

Use `EventStore::protocol_counts()` per time bucket. Only show protocols that have events.

### 17.3 Latency Heatmap

Toggle with `L` when conversations are available:
- X-axis = time, Y-axis = latency buckets (0-10ms, 10-50ms, 50-100ms, >100ms)
- Color intensity = number of conversations in that bucket
- Use block characters with varying brightness/color

### 17.4 Time Range Selection

Click-drag (or Shift+Left/Right) to select a time range. This filters the event list to only events in that range. Show selection as highlighted region.

## Key Files and Context

- `crates/prb-tui/src/panes/timeline.rs` — Current TimelinePane
- `crates/prb-tui/src/event_store.rs` — `time_buckets()`, `protocol_counts()`

## Exit Criteria

1. **Cursor:** Left/Right moves cursor across timeline buckets
2. **Jump:** Enter jumps event list to cursor's time bucket
3. **Multi-protocol:** Stacked per-protocol sparklines when focused
4. **Heatmap:** `L` toggles latency heatmap when conversations available
5. **Tests pass:** `cargo nextest run -p prb-tui`
6. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
