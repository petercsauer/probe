---
segment: 1
title: "Unit test gap sweep"
depends_on: []
risk: 2
complexity: Low
cycle_budget: 10
status: pending
commit_message: "test: add unit tests for stats inner, privileges, config, error variants, format detection"
---

# Segment 1: Unit test gap sweep

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Add pure unit tests for scattered untested code paths across 6 crates — no refactoring, no new dependencies.

**Depends on:** None

## Issues Addressed

Issue 1 — Scattered pure-logic unit test gaps across crates.

## Scope

- `crates/prb-capture/src/stats.rs` — `CaptureStatsInner::new` and `snapshot`
- `crates/prb-capture/src/privileges.rs` — `PrivilegeCheck::check` and `status` (macOS path)
- `crates/prb-capture/src/config.rs` — `with_snaplen`
- `crates/prb-capture/src/error.rs` — `CaptureError::Pcap` and `Other` Display
- `crates/prb-decode/src/schema_backed.rs` — edge case decode paths
- `crates/prb-dds/src/decoder.rs` — untested branches
- `crates/prb-pcap/src/pipeline_core.rs` — edge case branches

## Key Files and Context

**prb-capture/src/stats.rs:** `CaptureStatsInner` is `pub(crate)` with atomic counters. `snapshot(&self, start: Instant) -> CaptureStats` computes rates with a `max(0.001)` division guard. Tests go in `#[cfg(test)] mod tests` inline since the type is crate-private.

**prb-capture/src/privileges.rs:** On macOS/non-Linux, `check()` returns `Ok(())` and `status()` returns a static string. On Linux, it checks `caps::has_cap`. Add `#[cfg(not(target_os = "linux"))]` tests for the macOS path.

**prb-capture/src/config.rs:** `with_snaplen(u32) -> Self` is the only untested builder method.

**prb-capture/src/error.rs (inferred from tests):** `CaptureError::Pcap(pcap::Error)` via `#[from]` and `CaptureError::Other(String)` — test Display output.

## Implementation Approach

1. Add inline `#[cfg(test)]` tests in `stats.rs` for `CaptureStatsInner`.
2. Add tests in `stats_tests.rs` or inline for the snapshot division guard.
3. Add `test_privilege_check_macos` in `privileges.rs` or a new test file.
4. Add `test_config_with_snaplen` in `config_tests.rs`.
5. Add `test_capture_error_pcap` and `test_capture_error_other` in `error_tests.rs`.
6. For prb-decode: add edge-case tests for truncated/malformed proto in `schema_backed_tests.rs`.
7. For prb-dds: add tests for uncovered branches in decoder (check coverage report for specifics).
8. For prb-pcap: add edge-case tests for `pipeline_core.rs` uncovered branches.

## Alternatives Ruled Out

None — these are straightforward test additions.

## Pre-Mortem Risks

- `CaptureStatsInner` tests need `use std::sync::atomic::Ordering` and `std::time::Instant`.
- `pcap::Error` may not have a simple constructor — check if it has public variants or use `pcap::Error::MalformedError`.

## Build and Test Commands

- Build: `cargo build -p prb-capture -p prb-decode -p prb-dds -p prb-pcap`
- Test (targeted): `cargo test -p prb-capture -- stats && cargo test -p prb-capture -- privilege && cargo test -p prb-capture -- config && cargo test -p prb-capture -- error`
- Test (regression): `cargo test -p prb-capture && cargo test -p prb-decode && cargo test -p prb-dds && cargo test -p prb-pcap`
- Test (full gate): `cargo test --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_stats_inner_new_zeros`: all atomic fields start at 0
   - `test_stats_inner_snapshot`: set atomics, call snapshot, verify CaptureStats fields
   - `test_stats_inner_snapshot_division_guard`: sub-millisecond elapsed, no panic
   - `test_privilege_check_non_linux`: returns Ok on macOS
   - `test_privilege_status_non_linux`: returns non-empty string
   - `test_config_with_snaplen`: assert snaplen field set
   - `test_capture_error_pcap_display`: Pcap variant Display
   - `test_capture_error_other_display`: Other variant Display
2. **Regression tests:** All pre-existing tests in prb-capture, prb-decode, prb-dds, prb-pcap pass
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo test -p prb-capture -p prb-decode -p prb-dds -p prb-pcap`
5. **Self-review gate:** No dead code, no commented-out blocks, only test files changed
6. **Scope verification gate:** Only test files modified. No production code changes.

**Risk factor:** 2/10
**Estimated complexity:** Low
