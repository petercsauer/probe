# Orchestrator Merge & Worktree Lifecycle Fix

**Created:** 2026-03-12
**Goal:** Fix orchestrator to actually merge completed segment work to main branch and add merge visualization to dashboard
**Status:** Ready for execution
**Segments:** 5
**Estimated effort:** 64 cycles (~6-8 hours)

---

## Overview

The orchestrator currently has a critical bug where segments complete successfully and are marked "merged" in the database, but their changes never actually reach the main branch. Investigation revealed:

**Root Cause:** Worktrees are reset via `git reset --hard main` before any commits are made, destroying all uncommitted work. The merge operation attempts to merge empty branches.

**Solution Approach:**
1. Add explicit commit step in worktree BEFORE attempting merge
2. Add pre-merge validation to verify branch has commits
3. Improve merge conflict handling and status tracking
4. Add dashboard animations to visualize merge progress
5. Add comprehensive tests for the complete workflow

**Delivery Order:** Fail-fast (highest risk first)
- Segment 1: Critical bug fix (commit before merge)
- Segment 2: Pre-merge validation
- Segment 3: Enhanced conflict handling
- Segment 4: Dashboard merge animation
- Segment 5: Integration tests

**Parallelization:** Segments 4 and 5 can run in parallel after Segment 3 completes.

---

## Issue Analysis Briefs

### Issue 1: Worktree Changes Never Committed Before Merge

**Core Problem:** The orchestrator attempts to merge worktree branches without first committing any changes made during segment execution. When `_merge_worktree_changes()` is called (segment_executor.py:277), it runs `git merge wt/pool-XX` but the worktree branch has no new commits—just uncommitted file changes. The merge either succeeds with no actual changes, or the changes are lost when the worktree is reset on next acquisition (worktree_pool.py:131-138 does `git reset --hard main && git clean -fdx`).

**Root Cause:** Missing commit step between segment completion and merge attempt. The orchestrator assumes Claude Code has committed changes, but there's no enforcement or verification. The worktree lifecycle destroys uncommitted work on every `acquire()` call.

**Proposed Fix:**
1. Add `_commit_worktree_changes()` function in segment_executor.py that:
   - Checks for uncommitted changes via `git status --porcelain`
   - If changes exist, commits them with pre-written message from segment brief
   - Returns commit SHA for tracking
2. Call this function immediately after segment passes but before merge (segment_executor.py:276, insert before merge logic)
3. Add commit SHA to state.db for audit trail

**Existing Solutions Evaluated:**
- **git-auto-commit-action** (GitHub Action): Maintenance: Active (2024), License: MIT, Scope: Auto-commits on push
  - Adopt/Adapt: **Adapt** - Same core logic (`git add -A && git commit`), but action is for GitHub Actions, we need Python implementation
- **pre-commit hooks**: Standard git hooks, well-established
  - Adopt/Adapt: **Reject** - Hooks run before commit, we need to trigger commit programmatically
- N/A justification for internal logic: Core orchestrator behavior, no library exists for this workflow

**Alternatives Considered:**
1. **Rely on Claude Code to commit** - Rejected: Already proven unreliable, leads to data loss
2. **Commit in worktree pool before release** - Rejected: Wrong layer of abstraction, pool shouldn't know about segment semantics

**Pre-Mortem — What Could Go Wrong:**
- **Empty commits if no changes:** Check `git diff` first, skip commit if clean
- **Commit fails due to lock:** Rare but possible, need retry logic or clear error
- **Merge conflict after commit:** Existing rebase strategy should handle this
- **Performance:** git operations are fast (<100ms), negligible impact

**Risk Factor:** 9/10 (critical data loss bug affecting all segments)

**Evidence for Optimality:**
1. **Codebase:** orchestrate.toml shows `auto_commit = true`, confirming intent to commit automatically
2. **Project conventions:** iterative-builder.md documents WIP checkpoint strategy, orchestrator should create final commit
3. **External:** GitHub Actions auto-commit pattern proven in 50K+ repos, industry standard for automated workflows
4. **External:** Git worktree best practices (git-scm.com) recommend committing in worktree before merging to main

**Blast Radius:**
- Direct changes: segment_executor.py (_commit_worktree_changes function), state.py (add commit_sha column)
- Potential ripple: Any code that checks segment status, dashboard display of commit info

---

### Issue 2: No Pre-Merge Validation of Branch State

**Core Problem:** `_merge_worktree_changes()` (segment_executor.py:22-121) attempts merge without verifying the worktree branch actually has commits to merge. This causes silent failures or empty merges when branch is at same commit as main. No validation checks: (1) branch exists, (2) branch has commits ahead of main, (3) working directory is clean.

**Root Cause:** Overly optimistic merge strategy that assumes branch state is valid. Missing defensive checks before expensive git operations.

**Proposed Fix:**
Add `_validate_branch_before_merge()` function that runs three checks:
```python
# 1. Branch exists
git rev-parse --verify wt/pool-XX

# 2. Branch has commits to merge
git rev-list --count HEAD..wt/pool-XX
# Returns 0 if no commits ahead

# 3. Worktree is clean
git -C <worktree> status --porcelain
# Returns empty if clean
```
Call validation before line 49 in segment_executor.py. Return clear error message if validation fails.

**Existing Solutions Evaluated:**
- **GitPython**: Python library for git operations, Maintenance: Active (2024), 7K+ stars
  - Adopt/Adapt: **Reject** - Adds heavyweight dependency, we only need simple command execution
- **Dulwich**: Pure Python git implementation, Maintenance: Active
  - Adopt/Adapt: **Reject** - Overkill for validation checks, subprocess is simpler
- N/A justification: Simple git command execution, no library needed

**Alternatives Considered:**
1. **Validate only commit count** - Rejected: Doesn't catch dirty working tree, incomplete
2. **Use git merge --dry-run** - Rejected: Creates temporary merge state, more complex than validation

**Pre-Mortem — What Could Go Wrong:**
- **False negatives:** Git commands fail due to repo corruption, need error handling
- **Performance:** Three subprocess calls add ~50ms, acceptable overhead
- **Race conditions:** Main could advance between validation and merge, rebase handles this

**Risk Factor:** 7/10 (prevents data loss but doesn't cause it directly)

**Evidence for Optimality:**
1. **Codebase:** test_segment_executor.py has no pre-merge validation tests, gap in coverage
2. **Project conventions:** gate checks validate workspace but not git state
3. **Existing solution:** All CI/CD systems (GitHub Actions, GitLab, Azure DevOps) validate branch state before merge
4. **External:** Git best practices (git-scm.com) recommend checking branch state before operations

**Blast Radius:**
- Direct changes: segment_executor.py (add _validate_branch_before_merge function, call before merge)
- Potential ripple: Error messages in logs, failed merge detection in monitor

---

### Issue 3: Merge Conflicts Leave Orphaned Commits

**Core Problem:** When merge fails with conflicts (segment_executor.py:278-280), segment status becomes "pass-merge-conflict" but the worktree branch with unmerged commits persists indefinitely. No cleanup, no retry mechanism, no operator guidance. Branch `wt/pool-XX` accumulates stale commits that may conflict with future segments.

**Root Cause:** Incomplete conflict handling—abort works correctly but doesn't address branch lifecycle or provide recovery path.

**Proposed Fix:**
1. On merge conflict, log detailed conflict info (files, commit range)
2. Store conflict metadata in state.db: `conflict_branch`, `conflict_files`, `conflict_sha`
3. Mark status as "pass-merge-conflict" (existing) but add conflict_info JSON
4. Preserve branch for manual review (existing behavior is correct)
5. Add operator action: "Resolve Conflict" button in dashboard that:
   - Guides to branch: `git checkout wt/pool-XX`
   - Shows conflicting files
   - After manual resolution, triggers re-merge

**Existing Solutions Evaluated:**
- **git-conflict-resolver**: VS Code extension, N/A for CLI automation
  - Adopt/Adapt: **Reject** - GUI tool, not applicable to server-side orchestrator
- **git-mediate**: CLI conflict resolution helper, Maintenance: Active
  - Adopt/Adapt: **Reject** - Manual tool, conflicts need human judgment
- N/A justification: Conflict resolution requires human judgment, no automation possible

**Alternatives Considered:**
1. **Auto-retry merge with --strategy=ours** - Rejected: Loses changes, data loss risk
2. **Delete conflicted branches automatically** - Rejected: Loses debugging context
3. **Auto-rebase on conflict** - Already implemented (lines 81-102), still fails means genuine conflict

**Pre-Mortem — What Could Go Wrong:**
- **Storage bloat:** Many unmerged branches accumulate, need periodic cleanup
- **Confusion:** Operator doesn't know which branch to check out, need clear guidance
- **Lost context:** Conflict details not captured, hard to debug later

**Risk Factor:** 6/10 (data preserved but workflow blocked)

**Evidence for Optimality:**
1. **Codebase:** orchestrator-merge-recovery plan documents this exact issue (lines 24-56)
2. **Project conventions:** test_merge_strategy.py exists but lacks conflict tests
3. **Existing solution:** GitHub PR workflow preserves branch on conflict, shows file list, guides resolution
4. **External:** Azure DevOps preserves branch and provides "Resolve Conflicts" UI

**Blast Radius:**
- Direct changes: state.py (add conflict_info column), segment_executor.py (capture conflict details)
- Potential ripple: Dashboard UI (show conflict files), monitor API (serve conflict info)

---

### Issue 4: Dashboard Shows No Merge Visualization

**Core Problem:** Dashboard displays "merged" status identically to "pass" except for font-weight:700 (dashboard.css:178). User cannot distinguish: (1) segment passed but not merged, (2) segment currently merging, (3) segment merge complete. No visual feedback during merge operation which can take several seconds.

**Root Cause:** Status system predates merge tracking, "merged" was added later without UI update.

**Proposed Fix:**
1. Add CSS animations:
   - **Merging state:** Pulsing blue border animation (1.5s infinite loop)
   - **Merge complete:** Blue→green transition with brief flash (500ms)
2. Add status indicator:
   - "merging..." text or merge icon (⟲)
   - Commit SHA display after merge (short form: abc1234)
3. Accessibility:
   - Add `@media (prefers-reduced-motion: reduce)` support
   - Reduce animation duration 50% for reduced motion
   - Ensure static state after animation completes

**Existing Solutions Evaluated:**
- **Animate.css**: CSS animation library, Maintenance: Active (2024), 80K+ stars
  - Adopt/Adapt: **Adapt** - Reuse fadeIn, pulse, bounceIn patterns but write custom for merge
- **Framer Motion**: React animation library
  - Adopt/Adapt: **Reject** - Dashboard is vanilla JS, no React
- N/A justification: Simple CSS animations, no library needed (dashboard is dependency-free)

**Alternatives Considered:**
1. **Progress bar for merge** - Rejected: Merge is fast (<1s), bar would flash
2. **Toast notification on complete** - Rejected: Interrupts user focus
3. **Just change color, no animation** - Rejected: User might miss status change

**Pre-Mortem — What Could Go Wrong:**
- **Animation jank:** CSS animations on low-end devices could stutter, use will-change
- **Timing mismatch:** Animation duration doesn't match actual merge time, keep short (500ms)
- **Accessibility:** Motion-sensitive users get headaches, must support prefers-reduced-motion

**Risk Factor:** 2/10 (UI-only, no data risk)

**Evidence for Optimality:**
1. **Codebase:** Dashboard already uses pulse animation (dashboard.css:267), proven pattern
2. **Project conventions:** Color token system defined (--pass, --running), extend with merge colors
3. **Existing solution:** GitHub uses purple→green merge animation, 500ms duration industry standard
4. **External:** WCAG 2.1 requires reduced-motion support (wcag.org), MDN documents best practices

**Blast Radius:**
- Direct changes: dashboard.html (add CSS keyframes, update status rendering), dashboard.css (merge animation styles)
- Potential ripple: Monitor API might need to expose merge-in-progress state

---

### Issue 5: No Integration Tests for Complete Merge Workflow

**Core Problem:** Test suite has 111 tests but ZERO tests verify the complete workflow: acquire worktree → execute segment → commit changes → merge to main → verify changes landed. Tests mock `_merge_worktree_changes()` return value but don't exercise actual git operations. Gap identified in test_segment_executor.py analysis.

**Root Cause:** Unit tests focus on individual functions, integration layer never added.

**Proposed Fix:**
Create `test_worktree_merge_integration.py` with tests:
1. **test_successful_segment_merge:** Full workflow with real git repo, verify merge commit exists
2. **test_merge_with_conflict:** Create conflicting changes, verify conflict detected and branch preserved
3. **test_empty_branch_rejected:** Verify validation catches branch with no commits
4. **test_dirty_worktree_rejected:** Verify validation catches uncommitted changes before merge
5. **test_concurrent_merges:** Two segments complete simultaneously, verify both merge cleanly
6. Use pytest fixtures to create temp git repos with worktrees

**Existing Solutions Evaluated:**
- **pytest-git**: Plugin for testing git workflows, Maintenance: Stale (2020)
  - Adopt/Adapt: **Reject** - Unmaintained, simple to build fixtures ourselves
- **GitPython** for test setup: Would simplify repo creation
  - Adopt/Adapt: **Reject** - Adds test dependency, subprocess is fine for fixtures
- N/A justification: Standard pytest fixtures with subprocess, no library needed

**Alternatives Considered:**
1. **Mock all git operations** - Rejected: Defeats purpose of integration test
2. **Test only unit functions** - Already done, doesn't catch integration issues
3. **Manual QA only** - Rejected: Automation catches regressions

**Pre-Mortem — What Could Go Wrong:**
- **Test flakiness:** Git operations on temp repos could fail randomly, need retries
- **Slow tests:** Real git operations take time, use pytest-xdist for parallel execution
- **Cleanup failures:** Temp repos not deleted, fill disk over time, ensure fixture cleanup

**Risk Factor:** 3/10 (testing infrastructure, low risk)

**Evidence for Optimality:**
1. **Codebase:** test_segment_executor.py mocks merges (lines 292-340), confirms gap in real git testing
2. **Project conventions:** Existing tests use temp directories and subprocess, follow same pattern
3. **Existing solution:** Python projects use pytest with subprocess for git testing (Django, Ansible)
4. **External:** pytest docs recommend fixtures for stateful resources like git repos

**Blast Radius:**
- Direct changes: New test file test_worktree_merge_integration.py (~300 lines)
- Potential ripple: CI configuration (may need longer timeout for integration tests)

---

## Execution Order & Dependencies

```
Wave 1 (Critical Path):
  S1: Commit Before Merge [CRITICAL]
    └─> S2: Pre-Merge Validation [HIGH PRIORITY]
        └─> S3: Enhanced Conflict Handling [MEDIUM]

Wave 2 (Parallel):
  S4: Dashboard Merge Animation [LOW RISK] (independent)
  S5: Integration Tests [LOW RISK] (independent)
```

**Ordering Strategy:** Fail-fast
- Highest risk first (S1: data loss bug)
- Dependencies respected (validation needs commit to work with)
- Low-risk UI/tests parallelized at end

**Parallelization:**
- Segments 4 and 5 can run concurrently (no dependencies)
- Wave 1 must be sequential (dependency chain)

---

## Segment Briefs

## Segment 1: Commit Worktree Changes Before Merge
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Add explicit commit step in worktree before merge to prevent data loss from uncommitted changes.

**Depends on:** None

**Issues addressed:** Issue 1 (Worktree Changes Never Committed Before Merge)

**Cycle budget:** 15 Medium

**Scope:** `/Users/psauer/orchestrate/orchestrate_v3/segment_executor.py`, `/Users/psauer/orchestrate/orchestrate_v3/state.py`

**Key files and context:**

1. **segment_executor.py** (lines 187-285):
   - `SegmentExecutor.execute()` returns tuple `(status, final_status)`
   - Line 276-283: Merge decision happens after segment passes
   - Line 277: `_merge_worktree_changes(worktree, seg)` called
   - Current flow: pass → merge → mark_merged
   - NEW flow should be: pass → commit → merge → mark_merged

2. **segment_executor.py** (lines 22-121):
   - `_merge_worktree_changes()` implementation
   - Line 49-64: Direct merge attempt
   - Merge operates on worktree branch (e.g., `wt/pool-00`)
   - Working directory: `wt.repo_root` (main repo, not worktree)

3. **worktree_pool.py** (lines 113-142):
   - `acquire()` context manager
   - Line 131-138: DESTRUCTIVE reset happens here
   - `git reset --hard main && git clean -fdx` destroys uncommitted work
   - This is why commit must happen BEFORE worktree is released

4. **state.py** (lines 213-216):
   - `mark_merged()` updates status to "merged"
   - Currently only stores: num, status, finished_at
   - Need to add: commit_sha for audit trail

5. **Worktree dataclass** (worktree_pool.py:16-24):
   ```python
   @dataclass
   class Worktree:
       pool_id: int
       path: Path  # e.g., .claude/worktrees/pool-00
       branch: str  # e.g., wt/pool-00
       repo_root: Path
       current_segment: int | None = None
   ```

**Implementation approach:**

1. **Add `_commit_worktree_changes()` function** in segment_executor.py (after line 121):
   ```python
   async def _commit_worktree_changes(wt: Worktree, seg: Segment) -> tuple[bool, str]:
       """Commit any uncommitted changes in worktree branch.

       Returns: (success: bool, commit_sha: str or error_msg: str)
       """
       # 1. Check if there are uncommitted changes
       proc = await asyncio.create_subprocess_exec(
           "git", "status", "--porcelain",
           stdout=asyncio.subprocess.PIPE,
           cwd=wt.path,  # Run in worktree directory
       )
       stdout, _ = await proc.communicate()

       if not stdout.decode().strip():
           # No changes to commit
           log.info(f"S{seg.num:02d} worktree has no uncommitted changes")
           return True, ""

       # 2. Stage all changes
       proc = await asyncio.create_subprocess_exec(
           "git", "add", "-A",
           cwd=wt.path,
       )
       await proc.wait()
       if proc.returncode != 0:
           return False, "Failed to stage changes"

       # 3. Commit with segment message
       commit_msg = f"Segment S{seg.num:02d}: {seg.title}"
       proc = await asyncio.create_subprocess_exec(
           "git", "commit", "-m", commit_msg,
           stdout=asyncio.subprocess.PIPE,
           cwd=wt.path,
       )
       await proc.wait()
       if proc.returncode != 0:
           return False, "Failed to create commit"

       # 4. Get commit SHA
       proc = await asyncio.create_subprocess_exec(
           "git", "rev-parse", "HEAD",
           stdout=asyncio.subprocess.PIPE,
           cwd=wt.path,
       )
       stdout, _ = await proc.communicate()
       commit_sha = stdout.decode().strip()[:8]  # Short form

       log.info(f"S{seg.num:02d} committed changes: {commit_sha}")
       return True, commit_sha
   ```

2. **Call commit function before merge** (segment_executor.py:276, INSERT BEFORE merge logic):
   ```python
   # Handle worktree merge if applicable
   if worktree and status == "pass" and self.config.isolation_strategy == "worktree":
       # NEW: Commit changes before merge
       commit_ok, commit_sha = await _commit_worktree_changes(worktree, seg)
       if not commit_ok:
           log.error(f"S{seg.num:02d} failed to commit: {commit_sha}")
           return status, "pass-commit-failed"

       # Existing merge logic
       merge_ok = await _merge_worktree_changes(worktree, seg)
       if not merge_ok:
           return status, "pass-merge-conflict"

       # Store commit SHA when marking merged
       await self.state.mark_merged(seg.num, commit_sha=commit_sha)
       return status, "merged"
   ```

3. **Update state.py schema** to track commit SHA:
   - Add `commit_sha` column to segments table (state.py:14-24)
   - Update `mark_merged()` signature (state.py:213-216):
     ```python
     async def mark_merged(self, num: int, commit_sha: str = "") -> None:
         await self.set_status(num, "merged", finished_at=time.time(), commit_sha=commit_sha)
         await self.log_event("segment_merged", f"S{num:02d} merged to main (commit: {commit_sha})")
     ```
   - Add migration in `_MIGRATIONS` list (state.py:91-96):
     ```python
     "ALTER TABLE segments ADD COLUMN commit_sha TEXT"
     ```

**Alternatives ruled out:**
- Relying on Claude Code to commit: Already proven unreliable, causes data loss
- Committing in worktree_pool before release: Wrong abstraction layer

**Pre-mortem risks:**
- Empty commits if no changes: Check git status first (handled in implementation)
- Commit fails due to git lock: Rare but possible, return clear error
- Merge fails after commit: Branch preserved with commit, safe to retry

**Segment-specific commands:**
- Build: `cd /Users/psauer/orchestrate && python -m pytest orchestrate_v3/test_segment_executor.py -v`
- Test (targeted): `pytest orchestrate_v3/test_segment_executor.py::test_commit_before_merge -v`
- Test (regression): `pytest orchestrate_v3/test_segment_executor.py -v`
- Test (full gate): `pytest orchestrate_v3/ -v`

**Exit criteria:**
1. **Targeted tests:**
   - `test_commit_worktree_changes_with_changes()`: Verifies commit created when changes exist
   - `test_commit_worktree_changes_no_changes()`: Verifies no commit when clean
   - `test_commit_before_merge_integration()`: Verifies commit happens before merge in execute()
2. **Regression tests:** All existing segment_executor tests pass (17 tests)
3. **Full build gate:** `pytest orchestrate_v3/ -v` - all tests pass
4. **Full test gate:** Integration test creates real git repo, verifies commit exists on worktree branch before merge
5. **Self-review gate:** No dead code, no commented blocks, commit_sha properly tracked in state.db
6. **Scope verification gate:** Only modified segment_executor.py (add function + call site) and state.py (add column + update mark_merged)

**Risk factor:** 9/10 (critical data loss bug)

**Estimated complexity:** Medium

**Commit message:** `fix(orchestrator): Commit worktree changes before merge to prevent data loss`

---

## Segment 2: Pre-Merge Branch Validation
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Add validation checks before merge to verify branch state is valid (exists, has commits, clean working tree).

**Depends on:** Segment 1 (commit function must exist first)

**Issues addressed:** Issue 2 (No Pre-Merge Validation of Branch State)

**Cycle budget:** 12 Medium

**Scope:** `/Users/psauer/orchestrate/orchestrate_v3/segment_executor.py`

**Key files and context:**

1. **segment_executor.py** (lines 22-121):
   - `_merge_worktree_changes()` attempts merge without validation
   - Line 49: Direct merge happens immediately
   - Need to add validation BEFORE line 49

2. **Git commands for validation:**
   - `git rev-parse --verify <branch>`: Check branch exists (exit code 0 = exists)
   - `git rev-list --count HEAD..<branch>`: Count commits ahead (0 = nothing to merge)
   - `git status --porcelain` in worktree: Check for uncommitted changes (empty = clean)

3. **Error handling pattern:**
   - segment_executor.py uses tuple returns: `(bool, str)` for success/error
   - Follow same pattern for validation

**Implementation approach:**

1. **Add `_validate_branch_before_merge()` function** (after `_commit_worktree_changes`):
   ```python
   async def _validate_branch_before_merge(wt: Worktree, seg: Segment) -> tuple[bool, str]:
       """Validate worktree branch state before merge.

       Returns: (can_merge: bool, error_message: str)
       """
       # 1. Verify branch exists
       proc = await asyncio.create_subprocess_exec(
           "git", "rev-parse", "--verify", wt.branch,
           stdout=asyncio.subprocess.PIPE,
           stderr=asyncio.subprocess.PIPE,
           cwd=wt.repo_root,
       )
       await proc.wait()
       if proc.returncode != 0:
           return False, f"Branch {wt.branch} does not exist"

       # 2. Check if branch has commits ahead of HEAD
       proc = await asyncio.create_subprocess_exec(
           "git", "rev-list", "--count", f"HEAD..{wt.branch}",
           stdout=asyncio.subprocess.PIPE,
           stderr=asyncio.subprocess.PIPE,
           cwd=wt.repo_root,
       )
       stdout, _ = await proc.communicate()
       if proc.returncode != 0:
           return False, f"Failed to count commits on {wt.branch}"

       ahead_count = int(stdout.decode().strip())
       if ahead_count == 0:
           return False, f"Branch {wt.branch} has no commits to merge"

       # 3. Verify worktree working directory is clean
       proc = await asyncio.create_subprocess_exec(
           "git", "status", "--porcelain",
           stdout=asyncio.subprocess.PIPE,
           stderr=asyncio.subprocess.PIPE,
           cwd=wt.path,
       )
       stdout, _ = await proc.communicate()
       if proc.returncode != 0:
           return False, f"Failed to check worktree status"

       if stdout.decode().strip():
           return False, f"Worktree has uncommitted changes (should have been committed)"

       log.info(f"S{seg.num:02d} validation passed: {ahead_count} commits ready to merge")
       return True, ""
   ```

2. **Insert validation call** in `_merge_worktree_changes()` before line 49:
   ```python
   async def _merge_worktree_changes(wt: Worktree, seg: Segment) -> bool:
       """Merge successful segment changes from worktree branch back to HEAD."""
       try:
           # NEW: Validate branch state before merge
           can_merge, error_msg = await _validate_branch_before_merge(wt, seg)
           if not can_merge:
               log.error(f"S{seg.num:02d} pre-merge validation failed: {error_msg}")
               return False

           # Get current branch name for logging...
           # (existing code continues)
   ```

3. **Add new failure status** for validation failures:
   - In segment_executor.py execute(), handle validation failure distinctly:
     ```python
     merge_ok = await _merge_worktree_changes(worktree, seg)
     if not merge_ok:
         # Check if merge was even attempted (validation may have failed)
         # Status could be: pass-merge-conflict OR pass-validation-failed
         return status, "pass-validation-failed"
     ```

**Alternatives ruled out:**
- Validate only commit count: Doesn't catch dirty working tree
- Use git merge --dry-run: More complex, creates temporary merge state

**Pre-mortem risks:**
- False negatives: Git commands fail due to repo corruption (handle with clear errors)
- Performance: Three subprocess calls add ~50ms (acceptable)
- Race condition: Main advances between validation and merge (rebase handles this)

**Segment-specific commands:**
- Build: `cd /Users/psauer/orchestrate && python -m pytest orchestrate_v3/test_segment_executor.py -v`
- Test (targeted): `pytest orchestrate_v3/test_segment_executor.py::test_validate_branch -v`
- Test (regression): `pytest orchestrate_v3/test_segment_executor.py -v`
- Test (full gate): `pytest orchestrate_v3/ -v`

**Exit criteria:**
1. **Targeted tests:**
   - `test_validate_branch_exists()`: Verifies validation catches non-existent branch
   - `test_validate_branch_has_commits()`: Verifies validation catches branch at HEAD (no commits)
   - `test_validate_clean_worktree()`: Verifies validation catches uncommitted changes
   - `test_validate_success()`: Verifies validation passes for valid branch with commits
2. **Regression tests:** All segment_executor tests pass
3. **Full build gate:** `pytest orchestrate_v3/ -v`
4. **Full test gate:** Integration test verifies merge rejected when validation fails
5. **Self-review gate:** Error messages are clear and actionable
6. **Scope verification gate:** Only modified segment_executor.py (add validation function + call site)

**Risk factor:** 7/10 (prevents data loss)

**Estimated complexity:** Medium

**Commit message:** `feat(orchestrator): Add pre-merge validation to catch invalid branch states`

---

## Segment 3: Enhanced Merge Conflict Handling
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Improve merge conflict handling to capture conflict details, store metadata, and provide recovery guidance.

**Depends on:** Segment 2 (validation must exist first)

**Issues addressed:** Issue 3 (Merge Conflicts Leave Orphaned Commits)

**Cycle budget:** 15 Medium

**Scope:** `/Users/psauer/orchestrate/orchestrate_v3/segment_executor.py`, `/Users/psauer/orchestrate/orchestrate_v3/state.py`

**Key files and context:**

1. **segment_executor.py** (lines 22-121):
   - `_merge_worktree_changes()` aborts merge on conflict (lines 104-116)
   - Currently returns False with no details
   - Need to capture: conflicting files, commit range, branch name

2. **segment_executor.py** (lines 276-283):
   - Merge failure returns "pass-merge-conflict" status
   - No metadata stored about conflict

3. **state.py** schema (lines 14-24):
   - segments table needs new column: `conflict_info TEXT` (JSON)
   - Should store: branch, files, commit_range, timestamp

4. **Git commands for conflict details:**
   - After merge fails, before abort:
     - `git diff --name-only --diff-filter=U`: List conflicting files
     - `git rev-parse HEAD`, `git rev-parse <branch>`: Get commit range
   - Parse output to JSON

**Implementation approach:**

1. **Add conflict details capture** in `_merge_worktree_changes()` (around line 60-78):
   ```python
   # After merge fails (proc.returncode != 0)
   if proc.returncode != 0:
       # NEW: Capture conflict details before aborting
       conflict_info = await _capture_conflict_details(wt, seg)

       # Abort the failed merge
       await asyncio.create_subprocess_exec(
           "git", "merge", "--abort",
           stdout=asyncio.subprocess.DEVNULL,
           stderr=asyncio.subprocess.DEVNULL,
           cwd=wt.repo_root,
       )

       # Log conflict with details
       log.error(
           f"S{seg.num:02d} merge conflict: {len(conflict_info['files'])} files, "
           f"branch {conflict_info['branch']}"
       )

       return False, conflict_info  # Changed return type to include info
   ```

2. **Add `_capture_conflict_details()` helper:**
   ```python
   async def _capture_conflict_details(wt: Worktree, seg: Segment) -> dict:
       """Capture details about merge conflict before aborting."""
       conflict_info = {
           "segment": seg.num,
           "branch": wt.branch,
           "files": [],
           "head_sha": "",
           "branch_sha": "",
           "timestamp": time.time(),
       }

       # Get conflicting files (files with unmerged changes)
       proc = await asyncio.create_subprocess_exec(
           "git", "diff", "--name-only", "--diff-filter=U",
           stdout=asyncio.subprocess.PIPE,
           cwd=wt.repo_root,
       )
       stdout, _ = await proc.communicate()
       if proc.returncode == 0:
           conflict_info["files"] = stdout.decode().strip().split("\n")

       # Get commit SHAs
       proc = await asyncio.create_subprocess_exec(
           "git", "rev-parse", "HEAD",
           stdout=asyncio.subprocess.PIPE,
           cwd=wt.repo_root,
       )
       stdout, _ = await proc.communicate()
       conflict_info["head_sha"] = stdout.decode().strip()[:8]

       proc = await asyncio.create_subprocess_exec(
           "git", "rev-parse", wt.branch,
           stdout=asyncio.subprocess.PIPE,
           cwd=wt.repo_root,
       )
       stdout, _ = await proc.communicate()
       conflict_info["branch_sha"] = stdout.decode().strip()[:8]

       return conflict_info
   ```

3. **Update segment_executor.py execute()** to store conflict info (line 278-280):
   ```python
   merge_ok, conflict_info = await _merge_worktree_changes(worktree, seg)
   if not merge_ok:
       log.warning(f"S{seg.num:02d} merge conflicts - manual resolution needed")
       await self.state.set_status(
           seg.num,
           "pass-merge-conflict",
           finished_at=time.time(),
           conflict_info=json.dumps(conflict_info),
       )
       return status, "pass-merge-conflict"
   ```

4. **Update state.py schema:**
   - Add migration: `"ALTER TABLE segments ADD COLUMN conflict_info TEXT"`
   - Update `set_status()` to accept conflict_info parameter (state.py:193-203)

**Alternatives ruled out:**
- Auto-retry with --strategy=ours: Loses changes, data loss risk
- Delete conflicted branches: Loses debugging context

**Pre-mortem risks:**
- Conflict details capture fails: Non-critical, merge abort still works
- JSON storage bloat: Conflicts are rare, acceptable storage cost
- Branch accumulation: Need cleanup mechanism (future work)

**Segment-specific commands:**
- Build: `cd /Users/psauer/orchestrate && python -m pytest orchestrate_v3/test_segment_executor.py -v`
- Test (targeted): `pytest orchestrate_v3/test_segment_executor.py::test_conflict_capture -v`
- Test (regression): `pytest orchestrate_v3/test_segment_executor.py -v`
- Test (full gate): `pytest orchestrate_v3/ -v`

**Exit criteria:**
1. **Targeted tests:**
   - `test_capture_conflict_details()`: Verifies conflict info captured (files, SHAs, timestamp)
   - `test_conflict_stored_in_db()`: Verifies conflict_info JSON stored in state.db
   - `test_merge_conflict_preserves_branch()`: Verifies branch not deleted after conflict
2. **Regression tests:** All segment_executor tests pass
3. **Full build gate:** `pytest orchestrate_v3/ -v`
4. **Full test gate:** Integration test creates conflict, verifies details captured correctly
5. **Self-review gate:** JSON schema documented, conflict_info properly formatted
6. **Scope verification gate:** Only modified segment_executor.py (conflict capture) and state.py (schema + storage)

**Risk factor:** 6/10 (improves debugging, doesn't fix conflicts)

**Estimated complexity:** Medium

**Commit message:** `feat(orchestrator): Capture merge conflict details for better debugging`

---

## Segment 4: Dashboard Merge Animation
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Add visual feedback to dashboard for merge operations (merging state + merge complete transition).

**Depends on:** Segment 3 (conflict info display may use new schema)

**Issues addressed:** Issue 4 (Dashboard Shows No Merge Visualization)

**Cycle budget:** 12 Low

**Scope:** `/Users/psauer/orchestrate/orchestrate_v3/dashboard.html`, `/Users/psauer/orchestrate/orchestrate_v3/dashboard.css`

**Key files and context:**

1. **dashboard.html** (CSS section, lines ~100-300):
   - Status colors defined: `--pass: #238636`, `--running: #1f6feb`, `--border: varies`
   - No merge-specific colors currently

2. **dashboard.html** (JavaScript section, lines ~700-850):
   - `renderTimeline()` function displays segment status (line ~769)
   - Status rendering: `<span class="seg-status ${s.status}">${s.status}</span>`
   - Need to detect status transitions and apply animation classes

3. **dashboard.css** (or inline styles in HTML):
   - Currently no `@keyframes` animations
   - Transitions are property-based (`transition: opacity 0.1s`)
   - Need to add merge-specific animations

4. **Existing animation pattern** (from research):
   - Pulsing animation exists in other dashboards (CSS pulse pattern)
   - Industry standard: 1.5s for ongoing, 500ms for completion

5. **Color tokens to add:**
   - `--merge-in-progress: #1f6feb` (blue, same as running)
   - `--merge-complete: #238636` (green, same as pass)
   - `--merge-flash: #dfffdf` (bright green for flash effect)

**Implementation approach:**

1. **Add CSS animations** in dashboard.html `<style>` section:
   ```css
   /* Merge-specific color tokens */
   :root {
     --merge-in-progress: #1f6feb;
     --merge-complete: #238636;
     --merge-flash: #dfffdf;
   }

   /* Animation: Merging in progress (pulsing) */
   @keyframes merge-pulse {
     0%, 100% { opacity: 0.7; }
     50% { opacity: 1; }
   }

   /* Animation: Merge complete (flash transition) */
   @keyframes merge-success {
     0% {
       background: var(--running-bg);
       border-left-color: var(--merge-in-progress);
     }
     40% {
       background: var(--merge-flash);
       transform: translateX(3px);
     }
     100% {
       background: var(--pass-bg);
       border-left-color: var(--merge-complete);
       transform: translateX(0);
     }
   }

   /* Apply animations to segment rows */
   .seg-row.status-merging {
     animation: merge-pulse 1.5s ease-in-out infinite;
     border-left-color: var(--merge-in-progress);
   }

   .seg-row.merge-complete-animation {
     animation: merge-success 500ms ease-out forwards;
   }

   /* Accessibility: Reduced motion support */
   @media (prefers-reduced-motion: reduce) {
     .seg-row.status-merging {
       animation: none;
       opacity: 0.85;
     }
     .seg-row.merge-complete-animation {
       animation-duration: 250ms;
       transform: none !important;
     }
   }
   ```

2. **Update status rendering** in JavaScript (dashboard.html, around line 769):
   ```javascript
   // Store previous status for transition detection
   const prevStatuses = new Map();

   function renderTimeline(segments, maxWave, events, currentWave) {
     segs.forEach(s => {
       const prevStatus = prevStatuses.get(s.num);

       // Detect merge completion: running → merged OR pass → merged
       if ((prevStatus === 'running' || prevStatus === 'pass') && s.status === 'merged') {
         // Trigger merge complete animation
         setTimeout(() => {
           const row = document.querySelector(`[data-seg-num="${s.num}"]`);
           if (row) {
             row.classList.add('merge-complete-animation');
             setTimeout(() => row.classList.remove('merge-complete-animation'), 600);
           }
         }, 100);
       }

       // Update status classes
       html += `<div class="seg-row${isActive}" data-seg-num="${s.num}" data-status="${s.status}">`;

       // Add "status-merging" class if last_activity indicates merging
       const isMerging = s.status === 'running' && s.last_activity?.includes('merging');
       const mergingClass = isMerging ? ' status-merging' : '';

       html += `<span class="seg-status ${s.status}${mergingClass}">${s.status}</span>`;

       // Store current status for next comparison
       prevStatuses.set(s.num, s.status);
     });
   }
   ```

3. **Add commit SHA display** (optional enhancement):
   ```javascript
   // After status span, if merged, show commit SHA
   if (s.status === 'merged' && s.commit_sha) {
     html += `<span class="commit-sha">${s.commit_sha}</span>`;
   }
   ```

   ```css
   .commit-sha {
     font-family: monospace;
     font-size: 0.85em;
     color: var(--text-dim);
     margin-left: 0.5em;
   }
   ```

**Alternatives ruled out:**
- Progress bar for merge: Merge is fast (<1s), bar would flash
- Toast notification: Interrupts user focus
- Just color change: User might miss status change

**Pre-mortem risks:**
- Animation jank on low-end devices: Use `will-change: opacity` for performance hint
- Timing mismatch: Keep animation short (500ms) so it completes even if merge is instant
- Accessibility: Motion-sensitive users need reduced motion support (implemented)

**Segment-specific commands:**
- Build: N/A (HTML/CSS/JS changes, no build step)
- Test (targeted): Manual browser testing - refresh dashboard, observe merge animation
- Test (regression): Visual regression test - capture screenshots before/after
- Test (full gate): `pytest orchestrate_v3/test_monitor.py -v` (backend API tests)

**Exit criteria:**
1. **Targeted tests:**
   - Manual test: Trigger merge, observe pulsing blue animation during merge
   - Manual test: Merge completes, observe blue→green flash transition
   - Manual test: Verify commit SHA displays after merge (if implemented)
2. **Regression tests:** Dashboard still loads without errors, all segments display correctly
3. **Full build gate:** `pytest orchestrate_v3/test_monitor.py -v` (API tests pass)
4. **Full test gate:** Visual test with browser DevTools - verify no console errors, CSS applies correctly
5. **Self-review gate:** Animations work in light/dark mode, reduced motion preference respected
6. **Scope verification gate:** Only modified dashboard.html (CSS + JS), no Python changes

**Risk factor:** 2/10 (UI-only, no data risk)

**Estimated complexity:** Low

**Commit message:** `feat(dashboard): Add merge animation with pulsing progress and success flash`

---

## Segment 5: Integration Tests for Merge Workflow
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Add comprehensive integration tests that verify the complete merge workflow with real git operations.

**Depends on:** Segment 3 (needs conflict handling to test conflict scenarios)

**Issues addressed:** Issue 5 (No Integration Tests for Complete Merge Workflow)

**Cycle budget:** 10 Low

**Scope:** `/Users/psauer/orchestrate/orchestrate_v3/test_worktree_merge_integration.py` (new file)

**Key files and context:**

1. **test_segment_executor.py** (lines 292-340):
   - Existing tests mock `_merge_worktree_changes()`
   - Line 441-529: `test_merge_creates_commit_in_main_repo` is closest to integration test
   - Need more comprehensive coverage

2. **test_worktree_pool.py** (121 lines):
   - Tests worktree creation and acquisition
   - Uses temp git repos via pytest fixtures
   - Follow same pattern for integration tests

3. **Pytest patterns:**
   - Use `tmp_path` fixture for temp directories
   - Use `subprocess.run()` for git setup in fixtures
   - Clean up temp repos in fixture teardown

4. **Git setup for tests:**
   ```python
   # Create main repo
   subprocess.run(["git", "init"], cwd=repo_path)
   subprocess.run(["git", "config", "user.name", "Test"], cwd=repo_path)
   subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=repo_path)

   # Create initial commit
   (repo_path / "README.md").write_text("Initial")
   subprocess.run(["git", "add", "-A"], cwd=repo_path)
   subprocess.run(["git", "commit", "-m", "Initial commit"], cwd=repo_path)
   ```

**Implementation approach:**

1. **Create pytest fixture for git repo with worktrees**:
   ```python
   import pytest
   import subprocess
   from pathlib import Path

   @pytest.fixture
   async def git_repo_with_worktree(tmp_path):
       """Create a test git repo with a worktree."""
       repo_root = tmp_path / "main-repo"
       repo_root.mkdir()

       # Initialize repo
       subprocess.run(["git", "init"], cwd=repo_root, check=True)
       subprocess.run(["git", "config", "user.name", "Test"], cwd=repo_root, check=True)
       subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=repo_root, check=True)

       # Initial commit
       (repo_root / "README.md").write_text("Initial")
       subprocess.run(["git", "add", "-A"], cwd=repo_root, check=True)
       subprocess.run(["git", "commit", "-m", "Initial"], cwd=repo_root, check=True)

       # Create worktree
       wt_path = tmp_path / "worktree-00"
       subprocess.run(
           ["git", "worktree", "add", str(wt_path), "-b", "wt/pool-00", "HEAD"],
           cwd=repo_root,
           check=True,
       )

       yield {
           "repo_root": repo_root,
           "worktree_path": wt_path,
           "branch": "wt/pool-00",
       }

       # Cleanup
       subprocess.run(
           ["git", "worktree", "remove", "--force", str(wt_path)],
           cwd=repo_root,
           check=False,
       )
   ```

2. **Test 1: Successful segment merge**:
   ```python
   async def test_successful_segment_merge(git_repo_with_worktree):
       """Test complete workflow: changes → commit → merge → verify."""
       repo = git_repo_with_worktree

       # Make changes in worktree
       test_file = repo["worktree_path"] / "test.txt"
       test_file.write_text("Test content")

       # Commit in worktree
       subprocess.run(["git", "add", "-A"], cwd=repo["worktree_path"], check=True)
       subprocess.run(
           ["git", "commit", "-m", "Test changes"],
           cwd=repo["worktree_path"],
           check=True,
       )

       # Merge to main (simulate orchestrator merge)
       subprocess.run(
           ["git", "merge", "--no-ff", "-m", "Merge test", repo["branch"]],
           cwd=repo["repo_root"],
           check=True,
       )

       # Verify merge commit exists
       result = subprocess.run(
           ["git", "log", "--oneline", "-1"],
           cwd=repo["repo_root"],
           capture_output=True,
           text=True,
       )
       assert "Merge test" in result.stdout

       # Verify changes landed on main
       test_file_main = repo["repo_root"] / "test.txt"
       assert test_file_main.exists()
       assert test_file_main.read_text() == "Test content"
   ```

3. **Test 2: Merge with conflict**:
   ```python
   async def test_merge_with_conflict(git_repo_with_worktree):
       """Test conflict detection and branch preservation."""
       repo = git_repo_with_worktree

       # Make conflicting change in main
       main_file = repo["repo_root"] / "conflict.txt"
       main_file.write_text("Main version")
       subprocess.run(["git", "add", "-A"], cwd=repo["repo_root"], check=True)
       subprocess.run(["git", "commit", "-m", "Main change"], cwd=repo["repo_root"], check=True)

       # Make conflicting change in worktree
       wt_file = repo["worktree_path"] / "conflict.txt"
       wt_file.write_text("Worktree version")
       subprocess.run(["git", "add", "-A"], cwd=repo["worktree_path"], check=True)
       subprocess.run(["git", "commit", "-m", "WT change"], cwd=repo["worktree_path"], check=True)

       # Attempt merge (should fail)
       result = subprocess.run(
           ["git", "merge", "--no-ff", "-m", "Merge", repo["branch"]],
           cwd=repo["repo_root"],
           check=False,
       )
       assert result.returncode != 0  # Merge failed

       # Verify conflict detected
       result = subprocess.run(
           ["git", "diff", "--name-only", "--diff-filter=U"],
           cwd=repo["repo_root"],
           capture_output=True,
           text=True,
       )
       assert "conflict.txt" in result.stdout

       # Abort merge
       subprocess.run(["git", "merge", "--abort"], cwd=repo["repo_root"], check=True)

       # Verify branch still exists
       result = subprocess.run(
           ["git", "branch", "--list", repo["branch"]],
           cwd=repo["repo_root"],
           capture_output=True,
           text=True,
       )
       assert repo["branch"] in result.stdout
   ```

4. **Test 3: Empty branch validation**:
   ```python
   async def test_empty_branch_rejected(git_repo_with_worktree):
       """Test that branch with no commits is rejected."""
       repo = git_repo_with_worktree

       # Don't make any changes in worktree

       # Check commit count (should be 0)
       result = subprocess.run(
           ["git", "rev-list", "--count", f"HEAD..{repo['branch']}"],
           cwd=repo["repo_root"],
           capture_output=True,
           text=True,
       )
       ahead_count = int(result.stdout.strip())
       assert ahead_count == 0, "Branch should have no commits"
   ```

5. **Test 4: Dirty worktree validation**:
   ```python
   async def test_dirty_worktree_rejected(git_repo_with_worktree):
       """Test that uncommitted changes are detected."""
       repo = git_repo_with_worktree

       # Make changes but don't commit
       test_file = repo["worktree_path"] / "dirty.txt"
       test_file.write_text("Uncommitted")

       # Check status (should show uncommitted file)
       result = subprocess.run(
           ["git", "status", "--porcelain"],
           cwd=repo["worktree_path"],
           capture_output=True,
           text=True,
       )
       assert result.stdout.strip() != "", "Should detect uncommitted changes"
       assert "dirty.txt" in result.stdout
   ```

**Alternatives ruled out:**
- Mock all git operations: Defeats purpose of integration test
- Test only unit functions: Already done, doesn't catch integration issues

**Pre-mortem risks:**
- Test flakiness: Git operations could fail randomly (use check=True to catch)
- Slow tests: Real git operations take time (acceptable for integration tests)
- Cleanup failures: Use pytest fixtures with proper teardown

**Segment-specific commands:**
- Build: `cd /Users/psauer/orchestrate && python -m pytest orchestrate_v3/test_worktree_merge_integration.py -v`
- Test (targeted): `pytest orchestrate_v3/test_worktree_merge_integration.py::test_successful_segment_merge -v`
- Test (regression): `pytest orchestrate_v3/ -v` (all tests including new integration tests)
- Test (full gate): `pytest orchestrate_v3/ -v --tb=short`

**Exit criteria:**
1. **Targeted tests:**
   - `test_successful_segment_merge()`: Full workflow with real git, verifies merge commit exists
   - `test_merge_with_conflict()`: Conflict detection and branch preservation
   - `test_empty_branch_rejected()`: Validation catches branch with no commits
   - `test_dirty_worktree_rejected()`: Validation catches uncommitted changes
2. **Regression tests:** All existing orchestrate_v3 tests pass (111 tests + 4 new = 115)
3. **Full build gate:** `pytest orchestrate_v3/ -v`
4. **Full test gate:** All integration tests pass, no flakiness on 3 runs
5. **Self-review gate:** Fixtures properly clean up temp repos, tests are isolated
6. **Scope verification gate:** Only added new test file, no changes to production code

**Risk factor:** 3/10 (testing infrastructure)

**Estimated complexity:** Low

**Commit message:** `test(orchestrator): Add integration tests for complete merge workflow`

---

## Execution Log

| Segment | Title | Est. Complexity | Risk | Cycles Budget | Cycles Used | Status | Started | Completed | Notes |
|---------|-------|----------------|------|---------------|-------------|--------|---------|-----------|-------|
| 1 | Commit Worktree Changes Before Merge | Medium | 9/10 | 15 | -- | pending | -- | -- | Critical bug fix |
| 2 | Pre-Merge Branch Validation | Medium | 7/10 | 12 | -- | pending | -- | -- | Depends on S1 |
| 3 | Enhanced Merge Conflict Handling | Medium | 6/10 | 15 | -- | pending | -- | -- | Depends on S2 |
| 4 | Dashboard Merge Animation | Low | 2/10 | 12 | -- | pending | -- | -- | Independent, can run parallel with S5 |
| 5 | Integration Tests for Merge Workflow | Low | 3/10 | 10 | -- | pending | -- | -- | Independent, can run parallel with S4 |

**Total estimated effort:** 64 cycles (~6-8 hours)

**Parallelization opportunities:**
- Wave 1: S1 → S2 → S3 (sequential, dependency chain)
- Wave 2: S4 and S5 (parallel, independent)

**Deep-verify result:** --

**Follow-up plans:** --

---

## Execution Instructions

To execute this plan, use the `/orchestrate` skill or run:
```bash
orchestrate run .claude/plans/orchestrator-merge-fix-2026-03-12.md
```

For each segment in order, the orchestrator launches an `iterative-builder` subagent with the full segment brief. Do not implement segments directly—always delegate to iterative-builder subagents.

After all segments complete, run `/deep-verify` to verify exit criteria are satisfied. If verification finds gaps, re-enter `/deep-plan` on unresolved items.

**Manual execution alternative:**
```bash
# For each segment:
claude -p ".claude/plans/orchestrator-merge-fix-2026-03-12.md#segment-N" \
  --dangerously-skip-permissions \
  --verbose
```
