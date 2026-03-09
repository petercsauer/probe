Verify that a completed deep-plan's exit criteria are satisfied by the actual codebase. Run this after all segments have been built and committed, before declaring the plan complete.

Invocation: `/deep-verify <plan-file-or-directory>`

---

## Step 1: Ingest the Plan

Read the plan file (monolithic) or directory manifest (restructured format). Extract for every segment:
- All exit criteria (Targeted tests, Regression tests, Full build gate, Full test gate, Self-review gate, Scope verification gate)
- The pre-written commit message (to match against git log)
- The segment's stated scope (files/crates)

Build a verification checklist: one row per criterion, per segment.

---

## Step 2: Per-Segment Verification

For each segment in execution order:

### 2a. Commit check
```bash
git log --oneline | grep "<segment commit message keyword>"
```
- **PASS** if a commit matching the segment's commit message exists
- **FAIL** if no matching commit found (segment may not have been executed)

### 2b. Build gate
Run the segment's "Full build gate" command exactly as written in the segment brief.
- **PASS** if exit code 0
- **FAIL** if build errors

### 2c. Test gate
Run the segment's "Full test gate" command exactly as written in the segment brief.
```bash
cargo nextest run -p <crate>
```
- **PASS** if all tests pass
- **PARTIAL** if some tests fail
- **FAIL** if command errors or majority of tests fail

### 2d. Scope verification
Run:
```bash
git show --name-only <commit-hash>
```
Compare changed files against the segment's stated scope. Flag any file outside scope that was not annotated as `[supporting change]` in the builder's report.
- **PASS** if all changes are within scope or annotated
- **UNVERIFIABLE** if builder report is not available

### 2e. Self-review gate
Inspect the segment's commit diff:
```bash
git show <commit-hash>
```
Look for: TODO comments, commented-out code blocks, `unwrap()` in library code (not tests), dead imports, `#[allow(unused)]` attributes.
- **PASS** if none found
- **PARTIAL** if minor issues (e.g., one `// TODO: remove`)
- **UNVERIFIABLE** if too large to inspect manually — note and flag for human review

---

## Step 3: Cross-Segment Integration Check

Run full workspace gates regardless of individual segment results:

```bash
# Full build
cargo build --workspace

# Full test suite
cargo nextest run --workspace

# Lint gate
cargo clippy --workspace -- -D warnings
```

For each: **PASS**, **PARTIAL**, or **FAIL** with the exact error output.

Also check for workspace-level integration tests:
```bash
cargo nextest run --test '*'
```

---

## Step 4: Gaps Report

Produce a verdict table:

```
## Verification Report: [Plan Title]

**Plan file:** [path]
**Verified:** YYYY-MM-DD
**Segments checked:** N

### Per-Segment Results

| Segment | Commit | Build | Tests | Scope | Self-review | Overall |
|---------|--------|-------|-------|-------|-------------|---------|
| 1: [title] | PASS | PASS | PASS | PASS | PASS | ✅ PASS |
| 2: [title] | PASS | PASS | PARTIAL | UNVERIFIABLE | PASS | ⚠️ PARTIAL |

### Cross-Segment Integration

| Gate | Command | Result |
|------|---------|--------|
| Full workspace build | `cargo build --workspace` | PASS |
| Full workspace tests | `cargo nextest run --workspace` | PARTIAL (2 failing) |
| Lint gate | `cargo clippy --workspace -- -D warnings` | PASS |

### Gaps and Risks

#### [Severity: HIGH/MEDIUM/LOW] [Short description]
- **Segment:** [which segment]
- **Criterion:** [which exit criterion failed]
- **Detail:** [what exactly failed and what the error was]
- **Recommended action:** [fix inline / re-run builder / re-plan]
```

---

## Step 5: Final Verdict

**FULLY VERIFIED:** All criteria are PASS or UNVERIFIABLE-with-justification. Cross-segment gates all PASS. Update the plan's execution log: `**Deep-verify result:** FULLY VERIFIED YYYY-MM-DD`.

**PARTIALLY VERIFIED:** Some criteria PARTIAL or FAIL, but core functionality works (walking skeleton runs, no build errors). Feed PARTIAL/FAIL criteria into a follow-up `/deep-plan` cycle (Entry Point B, treating gaps as the existing plan).

**NOT VERIFIED:** Critical criteria FAIL (build broken, majority of tests failing, or core feature non-functional). Do not proceed. Re-enter `/deep-plan` on the failing segments.

---

## For This Project (PRB-specific)

All build/test commands use Rust/Cargo. See `.claude/commands/devcontainer-exec.md` for the full command reference and workspace structure.

Key integration check after all subsections:
```bash
# The walking skeleton must still work end-to-end
cargo build -p prb-cli
./target/debug/prb ingest fixtures/sample.json | ./target/debug/prb inspect --format table
```

If this pipeline fails after any subsection, that is a HIGH severity gap regardless of individual test results.
