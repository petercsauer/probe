---
id: "4"
title: "prb-capture has no DI seams — capture.rs and adapter.rs untestable"
risk: 6/10
addressed_by_segments: [4]
---

# Issue 4: prb-capture has no DI seams — capture.rs and adapter.rs untestable

## Core Problem

`capture.rs` (12% coverage) and `adapter.rs` (36%) are tightly coupled to `pcap::Capture<Active>` and OS privileges. `CaptureEngine::start()` calls `pcap::Capture::from_device().open()` directly. `capture_loop` takes a concrete `pcap::Capture<Active>`. No trait abstracts over the packet source, making it impossible to test the capture logic, channel-full drop counting, timeout handling, or stop/drop semantics without root and a real NIC.

## Root Cause

The initial implementation was a spike that hardcoded pcap. No trait boundary was introduced because the crate started as a thin pcap wrapper.

## Proposed Fix

Introduce `PacketSource` and `PrivilegeChecker` traits:

```rust
pub trait PacketSource: Send {
    fn next_packet(&mut self) -> Result<OwnedPacket, CaptureError>;
    fn stats(&self) -> Option<(u32, u32)>; // (received, dropped)
}

pub trait PrivilegeChecker: Send + Sync {
    fn check(&self, interface: &str) -> Result<(), CaptureError>;
}
```

Refactor `capture_loop` to accept `impl PacketSource`. Add `VecPacketSource` and `AlwaysOkPrivileges` test doubles. This enables testing: channel-full drops, timeout handling, stop flag, stats snapshot, AlreadyRunning guard, thread panic handling, Drop impl.

## Existing Solutions Evaluated

N/A — internal refactoring. The trait seam pattern is standard Rust DI (used throughout this codebase in prb-detect's `ProtocolDetector` trait, prb-fixture's `CaptureAdapter` trait).

## Alternatives Considered

- `run_in_executor` wrapping: doesn't help testability, just moves blocking. Rejected.
- `#[cfg(test)]` mock module: brittle, doesn't compose. Rejected.

## Pre-Mortem

- Changing `capture_loop` signature is an internal-only change but touches the hot path. Ensure no performance regression by keeping the trait object behind a generic (monomorphized).
- `LiveCaptureAdapter` currently owns a `CaptureEngine` — the trait injection needs to thread through this ownership.
- `OwnedPacket::from_pcap` takes a `pcap::Packet` — the `PacketSource` trait should return `OwnedPacket` directly so tests don't need pcap types.

## Risk Factor: 6/10

Refactoring a core capture path. All existing tests must continue passing. The API surface of `prb-capture` is consumed by `prb-tui` and `prb-cli`.

## Blast Radius

- Direct: `crates/prb-capture/src/capture.rs`, `adapter.rs`, `privileges.rs`
- Ripple: `crates/prb-tui/src/live.rs`, `crates/prb-cli/src/commands/capture.rs` (import paths may change)
