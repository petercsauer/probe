---
segment: 18
title: "Migrate Legacy Fixtures"
depends_on: [13]
risk: 2/10
complexity: Low
cycle_budget: 8
status: pending
commit_message: "chore: Remove unused root-level fixtures directory"
---

# Segment 18: Migrate Legacy Fixtures

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Remove root `fixtures/` directory (6 JSON files) if unused.

**Depends on:** Segment 13 (docs complete)

## Context: Issue 18 - Legacy Fixtures

**Core Problem:** Root-level `fixtures/` directory has 6 JSON files (recent dates) but grep shows zero Rust references. May be legacy or Python-only.

## Scope
- **Verify:** Check Python tests for references
- **Remove:** If unused, delete `fixtures/` directory

## Implementation Approach

```bash
# Verify no references
grep -r "fixtures/multi_transport" . --include="*.rs" --include="*.py"

# If no hits, remove
rm -rf fixtures/
```

## Build and Test Commands

**Build:** `cargo build --workspace`

**Test (targeted):** All tests pass without fixtures/

**Test (regression):** `cargo test --workspace`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:** No broken fixture references
2. **Regression tests:** All tests pass
3. **Full build gate:** Clean build
4. **Full test suite:** All tests pass
5. **Self-review:** Legacy fixtures removed if unused
6. **Scope verification:** Only fixtures/ directory affected
