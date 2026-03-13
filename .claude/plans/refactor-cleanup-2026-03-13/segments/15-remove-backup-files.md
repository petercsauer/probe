---
segment: 15
title: "Remove Backup Files"
depends_on: [13]
risk: 1/10
complexity: Low
cycle_budget: 6
status: pending
commit_message: "chore: Remove .bak files and add to .gitignore"
---

# Segment 15: Remove Backup Files

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Remove 18 .bak files committed to git, add `*.bak` to .gitignore.

**Depends on:** Segment 13 (docs complete, safe to clean)

## Context: Issue 15 - Backup Files in Git

**Core Problem:** 18 .bak files (400KB) tracked in git. Editor backups should not be committed.

## Scope
- **Delete:** 18 .bak files in crates/prb-tui, crates/prb-ai, tests/
- **Modify:** .gitignore

## Implementation Approach

```bash
# Remove all .bak files
find . -name "*.bak" -type f -delete

# Add to .gitignore
echo "*.bak" >> .gitignore
echo "*.orig" >> .gitignore
```

## Build and Test Commands

**Build:** `cargo build --workspace`

**Test (targeted):** `git status` shows .bak files removed

**Test (regression):** `cargo test --workspace`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:** No .bak files in git
2. **Regression tests:** All tests pass
3. **Full build gate:** Clean build
4. **Full test suite:** All tests pass
5. **Self-review:** .gitignore updated
6. **Scope verification:** Only cleanup and .gitignore modified
