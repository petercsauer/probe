---
segment: 16
title: "Request Waterfall"
depends_on: [9]
risk: 5
complexity: Medium
cycle_budget: 7
status: pending
commit_message: "feat(prb-tui): request waterfall view — horizontal timing bars, latency breakdown"
---

# Segment 16: Request Waterfall

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Add a Chrome DevTools-style waterfall view showing request/response pairs as horizontal timing bars with latency breakdown.

**Depends on:** S09 (Conversation View — conversations with timing data)

## Scope

- `crates/prb-tui/src/panes/waterfall.rs` — **New file.** Waterfall pane
- `crates/prb-tui/src/app.rs` — `W` key to toggle waterfall view

## Implementation

### 16.1 Waterfall Visualization

Press `W` to toggle waterfall view (replaces event list temporarily):

```
Waterfall ──────────────────────────────────────────
/api.v1.Users/Get     ████████░░░░░░░░░░  45ms
/api.v1.Auth/Verify        ██████████████  92ms  ERR
/api.v1.Items/List              ████░░░░░  23ms
                      |---------|---------|--------->
                      0ms       50ms      100ms
```

Each conversation gets a horizontal bar:
- Bar start = first event timestamp (relative to earliest)
- Bar end = last event timestamp
- Solid section = request phase
- Lighter section = response wait
- Color = protocol color from theme
- Red = error conversations

### 16.2 Rendering

Compute time range from all conversations. Map each conversation's start/end to column positions. Use Unicode block characters for bars:

```rust
fn render_bar(start_col: u16, end_col: u16, is_error: bool) -> Vec<Span> {
    // Full block: █, half blocks for sub-cell precision
    // Error bars in red, normal in protocol color
}
```

### 16.3 Interaction

- j/k to select conversations in the waterfall
- Enter to jump to that conversation's events in the event list
- Scroll if more conversations than visible rows
- Show method/endpoint label on left, duration on right

### 16.4 Latency Breakdown

For selected conversation, show timing breakdown at the bottom:
- Time to first byte (TTFR)
- Full response time
- Request size / response size

Use `ConversationMetrics` from `prb-core`.

### 16.5 Time Scale

Show time axis at the bottom with adaptive scale (ms/s). Auto-scale based on total time range.

## Key Files and Context

- `crates/prb-core/src/conversation.rs` — `Conversation`, `ConversationMetrics`
- `crates/prb-core/src/engine.rs` — `ConversationSet::sorted_by_time()`

## Exit Criteria

1. **Waterfall view:** `W` toggles horizontal bar chart of conversations
2. **Timing bars:** Bars show relative timing with protocol colors
3. **Error marking:** Error conversations shown in red with ERR label
4. **Navigation:** j/k select, Enter jumps to event list
5. **Time axis:** Bottom shows adaptive time scale
6. **Tests pass:** `cargo nextest run -p prb-tui`
7. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
