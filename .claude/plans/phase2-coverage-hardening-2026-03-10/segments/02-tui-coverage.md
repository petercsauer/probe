---
segment: 2
title: "prb-tui Unit + Render Tests"
depends_on: []
risk: 5
complexity: High
cycle_budget: 8
status: pending
commit_message: "test(prb-tui): add unit and render tests for app, timeline, event_list, theme"
---

# Segment 2: prb-tui Unit + Render Tests

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Bring prb-tui from ~55% to ≥90% line coverage by adding tests for the 0% files (app.rs, live.rs, timeline.rs) and improving coverage on event_list.rs and theme.rs.

**Depends on:** None

## Coverage Gaps

| File | Lines | Current | Target | Deficit |
|------|-------|---------|--------|---------|
| prb-tui/src/app.rs | 561 | 0% | 90% | ~504 |
| prb-tui/src/panes/timeline.rs | 190 | 0% | 90% | ~171 |
| prb-tui/src/live.rs | 83 | 0% | 90% | ~74 |
| prb-tui/src/panes/event_list.rs | 876 | 66% | 90% | ~210 |
| prb-tui/src/theme.rs | 117 | 28% | 90% | ~72 |
| prb-tui/src/loader.rs | 129 | 84% | 90% | ~8 |
| prb-tui/src/event_store.rs | 258 | 89% | 90% | ~3 |
| prb-tui/src/panes/decode_tree.rs | 534 | 84% | 90% | ~32 |

## Scope

- `crates/prb-tui/src/app.rs` — TUI application loop, input handling, focus management
- `crates/prb-tui/src/panes/timeline.rs` — Sparkline pane, time buckets, protocol counts
- `crates/prb-tui/src/live.rs` — Live capture bridge thread
- `crates/prb-tui/src/panes/event_list.rs` — Event list pane rendering and navigation
- `crates/prb-tui/src/theme.rs` — Color theme definitions
- `crates/prb-tui/src/loader.rs`, `event_store.rs`, `panes/decode_tree.rs` — minor gaps

## Key Files and Context

- `crates/prb-tui/src/ring_buffer.rs` — Already at 100%, good reference for test patterns
- `crates/prb-tui/src/panes/hex_dump.rs` — Already at 93%, good reference for pane testing
- ratatui's `TestBackend` for rendering tests without a real terminal

## Implementation Approach

### app.rs (561 lines, 0%) — Highest priority
Use ratatui `TestBackend` to test rendering without a terminal:
- Test `App::new` initialization (focus state, filter state, input mode)
- Test `handle_key_event` for each key binding: Tab (focus cycle), `/` (filter mode), `Esc` (exit filter), `q` (quit), arrow keys
- Test filter application: type a filter string → verify EventStore filter applied
- Test help overlay toggle
- Test `draw` method with TestBackend → verify no panic, correct layout areas
- Factor out testable pure functions from the event loop if needed

### timeline.rs (190 lines, 0%)
- Mock EventStore with fixed timestamps spanning a known range
- Test `time_buckets` computation with events at known times
- Test `calculate_selected_bucket` returns correct index
- Test `format_time_legend` and `format_timestamp_short` formatting
- Test rendering with TestBackend (sparkline has expected widths)

### live.rs (83 lines, 0%)
- Create a mock `LiveCaptureAdapter` that yields a fixed set of packets then stops
- Test that events are forwarded through the channel
- Test stop flag terminates the thread
- Test stats reporting

### event_list.rs (876 lines, 66% → 90%)
- Test column width computation edge cases
- Test sorting by each column (timestamp, protocol, src, dst, summary)
- Test selection wrapping at boundaries
- Test filter interaction with display indices
- Test rendering with very long summary strings (truncation)

### theme.rs (117 lines, 28% → 90%)
- Test every `protocol_color` mapping returns expected Style
- Test `severity_color` for each severity level
- Test `pane_border_style` for focused/unfocused states

## Alternatives Ruled Out

- Don't use `crossterm` event simulation — test `handle_key_event` directly with constructed `KeyEvent` values
- Don't try to test the actual terminal event loop — factor logic into testable methods

## Pre-Mortem Risks

- app.rs has a tight coupling between event loop and rendering — may need to extract handlers into separate testable functions
- ratatui TestBackend may not capture all widget details — focus on behavior tests, not pixel-perfect rendering

## Build and Test Commands

- Build: `cargo check -p prb-tui`
- Test (targeted): `cargo nextest run -p prb-tui`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** `cargo nextest run -p prb-tui` — all new tests pass
2. **Coverage gate:** app.rs ≥85%, timeline.rs ≥90%, live.rs ≥90%, event_list.rs ≥90%, theme.rs ≥90%
3. **Regression tests:** `cargo nextest run --workspace` — no regressions
4. **Full build gate:** `cargo build --workspace`
5. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
6. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
7. **Scope verification gate:** Only prb-tui test and source files modified
