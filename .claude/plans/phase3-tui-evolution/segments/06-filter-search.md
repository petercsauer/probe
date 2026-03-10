---
segment: 6
title: "Filter & Search UX"
depends_on: [1, 2]
risk: 4
complexity: Medium
cycle_budget: 7
status: pending
commit_message: "feat(prb-tui): incremental live filter, filter history, quick filters, syntax highlighting"
---

# Segment 6: Filter & Search UX

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Transform the filter bar from basic (type, press Enter) into a power-user tool with live preview, history, quick-filter shortcuts, and syntax highlighting.

**Depends on:** S01 (Visual Polish — theme), S02 (Column Layout — data display for quick filters)

## Current State

- Filter bar activates on `/`, user types a `prb-query` expression, Enter applies, Esc cancels
- Filter is all-or-nothing — no preview until Enter
- No filter history
- No shortcut to filter by selected event's properties
- Filter text is plain white, no syntax highlighting

## Scope

- `crates/prb-tui/src/app.rs` — Filter mode handling, quick-filter dispatch
- `crates/prb-tui/src/filter_state.rs` — **New file.** Filter history, incremental state
- `crates/prb-tui/src/panes/event_list.rs` — Quick-filter key handlers

## Implementation

### 6.1 Incremental Live Filtering

As the user types in the filter bar, parse and apply the filter incrementally. Show the filtered count updating in real-time:

```
 Filter: transport == "gRPC"  [2/4 events]
```

Debounce at 100ms. After the user stops typing, try `prb_query::parse_filter(&text)`. If valid, compute `preview_count` by filtering indices.

```rust
struct FilterState {
    text: String,
    last_change: Instant,
    preview_filter: Option<Filter>,
    preview_count: Option<usize>,
    committed_filter: Option<Filter>,
    history: Vec<String>,
    history_cursor: Option<usize>,
}
```

On each keystroke, update `text` and `last_change`. On next tick (if >100ms elapsed), try parsing. Show count inline. Enter commits, Esc reverts to `committed_filter`.

### 6.2 Filter History

Store recent filter expressions (up to 50). Up/Down arrows in filter mode cycle through history. Optionally persist to `~/.config/prb/history.json`.

### 6.3 Quick Filters from Context

When an event is selected, pressing `f` enters quick-filter prefix mode:

- `f` + `s` → `source == "<selected_source>"`
- `f` + `d` → `dest == "<selected_dest>"`
- `f` + `p` → `transport == "<selected_protocol>"`
- `f` + `c` → filter by conversation key

Auto-populates filter bar and applies immediately.

### 6.4 Syntax Highlighting in Filter Bar

Color the filter expression as the user types:
- Field names (`transport`, `source`): cyan
- Operators (`==`, `!=`, `contains`): yellow
- String values: green
- Numbers: magenta
- Invalid segments: red underline

Parse with `prb-query` tokenizer and render with styled spans.

### 6.5 Clear Filter Indicator

When filter is active, show `[filtered]` in status bar with Esc hint.

## Key Files and Context

- `crates/prb-tui/src/app.rs` — `handle_filter_key()`, `InputMode::Filter`
- `crates/prb-query/src/lib.rs` — `parse_filter()` function
- `crates/prb-tui/src/event_store.rs` — `filter_indices()`
- `crates/prb-core/src/event.rs` — `DebugEvent` fields for quick-filter generation

## Build and Test Commands

- Build: `cargo check -p prb-tui`
- Test (targeted): `cargo nextest run -p prb-tui`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Live preview:** Filtered count updates as user types (debounced)
2. **History:** Up/Down arrows cycle through past filter expressions
3. **Quick filters:** `f+s`, `f+d`, `f+p` apply contextual filters from selected event
4. **Syntax highlighting:** Filter bar shows colored field names, operators, values
5. **Clear indicator:** Active filter shown in status bar with clear hint
6. **Targeted tests:** New tests for FilterState pass
7. **Regression:** `cargo nextest run --workspace` — no regressions
8. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
