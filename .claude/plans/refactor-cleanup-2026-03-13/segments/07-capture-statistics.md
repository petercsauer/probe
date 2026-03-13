---
segment: 7
title: "Implement Real Capture Statistics"
depends_on: [1]
risk: 4/10
complexity: Low
cycle_budget: 12
status: pending
commit_message: "fix(tui): Replace fake capture statistics with real pcap_stats"
---

# Segment 7: Implement Real Capture Statistics

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Replace hardcoded zero statistics in live capture with actual pcap_stats() values.

**Depends on:** Segment 1 (test utilities)

## Context: Issue 07 - Fake Capture Statistics

**Core Problem:** `crates/prb-tui/src/live.rs:127` returns fake statistics (all zeros). Users cannot see real packet drop counts.

**Evidence:**
```rust
// live.rs:127
CaptureStats {
    packets_received: 0,  // Hardcoded
    packets_dropped: 0,   // Hardcoded
    packets_if_dropped: 0, // Hardcoded
}
```

**Proposed Fix:** Call `pcap::Capture::stats()` to get real statistics:
```rust
let stats = capture.stats().map_err(|e| CaptureError::Other(e.to_string()))?;
CaptureStats {
    packets_received: stats.received as u64,
    packets_dropped: stats.dropped as u64,
    packets_if_dropped: stats.if_dropped as u64,
}
```

## Scope
- **Files:** `crates/prb-tui/src/live.rs` (line 127)

## Build and Test Commands

**Build:** `cargo build --package prb-tui`

**Test (targeted):** `cargo test --package prb-tui --lib live`

**Test (regression):** `cargo test --package prb-tui`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:** Stats reflect real pcap values
2. **Regression tests:** All TUI tests pass
3. **Full build gate:** Clean build
4. **Full test suite:** All tests pass
5. **Self-review:** No fake data, stats are accurate
6. **Scope verification:** Only live.rs modified
