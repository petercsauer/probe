---
segment: 13
title: "Setup Documentation Tooling"
depends_on: [12]
risk: 2/10
complexity: Low
cycle_budget: 12
status: pending
commit_message: "ci(docs): Add cargo-rdme, lychee, and doc-coverage tooling"
---

# Segment 13: Setup Documentation Tooling

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Add automated documentation tooling (cargo-rdme for README sync, lychee for link checking, cargo-doc-coverage for metrics) to prevent future doc rot.

**Depends on:** Segment 12 (examples fixed, docs current)

## Context: Issue 13 - Documentation Tooling

**Core Problem:** READMEs manually maintained (drift risk), no link checking, no doc coverage metrics. Documentation can become outdated without detection.

**Proposed Fix:**
1. Add cargo-rdme workflow (lib.rs → README.md sync)
2. Add lychee for broken link detection
3. Add cargo-doc-coverage for metrics
4. Add CI checks to enforce

## Scope
- **Files:** `.github/workflows/docs.yml`, `justfile`, `.lycheeignore`

## Implementation Approach

### Step 1: Add cargo-rdme command to justfile
```makefile
# Generate READMEs from lib.rs
readme-sync:
    cargo install cargo-rdme --locked
    cd crates/prb-core && cargo rdme
    cd crates/prb-grpc && cargo rdme
    # ...repeat for all crates

# Check if READMEs are in sync (CI)
readme-check:
    cargo install cargo-rdme --locked
    cargo rdme --check --workspace-project prb-core
    cargo rdme --check --workspace-project prb-grpc
    # ...repeat for all crates
```

### Step 2: Add lychee CI check
```yaml
# .github/workflows/docs.yml
- name: Check for broken links
  uses: lycheeverse/lychee-action@v2
  with:
    args: '--exclude-path target/ --exclude-path .git/ .'
```

### Step 3: Add doc coverage check
```yaml
# .github/workflows/docs.yml
- name: Check doc coverage
  run: |
    cargo install cargo-doc-coverage --locked
    cargo doc-coverage --threshold 90
```

### Step 4: Create .lycheeignore
```
# Skip localhost links
http://localhost*
http://127.0.0.1*
# Skip placeholder URLs (will be replaced before publish)
https://github.com/yourusername/
```

## Build and Test Commands

**Build:** N/A (tooling setup only)

**Test (targeted):**
```bash
just readme-check  # Verify READMEs in sync
lychee README.md docs/  # Check links
cargo doc-coverage --threshold 90  # Check coverage
```

**Test (regression):** `cargo test --workspace` (no behavior changes)

**Test (full gate):** CI passes with new checks

## Exit Criteria

1. **Targeted tests:** `just readme-check` passes, lychee finds no broken links, doc coverage ≥90%
2. **Regression tests:** All tests pass (tooling doesn't affect runtime)
3. **Full build gate:** CI docs job passes with new checks
4. **Full test suite:** All workspace tests pass
5. **Self-review:** No TODOs in CI config, all tools installed from crates.io
6. **Scope verification:** Only CI and justfile modified
