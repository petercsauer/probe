---
segment: 4
title: "Capture trait seam refactoring"
depends_on: []
risk: 6
complexity: High
cycle_budget: 20
status: pending
commit_message: "refactor(prb-capture): introduce PacketSource trait, test capture_loop with fake source"
---

# Segment 4: Capture trait seam refactoring

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Introduce `PacketSource` and `PrivilegeChecker` traits in prb-capture to enable testing of `capture_loop`, `CaptureEngine::start/stop`, and `LiveCaptureAdapter` without pcap or root privileges. Push coverage from 47.6% to 80%+.

**Depends on:** None

## Issues Addressed

Issue 4 — prb-capture has no DI seams for pcap and privileges.

## Scope

- `crates/prb-capture/src/capture.rs` — trait extraction, `capture_loop` refactoring
- `crates/prb-capture/src/adapter.rs` — propagate trait bounds
- `crates/prb-capture/src/privileges.rs` — extract `PrivilegeChecker` trait
- New test files exercising the refactored paths

## Key Files and Context

**capture.rs current structure:**
- `CaptureEngine::start()` (line ~74): calls `pcap::Capture::from_device(interface).open()`, spawns thread running `capture_loop`
- `capture_loop` (line ~179): takes `pcap::Capture<Active>`, loops calling `cap.next_packet()`, sends `OwnedPacket` through channel, handles `TimeoutExpired`, tracks stats
- `CaptureEngine::stop()` (line ~127): sets stop flag, joins thread, returns stats
- `Drop for CaptureEngine` (line ~166): calls `stop()`

**Existing traits in the codebase (prior art):**
- `prb-detect/src/types.rs`: `pub trait ProtocolDetector: Send + Sync` — same pattern
- `prb-fixture/src/adapter.rs`: `pub trait CaptureAdapter` with `fn ingest(&mut self) -> Box<dyn Iterator<...>>` — the adapter-level trait

**`OwnedPacket` (capture.rs ~20-36):** Has `from_pcap(p: &pcap::Packet) -> Self` constructor that copies timestamp and data. The `PacketSource` trait should return `OwnedPacket` directly.

## Implementation Approach

1. Define traits in `capture.rs`:
   ```rust
   pub trait PacketSource: Send {
       fn next_packet(&mut self) -> Result<OwnedPacket, CaptureError>;
       fn stats(&self) -> Option<(u32, u32)>;
   }
   ```

2. Implement `PacketSource` for a new `PcapSource` struct that wraps `pcap::Capture<Active>`.

3. Make `capture_loop` generic: `fn capture_loop<S: PacketSource>(source: S, ...)`.

4. Define `PrivilegeChecker` trait in `privileges.rs`:
   ```rust
   pub trait PrivilegeChecker: Send + Sync {
       fn check(&self, interface: &str) -> Result<(), CaptureError>;
   }
   ```

5. Implement for `OsPrivilegeChecker` (current logic) and `NoOpPrivilegeChecker` (test double).

6. `CaptureEngine::start()` becomes generic or takes a boxed `PacketSource` factory.

7. Add test doubles:
   - `VecPacketSource`: yields packets from a `Vec<OwnedPacket>`, returns `TimeoutExpired`-equivalent after exhaustion
   - `AlwaysOkPrivileges`: returns `Ok(())` always

8. Write tests:
   - `test_capture_loop_processes_packets`: feed 5 packets via VecPacketSource, verify all received on channel
   - `test_capture_loop_channel_full_drop`: use a bounded(1) channel, send 10 packets, verify drop count
   - `test_capture_loop_stop_flag`: set stop flag after 2 packets, verify loop exits
   - `test_engine_already_running`: call start twice, expect AlreadyRunning error
   - `test_engine_stop_returns_stats`: start with VecPacketSource, stop, check stats
   - `test_engine_drop_calls_stop`: drop engine after start, no panic

## Alternatives Ruled Out

- `#[cfg(test)]` mock replacing pcap calls: doesn't compose, can't test real threading.
- `Box<dyn PacketSource>`: adds vtable overhead on hot path. Use generics with monomorphization.

## Pre-Mortem Risks

- Thread spawning with generics: `capture_loop::<S>` must be `Send` — verify `S: Send` bound suffices.
- `pcap::Capture<Active>` might not be `Send` — check. If not, `PcapSource` may need to open the capture inside the thread.
- Downstream crates (`prb-tui`, `prb-cli`) import `CaptureEngine` — ensure public API is backward-compatible or update callers.
- Performance: ensure the generic doesn't add overhead vs the current direct pcap call. Benchmark if uncertain.

## Build and Test Commands

- Build: `cargo build -p prb-capture`
- Test (targeted): `cargo test -p prb-capture -- capture_loop && cargo test -p prb-capture -- engine`
- Test (regression): `cargo test -p prb-capture`
- Test (full gate): `cargo test -p prb-capture -p prb-tui -p prb-cli`

## Exit Criteria

1. **Targeted tests:**
   - `test_capture_loop_processes_packets`: all packets received
   - `test_capture_loop_channel_full_drop`: drop count matches
   - `test_capture_loop_stop_flag`: loop exits cleanly
   - `test_engine_already_running`: returns AlreadyRunning
   - `test_engine_stop_returns_stats`: stats have correct packet count
   - `test_engine_drop_calls_stop`: no panic on drop
2. **Regression tests:** All pre-existing prb-capture tests pass, all prb-tui and prb-cli tests pass
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo test -p prb-capture -p prb-tui -p prb-cli`
5. **Self-review gate:** No dead code, no TODO hacks, traits are documented
6. **Scope verification gate:** Changes in `crates/prb-capture/src/` only (plus new test files). No changes to prb-tui or prb-cli unless import paths changed.

**Risk factor:** 6/10
**Estimated complexity:** High
