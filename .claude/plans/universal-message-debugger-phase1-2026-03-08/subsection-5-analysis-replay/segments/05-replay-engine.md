---
segment: 5
title: "Replay Engine + prb replay"
depends_on: [1]
risk: 4/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(replay): add replay engine with timed output, filtering, and prb replay command"
---

# Segment 5: Replay Engine + prb replay

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Build the replay engine with timed event output, speed control, filtering, and the `prb replay` CLI command.

**Depends on:** Segment 1 (for flow/filter infrastructure and output formatting)

## Context: Issues Addressed

**S5-5: Replay Timing Accuracy Below 1ms**

- **Core Problem:** Tokio's timer driver has 1ms resolution. Events spaced <1ms apart (burst traffic) will replay with incorrect timing.
- **Proposed Fix:** Hybrid timing: (1) inter-event gaps >= 1ms: `tokio::time::sleep`; (2) gaps < 1ms: `Instant::now()` spin-wait with `std::hint::spin_loop()`; (3) `--speed max`: skip all timing.
- **Pre-Mortem risks:** Spin-wait on loaded system may overshoot; Windows timer resolution ~16ms (document as known limitation).

**S5-6: Replay Output Throughput and Formatting**

- **Core Problem:** Rust stdout is line-buffered; 100k events/sec produces 100k syscalls/sec, throttling replay. Research: BufWriter gives ~4.7x speedup.
- **Proposed Fix:** Wrap stdout in `BufWriter`; `--format json` (NDJSON) and `--format table` (tabled); register Ctrl+C handler to flush; detect piped output and suppress color.
- **Pre-Mortem risks:** tabled derive may conflict with serde (use separate `EventDisplay` type); BufWriter loses data on SIGKILL (SIGINT handler covers Ctrl+C).

**S5-7: MCAP Message Filtering for Replay**

- **Core Problem:** MCAP `MessageStream` iterates all messages; filtering must be application-level. For 1M events with 1% match, 99% scanned unnecessarily.
- **Proposed Fix:** (1) Read MCAP summary section, map channels to transport types, build `HashSet<u16>` allowlist; (2) skip messages with `channel_id` not in allowlist (O(1)); (3) `--start`/`--end` time-range filtering; (4) filter syntax: `transport=grpc`, `topic=/my/topic`, `flow=<flow_id>`.
- **Pre-Mortem risks:** Channel metadata schema from Subsection 2 may not encode transport type; MCAP files without summary require full scan.

## Scope

- `crates/prb-replay/src/lib.rs` -- replay engine crate
- `crates/prb-replay/src/timing.rs` -- hybrid timing (tokio sleep + spin-wait)
- `crates/prb-replay/src/filter.rs` -- MCAP message filtering with channel pre-filter
- `crates/prb-replay/src/output.rs` -- BufWriter-wrapped JSON and table output
- `crates/prb-cli/src/commands/replay.rs` -- `prb replay` command

## Key Files and Context

- `crates/prb-replay/src/timing.rs` -- Hybrid timing: gaps >= 1ms use `tokio::time::sleep`; gaps < 1ms use `Instant::now()` spin-wait with `std::hint::spin_loop()`; `--speed max` skips all timing. Tokio timer resolution is 1ms (tokio issue #970). Windows timer ~16ms; document as known limitation.
- `crates/prb-replay/src/filter.rs` -- Read MCAP summary for channel list; map channels to transport types; build `HashSet<u16>` allowlist; skip messages with `channel_id` not in allowlist; `--start`/`--end` time-range; filter syntax: `transport=grpc`, `topic=/my/topic`, `flow=<flow_id>`.
- `crates/prb-replay/src/output.rs` -- All output through `BufWriter<StdoutLock>`; `--format json`: NDJSON via `serde_json::to_writer()`; `--format table` (default): `tabled` with derive macros; Ctrl+C handler flushes BufWriter; detect piped output and suppress color.
- CLI: `prb replay session.mcap [--speed 2.0] [--filter 'transport=grpc'] [--format json|table] [--start <ts>] [--end <ts>]`
- `crates/prb-storage/src/lib.rs` -- MCAP read API from Subsection 2. Uses `mcap::MessageStream` over memory-mapped file.

## Implementation Approach

1. Create `prb-replay` crate.
2. `ReplayEngine` struct: takes MCAP session path, filter config, speed multiplier, output format.
3. Replay loop:
   a. Open MCAP file via `memmap2` + `mcap::MessageStream`
   b. If filter specified, read summary section, build channel allowlist
   c. Iterate messages. For each: check channel allowlist, check time range, deserialize to DebugEvent
   d. Compute delta from previous event timestamp. Adjust by speed multiplier.
   e. If delta >= 1ms: `tokio::time::sleep(delta)`. If delta < 1ms and > 0: spin-wait. If speed=max: skip.
   f. Format event and write to BufWriter.
   g. After all events or on Ctrl+C: flush BufWriter.
4. `prb replay` subcommand wires CLI args to ReplayEngine.
5. Add `tabled` to prb-replay dependencies for table output.

## Alternatives Ruled Out

- Always use spin-wait (100% CPU for long replays, unacceptable)
- Ignore sub-ms timing by rounding up to 1ms (distorts burst patterns)
- `tokio-timerfd` for sub-ms precision (Linux-only, macOS unsupported)
- `sturgeon` crate for replay (designed for live async streams, not MCAP file playback)
- Custom formatting without tabled (reinvents the wheel, error-prone alignment)
- Secondary index file for filtering (Phase 2 optimization, overkill for Phase 1)

## Pre-Mortem Risks

- Spin-wait on a loaded system overshoots due to OS scheduling. Inherent; document.
- BufWriter loses data on SIGKILL (not SIGINT). SIGINT handler covers Ctrl+C but not `kill -9`. Acceptable.
- Channel naming convention from Subsection 2 may not encode transport type. If so, transport-based filtering requires deserializing events (slower). Verify Subsection 2's channel schema.
- MCAP files without summary section require full scan. Handle gracefully with a progress indicator.
- `tabled` derive macros and `serde::Serialize` on DebugEvent: use a separate `EventDisplay` newtype if derive conflicts arise.

## Build and Test Commands

- Build: `cargo build -p prb-replay -p prb-cli`
- Test (targeted): `cargo nextest run -p prb-replay`
- Test (regression): `cargo nextest run -p prb-correlation -p prb-storage -p prb-cli`
- Test (full gate): `cargo nextest run --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_replay_preserves_event_order`: Events emitted in timestamp order matching MCAP source.
   - `test_replay_speed_max`: With `--speed max`, all events emitted without delays. Measures elapsed time < 100ms for 1000 events.
   - `test_replay_speed_multiplier`: With `--speed 2.0`, inter-event delay is halved (within 5ms tolerance for >= 10ms gaps).
   - `test_replay_filter_by_transport`: With `--filter transport=grpc`, only gRPC events appear in output.
   - `test_replay_filter_by_time_range`: With `--start` and `--end`, only events in range appear.
   - `test_replay_json_output`: `--format json` produces valid NDJSON, one JSON object per line, parseable by `serde_json::from_str`.
   - `test_replay_table_output`: `--format table` produces human-readable table with expected columns.
   - `test_replay_bufwriter_throughput`: Replay 10K events at max speed completes in < 1s (verifies BufWriter, not line buffering).
   - `test_replay_empty_session`: Empty MCAP file produces no output and exits cleanly.
   - `test_replay_ctrl_c_flushes`: Simulated signal triggers BufWriter flush (integration test).
2. **Regression tests:** All Segment 1 tests and existing workspace tests pass.
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo nextest run --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changed files within `crates/prb-replay/`, `crates/prb-cli/src/commands/replay.rs`, and `crates/prb-cli/Cargo.toml`. Out-of-scope changes documented.
