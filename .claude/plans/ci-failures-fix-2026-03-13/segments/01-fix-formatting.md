---
segment: 1
title: "Fix Formatting Violations"
depends_on: []
risk: 1/10
complexity: Low
cycle_budget: 5
status: pending
commit_message: "fix(format): Apply rustfmt to test files from coverage-95 plan"
---

# Segment 1: Fix Formatting Violations

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Apply rustfmt to test files added in coverage-95 plan to pass CI format checks.

**Depends on:** None

## Context: Issues Addressed

**Core Problem:** Two test files fail `cargo fmt --check`: `normalize_linktype_tests.rs` has 4 formatting violations (line breaks at lines 16, 36, 66, 88; import ordering at line 213) and `pipeline_ingest_error_tests.rs` has multiple array formatting issues (lines 57, 88, 112, 123, 151, 172, 205). These files were added during coverage-95 plan execution on March 13, 2026, and merged via orchestrator worktree workflow which bypassed pre-commit formatting hooks.

**Proposed Fix:** Run `cargo fmt --all` to auto-format according to `rustfmt.toml` rules (max_width=100, edition=2024, reorder_imports=true, newline_style="Unix").

**Pre-Mortem Risks:**
- cargo fmt might reformat unrelated files with unstaged changes (verify git status first)
- Unexpected formatting changes (review full diff before committing)
- Future orchestrator runs could re-introduce formatting issues (document the issue for process improvement)

## Scope

- `/Users/psauer/probe/crates/prb-pcap/tests/normalize_linktype_tests.rs`
- `/Users/psauer/probe/crates/prb-pcap/tests/pipeline_ingest_error_tests.rs`

## Key Files and Context

**Files requiring formatting:**
- `/Users/psauer/probe/crates/prb-pcap/tests/normalize_linktype_tests.rs`
  - Line 16: Multi-line function call closure needs proper breaks: `).unwrap();` should be on separate line
  - Line 36: Multi-line function call should be single-line: `Ipv4Header::new(20, 64, IpNumber(6), [127, 0, 0, 1], [127, 0, 0, 2]).unwrap();`
  - Line 66: Multi-line function call should be single-line: `Ipv4Header::new(20, 64, IpNumber(6), [192, 168, 1, 1], [10, 0, 0, 1]).unwrap();`
  - Line 88: Assert macro needs proper formatting with line breaks for string argument
  - Line 213: Import ordering needs correction: `use prb_pcap::{TcpFlags, TcpSegmentInfo, TransportInfo};` (alphabetical)

- `/Users/psauer/probe/crates/prb-pcap/tests/pipeline_ingest_error_tests.rs`
  - Lines 57, 88, 112, 151, 172, 205: PCAP header arrays need line break normalization
  - Line 123: Chained method calls need proper indentation for readability

**Rustfmt configuration** (`/Users/psauer/probe/rustfmt.toml`):
```toml
edition = "2024"
max_width = 100
hard_tabs = false
tab_spaces = 4
newline_style = "Unix"
reorder_imports = true
reorder_modules = true
remove_nested_parens = true
use_try_shorthand = true
```

**Context:** These files were added in commit 027ce39 on March 12, 2026, as part of the coverage-95-testing plan (segments S05: Packet Normalization Memory Safety and S06: Pipeline Core Robustness). The orchestrator workflow uses git worktrees for parallel execution, and the final merge commit bypassed pre-commit hooks where `cargo fmt` would normally run.

## Implementation Approach

1. **Verify clean working tree:**
   ```bash
   git status
   # Should show no unstaged changes except possibly these test files
   # If other files have changes, stash them first
   ```

2. **Run cargo fmt on entire workspace:**
   ```bash
   cargo fmt --all
   # This auto-formats all Rust files according to rustfmt.toml
   ```

3. **Verify only expected files changed:**
   ```bash
   git diff --name-only
   # Should show only:
   #   crates/prb-pcap/tests/normalize_linktype_tests.rs
   #   crates/prb-pcap/tests/pipeline_ingest_error_tests.rs
   ```

4. **Review changes to ensure correctness:**
   ```bash
   git diff crates/prb-pcap/tests/normalize_linktype_tests.rs
   git diff crates/prb-pcap/tests/pipeline_ingest_error_tests.rs
   # Verify: only whitespace/formatting changes, no logic changes
   ```

5. **Verify formatting passes:**
   ```bash
   cargo fmt --all -- --check
   # Should exit 0 with no output (success)
   ```

6. **Stage and commit changes:**
   ```bash
   git add crates/prb-pcap/tests/normalize_linktype_tests.rs
   git add crates/prb-pcap/tests/pipeline_ingest_error_tests.rs
   git commit -m "fix(format): Apply rustfmt to test files from coverage-95 plan

Applied cargo fmt to test files that were merged via orchestrator worktree
workflow, bypassing pre-commit hooks. Fixes CI Format & Lint job failure.

Changes:
- normalize_linktype_tests.rs: Fixed line breaks and import ordering
- pipeline_ingest_error_tests.rs: Normalized array formatting"
   ```

## Alternatives Ruled Out

- **Manual formatting fixes:** Rejected - error-prone, cargo fmt is authoritative and deterministic
- **Disable format checking temporarily:** Rejected - defeats purpose of code standards, CI should enforce quality
- **Add format exemptions for test files:** Rejected - these are normal test files with no special requirements

## Pre-Mortem Risks

- **cargo fmt reformatting unrelated files:** Mitigation - check git status before running, only stage/commit the two test files
- **Unexpected formatting changes:** Mitigation - review full diff before committing, rustfmt is deterministic and repeatable
- **Breaking test functionality:** Mitigation - formatting changes are whitespace-only, cannot affect test logic or outcomes

## Build and Test Commands

- Build: `cargo build -p prb-pcap`
- Test (targeted): `cargo test -p prb-pcap normalize_linktype pipeline_ingest_error`
- Test (regression): `cargo test -p prb-pcap`
- Test (full gate): `cargo nextest run --workspace`
- Format check: `cargo fmt --all -- --check`

## Exit Criteria

1. **Targeted tests:**
   - `cargo fmt --all -- --check` exits 0 with no output
   - `git diff --name-only` shows exactly 2 files changed
   - `git diff` shows only whitespace/formatting changes, no logic changes

2. **Regression tests:**
   - All prb-pcap tests pass: `cargo test -p prb-pcap` (600+ tests)
   - Specifically: `cargo test -p prb-pcap normalize_linktype` passes
   - Specifically: `cargo test -p prb-pcap pipeline_ingest_error` passes

3. **Full build gate:**
   - `cargo build --workspace` succeeds with zero warnings
   - `cargo clippy --workspace --all-targets -- -D warnings` passes

4. **Full test gate:**
   - `cargo nextest run --workspace` passes (all workspace tests)

5. **Self-review gate:**
   - Only 2 test files modified (no unrelated formatting changes)
   - No functional changes (formatting/whitespace only)
   - No unrelated formatting changes to production code
   - Commit message explains context (orchestrator worktree bypass)

6. **Scope verification gate:**
   - Changed files are exactly:
     - `crates/prb-pcap/tests/normalize_linktype_tests.rs`
     - `crates/prb-pcap/tests/pipeline_ingest_error_tests.rs`
   - No changes to production code (src/ directories)
   - No changes to other test files
   - No changes to configuration files

**Risk factor:** 1/10

**Estimated complexity:** Low

**Commit message:** `fix(format): Apply rustfmt to test files from coverage-95 plan`
