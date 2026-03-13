---
segment: 16
title: "Archive Inactive Worktrees"
depends_on: [13]
risk: 3/10
complexity: Low
cycle_budget: 10
status: pending
commit_message: "chore: Archive inactive worktree pools"
---

# Segment 16: Archive Inactive Worktrees

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Remove 19.7GB of inactive git worktrees if no longer needed.

**Depends on:** Segment 13 (docs complete)

## Context: Issue 16 - Inactive Worktrees

**Core Problem:** `.claude/worktrees/pool-00` through `pool-03` total 19.7GB. May be stale from previous orchestration runs.

## Scope
- **Check:** `git worktree list` to see if active
- **Remove:** If inactive, delete worktree directories

## Implementation Approach

```bash
# Check if worktrees are active
git worktree list

# If none listed or all orphaned:
rm -rf .claude/worktrees/pool-*
```

## Build and Test Commands

**Build:** `cargo build --workspace`

**Test (targeted):** `git worktree list` shows no orphans

**Test (regression):** `cargo test --workspace`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:** No orphaned worktrees
2. **Regression tests:** All tests pass
3. **Full build gate:** Clean build
4. **Full test suite:** All tests pass
5. **Self-review:** Disk space reclaimed
6. **Scope verification:** Only .claude/worktrees/ affected
