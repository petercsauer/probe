---
segment: 3
title: "prb-cli Command Tests"
depends_on: []
risk: 4
complexity: Medium
cycle_budget: 6
status: pending
commit_message: "test(prb-cli): add unit tests for all CLI command handlers"
---

# Segment 3: prb-cli Command Tests

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Bring prb-cli from ~40% to ≥90% line coverage by testing all command handler modules.

**Depends on:** None

## Coverage Gaps

| File | Lines | Current | Target | Deficit |
|------|-------|---------|--------|---------|
| prb-cli/src/commands/plugins.rs | 347 | 0% | 90% | ~312 |
| prb-cli/src/commands/capture.rs | 177 | 20% | 90% | ~124 |
| prb-cli/src/commands/inspect.rs | 291 | 38% | 90% | ~150 |
| prb-cli/src/commands/ingest.rs | 377 | 56% | 90% | ~128 |
| prb-cli/src/commands/export.rs | 299 | 57% | 90% | ~99 |
| prb-cli/src/commands/merge.rs | 215 | 55% | 90% | ~74 |
| prb-cli/src/commands/schemas.rs | 150 | 57% | 90% | ~50 |
| prb-cli/src/commands/tui.rs | 19 | 0% | 90% | ~17 |
| prb-cli/src/main.rs | 42 | 67% | 90% | ~9 |

## Scope

- `crates/prb-cli/src/commands/*.rs` — All command handler modules
- `crates/prb-cli/src/main.rs` — CLI entry point

## Key Files and Context

- `crates/prb-cli/src/cli.rs` — Clap CLI definition (already tested via `verify_cli()`)
- `crates/prb-cli/src/output.rs` — Output formatting (90% covered)
- `tests/fixtures/` — Existing test fixture files (pcap, proto, json)

## Implementation Approach

Test each command handler with real temp directories and fixture files:

### plugins.rs (347 lines, 0%)
- Create temp plugin directory structure with manifest files
- Test `list_plugins` with empty dir, one plugin, multiple plugins
- Test `plugin_info` with valid/invalid plugin names
- Test `install_plugin` copies files correctly
- Test `remove_plugin` deletes directory
- Test error paths: missing dir, corrupt manifest, missing plugin

### inspect.rs (291 lines, 38%)
- Use existing test MCAP fixtures
- Test table output formatting, JSON output, summary mode
- Test `--where` filter application
- Test `--limit` and `--offset` pagination
- Test error paths: missing file, invalid format

### ingest.rs (377 lines, 56%)
- Test PCAP ingestion with fixture files → temp MCAP output
- Test JSON ingestion
- Test `--tls-keylog` option
- Test `--proto` schema loading during ingest
- Test error paths: missing file, unsupported format

### export.rs (299 lines, 57%)
- Test each export format: CSV, HAR, HTML, OTLP
- Use temp MCAP with known events → verify output structure
- Test `--format` flag parsing
- Test `--output` to temp file

### merge.rs, schemas.rs, capture.rs, tui.rs
- merge: test merge of two MCAP files into one
- schemas: test load/list with proto fixtures
- capture: test argument validation (interface, filter, duration)
- tui: test that the command struct constructs correctly

## Alternatives Ruled Out

- Don't test via actual CLI binary invocation for unit tests — call the handler functions directly
- Don't mock the filesystem — use real temp directories (more reliable)

## Pre-Mortem Risks

- Command handlers may have side effects (file I/O) — use `tempdir` for isolation
- Some commands require MCAP files with specific content — create minimal fixtures in tests

## Build and Test Commands

- Build: `cargo check -p prb-cli`
- Test (targeted): `cargo nextest run -p prb-cli`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** `cargo nextest run -p prb-cli` — all new tests pass
2. **Coverage gate:** Every file in commands/ ≥ 85% line coverage
3. **Regression tests:** `cargo nextest run --workspace` — no regressions
4. **Full build gate:** `cargo build --workspace`
5. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
6. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
7. **Scope verification gate:** Only prb-cli test and source files modified
