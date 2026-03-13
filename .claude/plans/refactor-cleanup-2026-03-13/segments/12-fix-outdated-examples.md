---
segment: 12
title: "Fix Outdated API Examples"
depends_on: [10, 11]
risk: 3/10
complexity: Low
cycle_budget: 10
status: pending
commit_message: "docs: Fix outdated API examples in crate READMEs"
---

# Segment 12: Fix Outdated API Examples

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Fix 7 outdated API examples in crate READMEs to match current APIs.

**Depends on:** Segments 10, 11 (docs complete, APIs stable)

## Context: Issue 12 - Outdated Examples

**Core Problem:** READMEs show wrong method names (`from_path` vs `new`, `events()` vs `ingest()`), wrong field names (`source_addr` vs `source.network`).

**Files to fix:**
- `crates/prb-pcap/README.md:25-34` - Wrong constructor
- `crates/prb-fixture/README.md:21-24` - Wrong methods
- `crates/prb-storage/README.md:28-31` - Wrong API
- `crates/prb-core/README.md:39-46` - Wrong fields

## Scope
- **Files:** 4 README.md files

## Build and Test Commands

**Build:** N/A (documentation only)

**Test (targeted):** Manually verify examples compile

**Test (regression):** `cargo test --workspace`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:** Examples in READMEs match actual APIs
2. **Regression tests:** All tests pass
3. **Full build gate:** Clean build
4. **Full test suite:** All tests pass
5. **Self-review:** No outdated API references
6. **Scope verification:** Only README files modified
