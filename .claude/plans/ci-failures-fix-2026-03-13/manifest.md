---
plan: "CI Failures Fix Plan"
goal: "Fix all CI pipeline failures: formatting, WASM linking, security vulnerabilities, and validate in CI"
generated: 2026-03-13
status: Ready for execution
parent_plan: null
rules_version: 2026-03-13
---

# CI Failures Fix Plan -- Manifest

## Execution Order (Quick Wins + Validation)

1. **Wave 1 (Parallel):** Segments 1 & 2 can run concurrently
   - Segment 1: Fix Formatting (5 minutes)
   - Segment 2: Exclude WASM Fixture (10 minutes)

2. **Wave 2:** Segment 3 (after Segment 2 completes)
   - Segment 3: Patch Security Vulnerabilities (20 minutes)

3. **Wave 3:** Segment 4 (after Segment 3 completes)
   - Segment 4: Push and Validate CI Pipeline (15 minutes)

## Dependency Diagram

```
Wave 1 (Parallel):
    [1: Formatting] ────┐
                        ├─── [Both independent, can run concurrently]
    [2: WASM Exclude] ──┼─── [Unblocks builds for S3]
                        │
                        ▼
Wave 2:
    [3: Security Patches] (requires working builds from S2)
                        │
                        ▼
Wave 3:
    [4: Push & Validate CI] (requires all fixes from S1-S3)
```

## Segment Index

| # | Title | File | Depends On | Risk | Complexity | Cycles | Status |
|---|-------|------|------------|------|------------|--------|--------|
| 1 | Fix Formatting Violations | segments/01-fix-formatting.md | None | 1/10 | Low | 5 | pending |
| 2 | Exclude WASM Fixture from Workspace | segments/02-exclude-wasm-fixture.md | None | 2/10 | Low | 10 | pending |
| 3 | Patch Security Vulnerabilities | segments/03-patch-security-vulnerabilities.md | 2 | 6/10 | Medium | 20 | pending |
| 4 | Push and Validate CI Pipeline | segments/04-push-and-validate-ci.md | 3 | 3/10 | Medium | 15 | pending |

**Total estimated effort:** 50 cycles (~50 minutes)

**Parallelization:**
- Wave 1: S01 and S02 run concurrently (10 minutes wall-clock)
- Wave 2: S03 runs solo (20 minutes)
- Wave 3: S04 runs solo (15 minutes)
- **Total wall-clock time:** ~45 minutes

## Issues Addressed

### Issue 1: Formatting Violations (Segment 1)
**Problem:** Two test files from coverage-95 plan fail `cargo fmt --check`
**Root Cause:** Worktree merge bypassed pre-commit hooks
**Fix:** Run `cargo fmt --all`
**Risk:** 1/10 - Cosmetic only

### Issue 2: WASM Linking Failure (Segment 2)
**Problem:** `prb-plugin-wasm-test-fixture` tries to link as native dylib, fails with undefined WASM runtime symbols
**Root Cause:** Fixture must compile to wasm32-unknown-unknown but workspace builds for native target
**Fix:** Add to workspace `exclude` list
**Risk:** 2/10 - Test-only crate with pre-built binary

### Issue 3: Security Vulnerabilities (Segment 3)
**Problem:** 4 cargo-audit warnings: wasmtime 37.0.3 (3 CVEs), backoff 0.4.0 (unmaintained)
**Root Cause:** Transitive dependencies via extism and async-openai haven't upgraded
**Fix:** Cargo patch for wasmtime 36.0.6, upgrade async-openai 0.20 → 0.33
**Risk:** 6/10 - Shared dependencies, API changes possible

### Issue 4: CI Validation (Segment 4)
**Problem:** Local fixes need to be validated in CI environment (fresh clone, multiple platforms)
**Root Cause:** CI runs in different environment than local dev, can catch platform-specific issues
**Fix:** Push commits, monitor CI run, debug any failures iteratively
**Risk:** 3/10 - CI might reveal new platform-specific issues

## Preamble Injection

Before launching any builder subagent, the orchestration agent assembles the prompt:

1. Read `.claude/commands/iterative-builder.md` (iteration protocol, WIP commits, cycle budgets)
2. Read `.claude/commands/devcontainer-exec.md` (build environment, Cargo commands)
3. Read the segment file from `segments/{NN}-{slug}.md`

**Assembled prompt** = `[iterative-builder.md]` + `[devcontainer-exec.md]` + `[segment file]`

## Execution Instructions

Use the external `orchestrate` tool to execute this plan:

```bash
cd /Users/psauer/probe
orchestrate run .claude/plans/ci-failures-fix-2026-03-13
```

The orchestrator will:
1. Launch iterative-builder subagents for each segment in dependency order
2. Execute Wave 1 with 2 parallel segments (S01, S02)
3. Execute Wave 2 after S02 completes (S03)
4. Execute Wave 3 after S03 completes (S04 - push and validate)
5. Run gate checks after each wave: `cargo nextest run --workspace`
6. Squash WIP commits into final segment commits
7. Update execution log with results

## Expected CI Impact

**Before plan:**
- ❌ Format & Lint - FAILING (formatting violations)
- ❌ Test (ubuntu-latest) - FAILING (WASM linking)
- ❌ Test (macos-latest) - FAILING (WASM linking)
- ❌ Test (windows-latest) - FAILING (WASM linking)
- ❌ Documentation - FAILING (WASM linking blocks docs)
- ❌ Security Audit - FAILING (4 advisories)
- ❌ Code Coverage - FAILING (WASM linking blocks coverage)
- ⏭️ Benchmarks - SKIPPED

**After plan (validated by Segment 4):**
- ✅ Format & Lint - PASSING (S01 fixes formatting)
- ✅ Test (ubuntu-latest) - PASSING (S02 fixes WASM linking)
- ✅ Test (macos-latest) - PASSING (S02 fixes WASM linking)
- ✅ Test (windows-latest) - PASSING (S02 fixes WASM linking)
- ✅ Documentation - PASSING (S02 fixes builds)
- ✅ Security Audit - PASSING (S03 patches CVEs, 1 warning acceptable)
- ✅ Code Coverage - PASSING (S02 fixes builds)
- ⏭️ Benchmarks - SKIPPED (runs on schedule only)

**Validation:** Segment 4 pushes all commits and waits for CI to complete, debugging any failures in real-time.

## Plan Status

**Status:** Ready for execution
**Created:** 2026-03-13
**Estimated completion:** 45 minutes wall-clock time
**Risk budget:** 1 segment at Risk 6/10 (S03), 1 at Risk 3/10 (S04) - acceptable with thorough testing
**Validation:** Plan includes CI validation loop (S04) to ensure all fixes work in production CI environment
