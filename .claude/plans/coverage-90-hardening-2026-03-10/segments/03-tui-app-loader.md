---
segment: 3
title: "TUI app key handlers + loader"
depends_on: []
risk: 3
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "test(prb-tui): add key handler, format detection, loader, and live.rs tests"
---

# Segment 3: TUI app key handlers + loader

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Push prb-tui from 85.1% to 92%+ by testing untested app.rs key handlers, loader.rs format detection, and live.rs non-capture paths.

**Depends on:** None

## Issues Addressed

Issue 3 — prb-tui app.rs key handlers and loader.rs format detection untested.

## Scope

- `crates/prb-tui/src/app.rs` — untested key handler branches, `try_decode_event`, `wire_message_to_json`
- `crates/prb-tui/src/loader.rs` — `detect_format` magic bytes, `load_pcap`, `load_mcap`
- `crates/prb-tui/src/live.rs` — `stop()`, `take_receiver()`

## Key Files and Context

**app.rs test infrastructure:** `pub fn test_handle_key(&mut self, key: KeyEvent) -> bool` and `pub fn test_render_to_buffer(&mut self, width: u16, height: u16) -> Buffer` are exposed as `#[doc(hidden)]` test hooks. Tests use `ratatui::backend::TestBackend`. Helper file `tests/buf_helpers.rs` provides buffer assertion utilities.

**app.rs untested branches:**
- `KeyCode::Char('?')` → sets `input_mode = InputMode::Help`
- `Esc` when `input_mode == Help` → returns to Normal
- `BackTab` → `focused_pane = focused_pane.prev()`
- Filter mode: Enter with empty text clears filter; Enter with invalid syntax sets `filter_error`
- `Esc` when a filter is active → clears filter, restores `all_indices`
- `try_decode_event()` with and without `SchemaRegistry`
- `wire_message_to_json()` for various `WireValue` variants

**loader.rs `detect_format`:** Reads first 4 bytes. Magic bytes: MCAP `[0x89, b'M', b'C', b'A']`, pcapng `[0x0a, 0x0d, 0x0d, 0x0a]`, pcap LE `[0xd4, 0xc3, 0xb2, 0xa1]`, pcap BE `[0xa1, 0xb2, 0xc3, 0xd4]`, JSON `{` or `[`. Falls back to extension.

**live.rs:** `LiveDataSource` has `stop()` (sets atomic flag) and `take_receiver()` (takes Option). Both work without calling `start()`.

## Implementation Approach

1. **app.rs key handlers:** In `tests/app_comprehensive_test.rs` or new file:
   - `test_help_toggle`: send `Char('?')`, assert help mode; send `Esc`, assert normal
   - `test_backtab_reverse_cycle`: send BackTab 4 times, verify pane cycle
   - `test_filter_empty_clears`: enter filter mode, press Enter with empty text, verify no filter
   - `test_filter_invalid_shows_error`: type invalid filter, press Enter, assert `filter_error.is_some()`
   - `test_esc_clears_active_filter`: apply valid filter, press Esc, verify indices restored

2. **loader.rs magic bytes:** In `tests/loader_coverage_test.rs` or similar:
   - Write magic bytes to temp files, call `detect_format`, assert variants
   - Empty file with `.json` extension → JSON
   - Unknown bytes + unknown extension → error

3. **live.rs:** In `tests/live_test.rs`:
   - `test_stop_flag_without_start`: construct `LiveDataSource` (check if possible without adapter), call `stop()` 
   - `test_take_receiver_idempotent`: call twice, second returns None
   - `CaptureState` and `AppEvent` variant construction + Debug/Clone

4. **app.rs pure logic:** Test `try_decode_event` and `wire_message_to_json` if accessible, or increase coverage via integration render tests.

## Alternatives Ruled Out

- Testing `App::run()` directly: requires real terminal, not feasible.
- Using `crossterm::event::push_event`: not available in test mode.

## Pre-Mortem Risks

- `LiveDataSource::new()` may require a `LiveCaptureAdapter` which needs pcap config. If so, test only `CaptureState`/`AppEvent` types and defer live.rs to S4.
- `wire_message_to_json` is likely private — may need `pub(crate)` visibility or test indirectly.
- Filter clearing logic depends on `all_indices` being populated — need events in the store.

## Build and Test Commands

- Build: `cargo build -p prb-tui`
- Test (targeted): `cargo test -p prb-tui -- help_toggle && cargo test -p prb-tui -- backtab && cargo test -p prb-tui -- detect_format && cargo test -p prb-tui -- live`
- Test (regression): `cargo test -p prb-tui`
- Test (full gate): `cargo test -p prb-tui`

## Exit Criteria

1. **Targeted tests:**
   - Key handler: help toggle, backtab cycle, filter clear, filter error
   - Loader: detect_format for MCAP, pcapng, pcap LE, pcap BE, JSON, unknown
   - Live: stop flag, take_receiver idempotency
2. **Regression tests:** All 55 existing prb-tui unit tests + all integration tests pass
3. **Full build gate:** `cargo build -p prb-tui`
4. **Full test gate:** `cargo test -p prb-tui`
5. **Self-review gate:** No dead code, minimal production changes (only visibility if needed)
6. **Scope verification gate:** Changes in `crates/prb-tui/` only

**Risk factor:** 3/10
**Estimated complexity:** Medium
