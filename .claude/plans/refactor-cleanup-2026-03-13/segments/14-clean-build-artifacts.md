---
segment: 14
title: "Clean Build Artifacts"
depends_on: [13]
risk: 1/10
complexity: Low
cycle_budget: 8
status: pending
commit_message: "chore: Remove build artifacts and add .gitignore entries"
---

# Segment 14: Clean Build Artifacts

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Remove 65MB of committed build artifacts (test fixture target/ directory and 175 .profraw files), update .gitignore to prevent recurrence.

**Depends on:** Segment 13 (docs complete, safe to clean)

## Context: Issue 14 - Build Artifacts Cleanup

**Core Problem:** Test plugin build artifacts (65MB) and LLVM profiling files (175 .profraw files, 700KB) are cluttering the repository. Should be in .gitignore.

**Proposed Fix:**
1. Remove `crates/prb-plugin-native/tests/fixtures/target/`
2. Remove all `*.profraw` files
3. Add `*.profraw` and `*.profdata` to .gitignore

## Scope
- **Cleanup:** 65MB build artifacts, 175 profraw files
- **Files:** `.gitignore` (add patterns), `git rm -r` commands

## Implementation Approach

### Step 1: Remove fixture build artifacts
```bash
rm -rf crates/prb-plugin-native/tests/fixtures/target/
```

### Step 2: Remove profraw files
```bash
find . -name "*.profraw" -type f -delete
find . -name "*.profdata" -type f -delete
```

### Step 3: Update .gitignore
```gitignore
# After line 98 (existing coverage section)
*.profraw
*.profdata

# Confirm target/ is already ignored (should be on line 2)
```

### Step 4: Verify no other build artifacts
```bash
find . -name "Cargo.lock" -path "*/tests/fixtures/*"  # Keep these (dynamic libs)
find . -name "*.rlib" -o -name "*.rmeta"  # Should all be in target/
```

## Build and Test Commands

**Build:** `cargo build --workspace` (rebuilds test plugin, now in proper location)

**Test (targeted):**
```bash
cargo test --package prb-plugin-native --test adapter_integration_tests
```

**Test (regression):** `cargo test --workspace` (test plugin rebuilds automatically)

**Test (full gate):** All tests pass, artifacts regenerate in correct locations

## Exit Criteria

1. **Targeted tests:** Test plugin integration tests pass (rebuild happens automatically)
2. **Regression tests:** All workspace tests pass (artifact cleanup doesn't break anything)
3. **Full build gate:** Clean build succeeds, artifacts go to proper locations
4. **Full test suite:** All tests pass
5. **Self-review:** No build artifacts in git, .gitignore prevents recurrence
6. **Scope verification:** Only cleanup commands and .gitignore modified
