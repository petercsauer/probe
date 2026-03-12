---
segment: 05
title: Dependency Management
depends_on: []
risk: 2
complexity: Low
cycle_budget: 2
estimated_lines: 2 files modified
---

# Segment 05: Dependency Management Workflow

## Context

Create automated dependency checking workflow and enhance workspace dependency management. This includes weekly outdated dependency checks and improved workspace.dependencies usage.

## Current State

- No automated dependency checking
- workspace.dependencies partially used
- Some duplicate dependencies (bitflags v1.3.2 in tree)

## Goal

Set up automated weekly dependency audits and optimize workspace dependency management.

## Exit Criteria

1. [ ] `.github/workflows/dependencies.yml` created
2. [ ] Workflow runs weekly to check for outdated dependencies
3. [ ] Manual workflow dispatch enabled for on-demand checks
4. [ ] Workspace Cargo.toml uses workspace.dependencies for all common deps
5. [ ] cargo tree --duplicates shows minimal duplication
6. [ ] Manual test: Run workflow, verify output

## Implementation Plan

### File 1: Dependency Check Workflow

Create `.github/workflows/dependencies.yml`:

```yaml
name: Dependency Updates

on:
  schedule:
    - cron: '0 9 * * 1'  # Weekly on Monday at 9 AM UTC
  workflow_dispatch:       # Allow manual trigger

jobs:
  outdated:
    name: Check Outdated Dependencies
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      - name: Install cargo-outdated
        run: cargo install cargo-outdated --locked

      - name: Check outdated
        run: |
          echo "## Outdated Dependencies" >> $GITHUB_STEP_SUMMARY
          cargo outdated --root-deps-only >> $GITHUB_STEP_SUMMARY
          cargo outdated --root-deps-only

  duplicates:
    name: Check Duplicate Dependencies
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      - name: Check duplicates
        run: |
          echo "## Duplicate Dependencies" >> $GITHUB_STEP_SUMMARY
          cargo tree --duplicates >> $GITHUB_STEP_SUMMARY || echo "No duplicates found"
          cargo tree --duplicates || echo "No duplicates found"

  audit:
    name: Security Audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      - name: Install cargo-audit
        run: cargo install cargo-audit --locked

      - name: Run audit
        run: cargo audit
```

### File 2: Optimize Workspace Dependencies

Review and consolidate in `/Users/psauer/probe/Cargo.toml`:

Current workspace.dependencies is good but can be enhanced:

```toml
[workspace.dependencies]
# ... existing dependencies ...

# Ensure all common deps are here
insta = { version = "1", features = ["json", "yaml"] }
proptest = "1"
assert_cmd = "2"
predicates = "3"
tempfile = "3"
criterion = { version = "0.5", features = ["html_reports"] }
walkdir = "2"

# Add any missing common dependencies from crate Cargo.tomls
```

Then update individual crate Cargo.toml files to use `workspace = true` for all common dependencies.

## Files to Create/Modify

1. `.github/workflows/dependencies.yml` (new, ~55 lines)
2. `Cargo.toml` (modify workspace.dependencies section)
3. Individual crate `Cargo.toml` files (update to use workspace deps)

## Test Plan

1. Create dependencies workflow file
2. Trigger workflow manually:
   - Go to Actions tab
   - Select "Dependency Updates" workflow
   - Click "Run workflow"
3. Verify workflow runs and reports:
   - Outdated dependencies
   - Duplicate dependencies
   - Security audit results
4. Review workspace.dependencies consolidation:
   ```bash
   cargo tree --duplicates
   # Should show minimal or zero duplicates
   ```
5. Verify builds still work:
   ```bash
   cargo build --workspace
   cargo test --workspace
   ```
6. Commit: "infra: Add dependency management workflow"

## Blocked By

None - this is independent infrastructure.

## Blocks

None - doesn't block other segments.

## Success Metrics

- Dependency workflow created and runs successfully
- Weekly schedule configured
- Manual trigger working
- Workspace dependencies consolidated
- Duplicate dependencies minimized

## Notes

- cargo-outdated may suggest updates that need testing before applying
- Some duplicates are unavoidable (e.g., transitive dependencies)
- Security audit failures should block the workflow (fail fast)
- Consider creating issues automatically for outdated/vulnerable deps
