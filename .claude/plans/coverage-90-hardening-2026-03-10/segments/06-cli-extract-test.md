---
segment: 6
title: "CLI command extraction + tests"
depends_on: [4]
risk: 5
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "test(prb-cli): extract testable helpers, add format detection, inspect, ingest, plugins, schemas tests"
---

# Segment 6: CLI command extraction + tests

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Push prb-cli from 62.9% to 85%+ by extracting testable helpers from monolithic command functions and adding targeted tests.

**Depends on:** Segment 4 (capture trait seams enable capture.rs testing)

## Issues Addressed

Issue 6 — prb-cli command modules monolithic and partially untested.

## Scope

- `crates/prb-cli/src/commands/tui.rs` — extract data loading into testable function
- `crates/prb-cli/src/commands/capture.rs` — test list_interfaces, format capture output
- `crates/prb-cli/src/commands/inspect.rs` — MCAP input, trace grouping, decoded payload
- `crates/prb-cli/src/commands/ingest.rs` — detect_format magic bytes, effective_jobs_with_env
- `crates/prb-cli/src/commands/plugins.rs` — plugin_dir env resolution
- `crates/prb-cli/src/commands/schemas.rs` — run_export with MCAP fixture

## Key Files and Context

**Existing test infrastructure:** `tests/command_handlers_test.rs` has 82 tests using helper functions like `create_debug_events_json`, `create_debug_events_ndjson`, `create_test_mcap`. These create temp files and call `run_*` functions directly.

**ingest.rs `detect_format` (~lines 22-51):** Reads first 4 bytes, matches magic:
- `[0x0a, 0x0d, 0x0d, 0x0a]` → Pcapng
- `[0xd4, 0xc3, 0xb2, 0xa1]` or `[0xa1, 0xb2, 0xc3, 0xd4]` → Pcap
- `[0x89, b'M', b'C', b'A']` → Mcap (if this branch exists)
- `b'{' | b'['` → Json
- Falls back to extension. Currently `fn detect_format` is private.

**ingest.rs `effective_jobs_with_env` (~lines 264-279):** `if cli_jobs != 0 { cli_jobs } else if let Ok(v) = env::var("PRB_JOBS") { v.parse()... } else { available_parallelism()... }`. Currently private.

**inspect.rs `format_grouped_by_trace` (~lines 100-188):** Groups events by `metadata["otel.trace_id"]`, formats trace spans with timing. Tested for no-trace events but not for events with actual trace IDs.

**plugins.rs `plugin_dir` (~lines 14-26):** Checks `PRB_PLUGIN_DIR` env, then `dirs::home_dir() / ".prb/plugins"`, then fallback `.prb/plugins`.

## Implementation Approach

1. Make `detect_format` and `effective_jobs_with_env` `pub(crate)` in ingest.rs.
2. Extract `build_tui_event_store(args) -> Result<EventStore>` from `run_tui` (optional — may not be needed if we can test enough indirectly).
3. Add tests in `command_handlers_test.rs`:
   - `test_detect_format_pcapng_magic`: write `[0x0a, 0x0d, 0x0d, 0x0a, ...]` to temp file
   - `test_detect_format_pcap_le_magic`: write `[0xd4, 0xc3, 0xb2, 0xa1, ...]`
   - `test_detect_format_json_brace`: write `{"events": []}` to temp file
   - `test_detect_format_extension_fallback`: write random bytes with `.pcap` extension
   - `test_detect_format_unknown_error`: random bytes, `.xyz` extension
   - `test_effective_jobs_from_env`: set `PRB_JOBS=3`, call with `0`, assert 3
   - `test_effective_jobs_explicit`: call with `8`, assert 8
4. Add inspect tests:
   - `test_inspect_mcap_input`: create MCAP with `SessionWriter`, run inspect
   - `test_inspect_group_by_trace_with_ids`: NDJSON with `otel.trace_id` metadata
   - `test_inspect_wire_format_decoded_payload`: NDJSON with decoded payload variant
5. Add schemas test:
   - `test_schemas_export_with_descriptor`: create MCAP with embedded schema, call `run_export`
   - `test_schemas_export_no_schemas_error`: MCAP without schemas, expect error
6. Add plugins test:
   - `test_plugin_dir_from_env`: set `PRB_PLUGIN_DIR`, verify
   - `test_plugin_dir_default`: unset env, verify default path

## Alternatives Ruled Out

- Testing `run_tui` end-to-end: requires terminal, not feasible.
- Testing `run_capture` event loop: requires S4 trait seams. The `list_interfaces` path is already tested.

## Pre-Mortem Risks

- `effective_jobs_with_env` reads env vars — tests must clean up with `std::env::remove_var`.
- `plugin_dir` reads `dirs::home_dir()` which varies by machine — assert on path suffix not absolute path.
- Creating MCAP fixtures with `SessionWriter` — ensure `prb-storage` is in dev-deps of prb-cli.

## Build and Test Commands

- Build: `cargo build -p prb-cli`
- Test (targeted): `cargo test -p prb-cli -- detect_format && cargo test -p prb-cli -- effective_jobs && cargo test -p prb-cli -- mcap && cargo test -p prb-cli -- plugin_dir`
- Test (regression): `cargo test -p prb-cli`
- Test (full gate): `cargo test -p prb-cli`

## Exit Criteria

1. **Targeted tests:**
   - Format detection: pcapng, pcap LE, JSON, extension fallback, unknown error
   - Jobs: env override, explicit value
   - Inspect: MCAP input, trace grouping with IDs
   - Schemas: export success, export no-schemas error
   - Plugins: plugin_dir from env, default
2. **Regression tests:** All 82+ existing command_handlers tests pass, all CLI e2e and integration tests pass
3. **Full build gate:** `cargo build -p prb-cli`
4. **Full test gate:** `cargo test -p prb-cli`
5. **Self-review gate:** No dead code, minimal visibility changes (pub(crate) only)
6. **Scope verification gate:** Changes in `crates/prb-cli/` only

**Risk factor:** 5/10
**Estimated complexity:** Medium
