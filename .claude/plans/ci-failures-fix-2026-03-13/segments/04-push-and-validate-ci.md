---
segment: 4
title: "Push and Validate CI Pipeline"
depends_on: [3]
risk: 3/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "ci: Validate all CI jobs pass after fixes"
---

# Segment 4: Push and Validate CI Pipeline

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Push all commits to GitHub and wait for CI to complete successfully, debugging any failures.

**Depends on:** Segment 3 (all fixes must be committed before pushing)

## Context: Issues Addressed

**Core Problem:** After applying all fixes (formatting, WASM exclusion, security patches), we need to verify that CI actually passes. The fixes were developed and tested locally, but CI runs in a clean environment with different conditions (fresh clone, multiple OS platforms, strict checks). This segment closes the validation loop by pushing commits and monitoring CI results.

**Proposed Fix:** Push all commits to origin/main, trigger a CI run, monitor the run status, and debug any failures. If CI fails, analyze the failure logs, identify root causes, and apply additional fixes in an iterative debugging loop.

**Pre-Mortem Risks:**
- CI might fail on platforms not tested locally (Windows, different macOS/Linux versions)
- Cargo.lock conflicts if someone else pushed changes
- Security audit might find new vulnerabilities in transitive dependencies
- Clippy might catch warnings that weren't caught locally (version differences)
- Tests might be flaky or environment-dependent
- Network issues or GitHub Actions quota problems

## Scope

- Git push to remote
- GitHub Actions CI monitoring
- CI failure analysis and debugging
- Additional fixes if needed

## Key Files and Context

**Git state before push:**
- Should have 3-4 commits from segments 1-3:
  1. "fix(format): Apply rustfmt to test files from coverage-95 plan"
  2. "fix(build): Exclude WASM test fixture from workspace builds"
  3. "fix(security): Patch wasmtime CVEs and upgrade async-openai"
  4. Possibly: Additional fix commits if API changes were needed

**CI workflow** (`.github/workflows/ci.yml`):
- 8 jobs: Format & Lint, Test (ubuntu/macos/windows), Documentation, Security Audit, Code Coverage, Benchmarks
- All must pass for CI to be green

**GitHub CLI tools:**
- `gh run list` - List recent CI runs
- `gh run view <run-id>` - View run details
- `gh run watch <run-id>` - Watch run in real-time
- `gh run view <run-id> --log-failed` - View failed job logs

**Expected CI timeline:**
- Format & Lint: ~15 seconds
- Security Audit: ~3 minutes
- Test (ubuntu): ~3 minutes
- Test (macos): ~5 minutes (slower runners)
- Test (windows): ~8 minutes (slowest)
- Documentation: ~2 minutes
- Code Coverage: ~4 minutes
- Benchmarks: Skipped (only runs on schedule)
- **Total:** ~8-10 minutes for all jobs

## Implementation Approach

### Phase 1: Pre-Push Verification

1. **Verify all commits are present:**
   ```bash
   git log --oneline -4
   # Should show commits from segments 1-3
   # Verify commit messages match expected format
   ```

2. **Verify local tests still pass:**
   ```bash
   cargo test --workspace
   # Final sanity check before pushing
   ```

3. **Verify local build passes:**
   ```bash
   cargo build --workspace --all-targets
   # Ensure no build regressions
   ```

4. **Verify formatting is clean:**
   ```bash
   cargo fmt --all -- --check
   # Should exit 0 (formatting fixed in S01)
   ```

5. **Verify security audit is clean:**
   ```bash
   cargo audit
   # Should show 0 vulnerabilities, 1 warning (backoff - acceptable)
   ```

### Phase 2: Push to Remote

6. **Check remote status:**
   ```bash
   git fetch origin
   git status
   # Check if origin/main has moved ahead (potential conflicts)
   ```

7. **Handle conflicts if needed:**
   ```bash
   if git log origin/main..HEAD --oneline | grep -q .; then
     echo "Commits to push exist"
   fi

   if git log HEAD..origin/main --oneline | grep -q .; then
     echo "WARNING: origin/main has new commits - may need rebase"
     git log HEAD..origin/main --oneline
     # If conflicts exist, rebase on origin/main
     git rebase origin/main
     # Re-run tests after rebase
     cargo test --workspace
   fi
   ```

8. **Push commits:**
   ```bash
   git push origin main
   # Push all commits to trigger CI
   ```

### Phase 3: Monitor CI Run

9. **Get the CI run ID:**
   ```bash
   # Wait a few seconds for GitHub to register the push
   sleep 5

   # Get the most recent CI run for main branch
   gh run list --branch main --workflow ci.yml --limit 1 --json databaseId,status,conclusion,headSha

   # Extract run ID
   RUN_ID=$(gh run list --branch main --workflow ci.yml --limit 1 --json databaseId --jq '.[0].databaseId')
   echo "Monitoring CI run: $RUN_ID"
   echo "View in browser: https://github.com/petercsauer/probe/actions/runs/$RUN_ID"
   ```

10. **Watch CI progress:**
    ```bash
    # Watch CI run in real-time
    gh run watch $RUN_ID --interval 10
    # Or use gh run view for periodic checks
    ```

11. **Wait for completion (with timeout):**
    ```bash
    # Poll until CI completes (max 15 minutes)
    for i in {1..90}; do
      STATUS=$(gh run view $RUN_ID --json status --jq '.status')
      CONCLUSION=$(gh run view $RUN_ID --json conclusion --jq '.conclusion')

      echo "CI status: $STATUS, conclusion: $CONCLUSION"

      if [ "$STATUS" = "completed" ]; then
        break
      fi

      sleep 10
    done

    if [ "$STATUS" != "completed" ]; then
      echo "ERROR: CI timed out after 15 minutes"
      exit 1
    fi
    ```

### Phase 4: Analyze Results

12. **Check CI conclusion:**
    ```bash
    CONCLUSION=$(gh run view $RUN_ID --json conclusion --jq '.conclusion')

    if [ "$CONCLUSION" = "success" ]; then
      echo "✅ CI PASSED - All jobs successful!"
      exit 0
    else
      echo "❌ CI FAILED - Analyzing failures..."
    fi
    ```

13. **Get failed job details:**
    ```bash
    # List all jobs and their conclusions
    gh run view $RUN_ID --json jobs --jq '.jobs[] | {name: .name, conclusion: .conclusion}'

    # Get failed job IDs
    FAILED_JOBS=$(gh run view $RUN_ID --json jobs --jq '.jobs[] | select(.conclusion == "failure") | .databaseId' | tr '\n' ' ')

    echo "Failed jobs: $FAILED_JOBS"
    ```

14. **View failed job logs:**
    ```bash
    # Get logs for failed jobs
    gh run view $RUN_ID --log-failed

    # Or view specific job logs
    for JOB_ID in $FAILED_JOBS; do
      echo "=== Logs for job $JOB_ID ==="
      gh api "repos/petercsauer/probe/actions/jobs/$JOB_ID/logs" | tail -100
    done
    ```

### Phase 5: Debug and Fix Failures

15. **Categorize failures:**

    **Common CI failure patterns:**

    a. **Format & Lint failures:**
       - Symptom: "cargo fmt --check" exit code 1
       - Debug: Run `cargo fmt --all -- --check` locally, review diff
       - Fix: Run `cargo fmt --all`, commit, push

    b. **Build failures (platform-specific):**
       - Symptom: Compilation errors on Windows/macOS but not locally
       - Debug: Check error messages for platform-specific issues (path separators, line endings)
       - Fix: Add platform-specific cfg attributes or fix path handling

    c. **Test failures:**
       - Symptom: Tests pass locally but fail in CI
       - Debug: Check for environment dependencies (temp dirs, timing, parallelism)
       - Fix: Make tests deterministic, use proper fixtures

    d. **Security audit failures:**
       - Symptom: New vulnerabilities appeared
       - Debug: Run `cargo audit` locally, check if new advisories were published
       - Fix: Update patches or dependencies

    e. **Clippy warnings:**
       - Symptom: New warnings on CI (different Rust version)
       - Debug: Run `cargo clippy --workspace --all-targets -- -D warnings` locally
       - Fix: Address warnings or update Cargo.toml clippy config

    f. **Timeout issues:**
       - Symptom: Jobs time out (especially Windows)
       - Debug: Check if builds/tests are hanging
       - Fix: Reduce test scope or increase timeouts

16. **Apply fixes iteratively:**
    ```bash
    # After identifying root cause, apply fix
    # Example: Additional formatting needed
    cargo fmt --all
    git add -A
    git commit -m "fix(ci): Address additional formatting issues found in CI"
    git push origin main

    # Restart monitoring loop (go back to Phase 3)
    ```

17. **Document any CI-specific issues:**
    ```bash
    # If fixes were needed, document what was found
    # Add to commit message or CHANGELOG
    echo "CI required additional fixes beyond local testing:"
    echo "- Platform-specific issue: ..."
    echo "- Environment difference: ..."
    ```

## Alternatives Ruled Out

- **Manual CI monitoring in browser:** Rejected - automated monitoring with `gh` CLI is faster and can be integrated into the segment
- **Skip CI validation:** Rejected - defeats the purpose of the plan (fixing CI failures)
- **Push without waiting for CI:** Rejected - need to verify fixes work before declaring success
- **Only check CI status, don't debug:** Rejected - if CI fails after our fixes, we need to understand why

## Pre-Mortem Risks

- **New CI failures unrelated to our changes:** Mitigation - compare failures to the original issues, separate new problems from regression
- **Flaky tests causing false failures:** Mitigation - re-run failed jobs, identify flaky tests and document them
- **GitHub Actions quota exceeded:** Mitigation - already addressed by making repo public in previous session
- **Long CI wait times blocking segment:** Mitigation - 15-minute timeout, fail fast if CI doesn't complete
- **Multiple developers pushing simultaneously:** Mitigation - rebase and re-test if origin/main moves ahead

## Build and Test Commands

- Build: `cargo build --workspace --all-targets`
- Test (targeted): N/A (testing is delegated to CI)
- Test (regression): `cargo test --workspace` (pre-push verification)
- Test (full gate): CI runs full gate (`cargo nextest run --workspace`)
- Push: `git push origin main`
- Monitor: `gh run watch <run-id>`

## Exit Criteria

1. **Targeted tests:**
   - All commits from segments 1-3 pushed to origin/main
   - CI run triggered and completed
   - All 8 CI jobs show status "success"
   - `gh run view <run-id> --json conclusion --jq '.conclusion'` returns "success"
   - No failed jobs in run summary

2. **Regression tests:**
   - Local tests still pass after any CI-driven fixes: `cargo test --workspace`
   - All previous segment fixes remain intact

3. **Full build gate:**
   - CI "Format & Lint" job passes (formatting is clean)
   - CI "Security Audit" job passes (0 vulnerabilities, 1 acceptable warning)
   - CI "Test (ubuntu-latest)" job passes
   - CI "Test (macos-latest)" job passes
   - CI "Test (windows-latest)" job passes
   - CI "Documentation" job passes
   - CI "Code Coverage" job passes
   - Benchmarks skipped (expected - only runs on schedule)

4. **Full test gate:**
   - No regression in any CI job compared to pre-fix state
   - All originally failing jobs now passing

5. **Self-review gate:**
   - If CI required additional fixes, those fixes are minimal and well-documented
   - No "quick hacks" to make CI pass (e.g., skipping tests, disabling warnings)
   - Commit history is clean (no "fix CI take 2, take 3" spam - squash if needed)
   - Any CI-specific issues are documented in commit messages

6. **Scope verification gate:**
   - Only commits related to CI fixes are pushed
   - No unrelated changes snuck in
   - Commit messages follow conventional commit format
   - If additional fixes were needed, they're in separate logical commits

**Success criteria summary:**
```bash
# This command should show all green checkmarks:
gh run view <run-id>

# Expected output:
✓ Format & Lint (15s)
✓ Security Audit (3m)
✓ Test (ubuntu-latest) (3m)
✓ Test (macos-latest) (5m)
✓ Test (windows-latest) (8m)
✓ Documentation (2m)
✓ Code Coverage (4m)
- Benchmarks (skipped)

Run conclusion: success
```

**Risk factor:** 3/10

**Estimated complexity:** Medium

**Commit message:** `ci: Validate all CI jobs pass after fixes`
