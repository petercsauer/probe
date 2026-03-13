---
segment: 8
title: "Implement Linktype Detection"
depends_on: [1]
risk: 5/10
complexity: Medium
cycle_budget: 14
status: pending
commit_message: "fix(capture): Detect actual linktype instead of hardcoding Ethernet"
---

# Segment 8: Implement Linktype Detection

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Replace hardcoded `linktype=1` (Ethernet) with actual linktype detection from pcap handle.

**Depends on:** Segment 1 (test utilities)

## Context: Issue 08 - Hardcoded Linktype

**Core Problem:** `crates/prb-capture/src/adapter.rs:96` hardcodes linktype=1, assuming all captures are Ethernet. Breaks on WiFi, loopback, raw IP captures.

**Evidence:**
```rust
// adapter.rs:96
linktype: 1,  // Hardcoded Ethernet (DLT_EN10MB)
```

**Proposed Fix:**
```rust
let linktype = capture.get_datalink().0;
```

## Scope
- **Files:** `crates/prb-capture/src/adapter.rs` (line 96)

## Build and Test Commands

**Build:** `cargo build --package prb-capture`

**Test (targeted):** `cargo test --package prb-capture`

**Test (regression):** `cargo test --workspace`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:** Linktype matches actual capture
2. **Regression tests:** All capture tests pass
3. **Full build gate:** Clean build
4. **Full test suite:** All tests pass
5. **Self-review:** No hardcoded constants
6. **Scope verification:** Only adapter.rs modified
