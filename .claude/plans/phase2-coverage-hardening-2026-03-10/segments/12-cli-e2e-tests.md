---
segment: 12
title: "CLI End-to-End Tests"
depends_on: [3, 11]
risk: 4
complexity: Medium
cycle_budget: 4
status: pending
commit_message: "test(prb-cli): add end-to-end CLI tests with assert_cmd"
---

# Segment 12: CLI End-to-End Tests

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Add end-to-end CLI tests that invoke the `prb` binary and verify output, closing remaining coverage gaps in CLI command dispatch and main.rs.

**Depends on:** Segments 3 (CLI unit tests), 11 (integration tests)

## Scope

- `tests/cli_e2e.rs` — New test file using `assert_cmd` crate
- Covers: `prb ingest`, `prb inspect`, `prb export`, `prb schemas`, `prb merge`
- Does NOT cover: `prb capture` (needs root), `prb tui` (needs terminal), `prb explain` (needs API key)

## Key Files and Context

- `crates/prb-cli/src/main.rs` — Entry point (67% → 90%)
- `crates/prb-cli/src/cli.rs` — Clap CLI definition
- `tests/fixtures/` — Existing test fixtures

## Implementation Approach

### Use `assert_cmd` for binary testing
Add `assert_cmd` and `predicates` to dev-dependencies:

```rust
// tests/cli_e2e.rs
use assert_cmd::Command;
use predicates::prelude::*;
```

### Test scenarios

**prb ingest:**
- `prb ingest tests/fixtures/sample.pcapng -o /tmp/test.mcap` → exit 0, output file exists
- `prb ingest nonexistent.pcap` → exit non-zero, stderr contains error
- `prb ingest --help` → shows usage

**prb inspect:**
- `prb inspect /tmp/test.mcap` → exit 0, output contains event data
- `prb inspect /tmp/test.mcap --format json` → valid JSON output
- `prb inspect /tmp/test.mcap --format table` → table-formatted output
- `prb inspect /tmp/test.mcap --where "proto == grpc"` → filtered output
- `prb inspect /tmp/test.mcap --limit 5` → at most 5 events

**prb export:**
- `prb export /tmp/test.mcap --format csv -o /tmp/out.csv` → valid CSV
- `prb export /tmp/test.mcap --format har -o /tmp/out.har` → valid HAR JSON
- `prb export /tmp/test.mcap --format html -o /tmp/out.html` → contains HTML tags

**prb schemas:**
- `prb schemas list` → exit 0
- `prb schemas load tests/fixtures/test.proto` → exit 0 (if proto fixture exists)

**prb merge:**
- Create two MCAP files, merge them → output contains events from both

**prb (no args):**
- Shows help text

**prb --version:**
- Shows version string

### Test isolation
- Use tempdir for all output files
- Create fixture MCAP in a setup function shared across tests
- Tests should be independent and parallelizable

## Alternatives Ruled Out

- Don't use `trycmd` (snapshot testing) — too brittle for evolving output formats
- Don't test `prb capture` or `prb tui` in e2e — they need special environments

## Build and Test Commands

- Build: `cargo build -p prb-cli`
- Test (targeted): `cargo nextest run --test cli_e2e`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** `cargo nextest run --test cli_e2e` — at least 12 e2e tests pass
2. **Coverage gate:** prb-cli/src/main.rs ≥ 90%, overall CLI coverage improvement
3. **Regression tests:** `cargo nextest run --workspace` — no regressions
4. **Full build gate:** `cargo build --workspace`
5. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
6. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
7. **Scope verification gate:** Only test files and Cargo.toml dev-dependencies modified
