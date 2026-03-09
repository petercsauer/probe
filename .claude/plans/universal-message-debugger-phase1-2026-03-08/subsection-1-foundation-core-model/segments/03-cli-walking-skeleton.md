---
segment: 3
title: "CLI Skeleton + Walking Skeleton Integration"
depends_on: [2]
risk: 3/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(cli): add ingest/inspect commands with fixture pipeline"
---

# Segment 3: CLI Skeleton + Walking Skeleton Integration

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Build the `prb` CLI binary with `ingest` and `inspect` subcommands, wire the fixture adapter end-to-end, and validate the walking skeleton with integration tests.

**Depends on:** Segment 2

## Context: Issues Addressed

**Integration of all prior issues:** This segment validates the full architecture. S1-1 (DebugEvent): CLI reads/writes DebugEvents as NDJSON. S1-2 (sync traits): JsonFixtureAdapter.ingest() is called synchronously. S1-3 (workspace): prb-cli crate. S1-4 (fixture format): ingest reads fixture files. S1-5 (error convention): CLI uses anyhow::Result; CoreError and FixtureError convert via Into. Pre-mortem: integration tests may be flaky if binary not built (assert_cmd cargo_bin() handles this); table formatting may break with long values (truncate with configurable max width); insta snapshots sensitive to formatting (use named snapshots); NDJSON will be replaced by MCAP in Subsection 2 (design ingest signature for backward-compatible switch).

## Scope

- `crates/prb-cli/` (full CLI implementation)
- Workspace-level integration tests

## Key Files and Context

After Segment 2, the following exist:
- `crates/prb-core/` -- DebugEvent, all traits (CaptureAdapter etc.), CoreError, supporting types
- `crates/prb-fixture/` -- JsonFixtureAdapter implementing CaptureAdapter, FixtureError
- `fixtures/*.json` -- test fixtures

Files to create/modify:
- `crates/prb-cli/Cargo.toml` -- add deps: prb-core, prb-fixture, clap, anyhow, tracing, tracing-subscriber, serde_json, camino
- `crates/prb-cli/src/main.rs` -- entry point with tracing setup
- `crates/prb-cli/src/cli.rs` -- clap derive structs (Cli, Commands enum)
- `crates/prb-cli/src/commands/mod.rs` -- command module
- `crates/prb-cli/src/commands/ingest.rs` -- `prb ingest <file>` command
- `crates/prb-cli/src/commands/inspect.rs` -- `prb inspect <file>` command (reads ingested events)
- `crates/prb-cli/src/output.rs` -- output formatting (table + JSON modes)
- `crates/prb-cli/tests/integration.rs` -- CLI integration tests

CLI structure:
```
prb ingest <fixture.json> [--output <session.json>]
    Reads a JSON fixture file, converts to DebugEvents, writes to stdout (or file).
    Output is newline-delimited JSON (one DebugEvent per line).

prb inspect [<session.json>] [--format table|json] [--filter <transport>]
    Reads DebugEvents from stdin or file, displays in human-readable format.
    Default format: table (compact, one line per event).
    JSON format: pretty-printed full events.
```

For Subsection 1, the "session" is a simple NDJSON file of serialized DebugEvents. MCAP storage replaces this in Subsection 2. Design command signatures for backward-compatible switch.

Output table format example:
```
TIMESTAMP           TRANSPORT  DIR   SOURCE         METADATA
2024-03-08T12:00:00 grpc       IN    10.0.0.1:8080  grpc.method=/svc/Method
2024-03-08T12:00:01 raw_tcp    OUT   10.0.0.2:443   -
```

tracing-subscriber: EnvFilter from RUST_LOG, default level warn, compact format to stderr (so it doesn't interfere with stdout data pipeline). CLI uses anyhow::Result for all command handlers.

## Implementation Approach

1. Update prb-cli Cargo.toml with all dependencies
2. Create `cli.rs` with clap derive structs:
   ```rust
   #[derive(Parser)]
   #[command(name = "prb", about = "Universal message debugger")]
   struct Cli {
       #[command(subcommand)]
       command: Commands,
       #[arg(long, default_value = "warn")]
       log_level: String,
   }

   #[derive(Subcommand)]
   enum Commands {
       Ingest(IngestArgs),
       Inspect(InspectArgs),
   }
   ```
3. Implement `ingest` command: instantiate JsonFixtureAdapter, call `ingest()`, write each DebugEvent as JSON to stdout or file
4. Implement `inspect` command: read NDJSON from stdin or file, format and display
5. Implement output formatting in `output.rs`: table formatter (fixed-width columns) and JSON formatter (pretty-print)
6. Set up tracing in `main.rs`
7. Write integration tests using `assert_cmd` and `insta`: pipe test, file test, error cases (nonexistent file, malformed JSON, invalid format flag)

## Alternatives Ruled Out

- MCAP as intermediate format in Subsection 1: adds heavy dependency before storage crate exists. NDJSON is simple and validates the pipeline.
- TUI/interactive mode: out of scope per parent plan's non-goals.
- Colored output by default: can interfere with piping. Defer --color flag if not needed.

## Pre-Mortem Risks

- Integration tests that pipe `prb ingest | prb inspect` may be flaky if binary not built. Use `cargo_bin()` which builds the binary; `cargo nextest run` handles this.
- Table formatting may break with long field values. Truncate with `...` at configurable max column width.
- Snapshot tests (insta) sensitive to formatting changes. Use named snapshots for clear diffs.
- NDJSON intermediate format will be replaced by MCAP in Subsection 2. Design ingest signature for backward-compatible switch.

## Build and Test Commands

- Build: `cargo build --workspace`
- Test (targeted): `cargo nextest run -p prb-cli`
- Test (regression): `cargo nextest run -p prb-core -p prb-fixture`
- Test (full gate): `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings && cargo fmt --all -- --check`

## Exit Criteria

1. **Targeted tests:**
   - `test_cli_ingest_fixture_to_stdout`: `prb ingest fixtures/sample.json` exits 0, stdout contains valid NDJSON DebugEvents
   - `test_cli_ingest_fixture_to_file`: `prb ingest fixtures/sample.json --output /tmp/test.json` creates file with valid content
   - `test_cli_inspect_from_stdin`: pipe NDJSON to `prb inspect`, verify table output matches snapshot
   - `test_cli_inspect_from_file`: `prb inspect /tmp/test.json --format table`, verify snapshot
   - `test_cli_inspect_json_format`: `prb inspect /tmp/test.json --format json`, verify pretty-printed JSON
   - `test_cli_ingest_nonexistent_file`: `prb ingest nonexistent.json` exits non-zero with helpful error message
   - `test_cli_ingest_malformed`: `prb ingest fixtures/malformed.json` exits non-zero with parse error
   - `test_cli_inspect_filter_transport`: `prb inspect --filter grpc` shows only gRPC events
   - `test_cli_help`: `prb --help` exits 0, output contains "Universal message debugger"
   - `test_cli_pipe_end_to_end`: `prb ingest fixtures/grpc_sample.json | prb inspect --format table` produces expected table output (insta snapshot)
   - `test_cli_version`: `prb --version` exits 0
2. **Regression tests:** All Segment 1 and 2 tests pass (`cargo nextest run -p prb-core -p prb-fixture`)
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo nextest run --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changed files are limited to: `Cargo.toml` (if workspace members change), `Cargo.lock`, `crates/prb-cli/**`. No changes to prb-core or prb-fixture source (only their tests may be referenced).
