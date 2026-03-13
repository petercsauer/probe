---
segment: 9
title: "Remove Dead Code in Adapter"
depends_on: [1]
risk: 2/10
complexity: Low
cycle_budget: 8
status: pending
commit_message: "chore(capture): Remove unused set_promiscuous method"
---

# Segment 9: Remove Dead Code in Adapter

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Remove dead `set_promiscuous()` method that is never called.

**Depends on:** Segment 1 (test utilities)

## Context: Issue 09 - Dead Code

**Core Problem:** `set_promiscuous()` method exists but has zero callers (grep confirms).

## Scope
- **Files:** `crates/prb-capture/src/adapter.rs`

## Build and Test Commands

**Build:** `cargo build --package prb-capture`

**Test (targeted):** `cargo test --package prb-capture`

**Test (regression):** `cargo test --workspace`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:** All tests pass
2. **Regression tests:** No references to removed method
3. **Full build gate:** Clean build
4. **Full test suite:** All tests pass
5. **Self-review:** Dead code removed, no unused warnings
6. **Scope verification:** Only adapter.rs modified
