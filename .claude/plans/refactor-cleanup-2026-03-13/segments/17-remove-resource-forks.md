---
segment: 17
title: "Remove macOS Resource Forks"
depends_on: [13]
risk: 1/10
complexity: Low
cycle_budget: 6
status: pending
commit_message: "chore: Remove macOS resource fork files"
---

# Segment 17: Remove macOS Resource Forks

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Remove 18 `._*` AppleDouble files (macOS metadata).

**Depends on:** Segment 13 (docs complete)

## Context: Issue 17 - macOS Resource Forks

**Core Problem:** 18 files like `._tcp`, `._tls12.pcap` (72KB total) are macOS resource forks, should not be in git.

## Scope
- **Delete:** All `._*` files in tests/fixtures/captures/
- **Update:** .gitignore already has `._*` rule

## Implementation Approach

```bash
find tests/fixtures/captures -name "._*" -type f -delete
```

## Build and Test Commands

**Build:** `cargo build --workspace`

**Test (targeted):** `find . -name "._*"` returns nothing

**Test (regression):** `cargo test --workspace`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:** No `._*` files remain
2. **Regression tests:** All tests pass
3. **Full build gate:** Clean build
4. **Full test suite:** All tests pass
5. **Self-review:** macOS metadata removed
6. **Scope verification:** Only tests/fixtures/ affected
