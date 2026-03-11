---
id: "6"
title: "prb-cli command modules monolithic and partially untested"
risk: 5/10
addressed_by_segments: [6]
---

# Issue 6: prb-cli command modules monolithic and partially untested

## Core Problem

`tui.rs` (0%), `capture.rs` (17%), `inspect.rs` (61%), `ingest.rs` (64%), `plugins.rs` (59%), `schemas.rs` (66%) contain monolithic `run_*` functions that mix argument parsing, I/O, and terminal/capture operations. Key untested paths: `run_tui` data loading, `detect_format` magic bytes, `effective_jobs_with_env`, `format_grouped_by_trace` with trace IDs, `read_events_from_mcap`, `run_export` (schemas), `plugin_dir` env resolution, and `run_capture` event loop formatting.

## Root Cause

`run_*` functions do everything inline: load data, open terminal/capture device, run event loop, format output. The testable logic (format detection, data loading, output formatting) is interleaved with untestable I/O (terminal, pcap).

## Proposed Fix

1. Extract testable helpers: `detect_format(path) -> InputFormat` (make pub(crate)), `effective_jobs_with_env(cli_jobs) -> usize`, `build_tui_event_store(args) -> Result<EventStore>`, `plugin_dir(custom) -> PathBuf`.
2. Add tests for extracted helpers using `tempfile` fixtures.
3. Add MCAP-based tests for `inspect` and `schemas` (create MCAP programmatically with `prb_storage::SessionWriter`).
4. Test `format_grouped_by_trace` with events containing `otel.trace_id` metadata.

## Existing Solutions Evaluated

N/A — internal refactoring. Existing test patterns in `command_handlers_test.rs` provide the template.

## Pre-Mortem

- Extracting from `run_tui` may change function signatures — ensure `TuiArgs` fields are still consumed correctly.
- `effective_jobs_with_env` reads env vars — tests must use `std::env::set_var` with cleanup or `temp_env` crate.
- The `capture.rs` event loop is only fully testable after S4 lands (trait seams).

## Risk Factor: 5/10

Multiple files touched, function extraction changes internal structure.

## Blast Radius

- Direct: `crates/prb-cli/src/commands/*.rs`, `crates/prb-cli/tests/`
- Ripple: None (public API unchanged)
