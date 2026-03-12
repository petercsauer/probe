---
id: "3"
title: "prb-tui app.rs key handlers and loader.rs format detection untested"
risk: 3/10
addressed_by_segments: [3]
---

# Issue 3: prb-tui app.rs key handlers and loader.rs format detection untested

## Core Problem

`app.rs` (70% coverage) has untested key handler branches: help toggle (`?`/Esc), BackTab reverse focus, filter-empty-clears, filter-parse-error, and `try_decode_event`/`wire_message_to_json` pure logic. `loader.rs` (58%) has untested `detect_format` magic-byte branches (MCAP `0x89 M C A`, pcapng `0x0a0d0d0a`, pcap BE) and `load_schemas` paths. `live.rs` (0%) has testable non-pcap paths (`stop()` flag, `take_receiver()` idempotency).

## Root Cause

`app.rs` tests use `test_handle_key`/`test_render_to_buffer` helpers extensively but missed several key-event branches. `loader.rs` tests only exercise JSON loading. `live.rs` was skipped entirely because `start()` requires pcap, but `stop()`/`take_receiver()` don't.

## Proposed Fix

Add tests using the existing `ratatui::backend::TestBackend` and `buf_helpers.rs` infrastructure. For `loader.rs`, write magic bytes to temp files via `tempfile`. For `live.rs`, test `stop()`/`take_receiver()` without calling `start()`.

## Existing Solutions Evaluated

N/A — internal test additions. TestBackend already in place.

## Pre-Mortem

- `try_decode_event` requires a `SchemaRegistry` — may need to construct one from a test `.proto` file.
- `wire_message_to_json` is private — test indirectly via `try_decode_event` or make `pub(crate)` for testing.

## Risk Factor: 3/10

Well-established test infrastructure, isolated changes.

## Blast Radius

- Direct: `crates/prb-tui/tests/` (new or extended test files)
- Ripple: None
