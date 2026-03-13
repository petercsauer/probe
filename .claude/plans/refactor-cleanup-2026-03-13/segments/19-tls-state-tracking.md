---
segment: 19
title: "Improve TLS State Tracking"
depends_on: [14, 15, 16]
risk: 4/10
complexity: Medium
cycle_budget: 14
status: pending
commit_message: "feat(pcap): Improve TLS session state tracking and error messages"
---

# Segment 19: Improve TLS State Tracking

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Enhance TLS session tracking with better state management and error messages.

**Depends on:** Segments 14, 15, 16 (cleanup complete)

## Context: Issue 19 - TLS State Tracking Gaps

**Core Problem:** TLS decryption sometimes loses session state across handshakes. Better state tracking needed.

## Scope
- **Files:** `crates/prb-pcap/src/tls/decrypt.rs`

## Implementation Approach

Add session state enum and tracking:
```rust
enum TlsSessionState {
    Handshake,
    Established,
    Rekeying,
    Closed,
}

// Track state transitions
// Add better error messages for state mismatches
```

## Build and Test Commands

**Build:** `cargo build --package prb-pcap`

**Test (targeted):** `cargo test --package prb-pcap --lib tls`

**Test (regression):** `cargo test --package prb-pcap`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:** TLS state transitions tracked correctly
2. **Regression tests:** All TLS tests pass
3. **Full build gate:** Clean build
4. **Full test suite:** All tests pass
5. **Self-review:** State tracking complete, error messages clear
6. **Scope verification:** Only tls/decrypt.rs modified
