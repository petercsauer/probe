# Orchestrator v2: Merge Fix & Recovery Agent

**Created:** 2026-03-11
**Status:** APPROVED
**Total Segments:** 3
**Risk Budget:** 14/30 (moderate)
**Estimated Cycles:** 40

---

## Metadata

- **Goal:** Fix worktree merge failures on detached HEAD and add basic automated recovery for cascade failures
- **Issues Addressed:**
  - Issue #0: Worktree merge skipped when orchestrator runs in detached HEAD
  - Issue #1: Segments stuck at PARTIAL/BLOCKED even after workspace is fixed
- **Execution Strategy:** Sequential dependency-order (S1 → S2 → S3)
- **Parallelization:** None (linear dependencies)

---

## Issue Analysis

### Issue #0: Worktree Merge Failure in Detached HEAD

**Core Problem:** When the orchestrator runs from Claude Code (detached HEAD state), successful segment changes in worktrees are never merged back to the main repository. Each subsequent segment resets to a stale main branch, defeating the purpose of worktree isolation. Segments S9, S12, and S18 were blocked because S10's fixes never propagated despite S10 passing.

**Root Cause:** Lines 238-241 in `scripts/orchestrate_v2/__main__.py` check if current HEAD is a branch before merging. When `git symbolic-ref --short HEAD` fails (returns error code for detached HEAD), the function logs a warning, returns `True` (treating it as success), and skips the merge entirely:
```python
if not current_branch:
    log.warning("Skipping merge for S%02d - not on a branch (detached HEAD)", seg.num)
    return True  # Don't fail the segment
```

**Proposed Fix:** Remove the detached HEAD check and perform the merge directly. Git supports merging on detached HEAD - it creates a merge commit and advances HEAD forward. This preserves all merge history and is the simplest solution.

**Existing Solutions Evaluated:** N/A - This is git command usage, not a library integration problem.

**Alternatives Considered:**
1. Temporary branch strategy - Rejected: cleanup overhead, unnecessary complexity
2. Update-ref approach - Rejected: requires computing merge commit SHA, overly complex
3. Require branch - Rejected: defeats Claude Code integration, forces manual workflow

**Pre-Mortem — What Could Go Wrong:**
- HEAD advances but remains detached, could confuse users expecting named branch
- Merge conflicts still require manual resolution (but existing conflict handling works)
- If orchestrator crashes mid-merge, detached HEAD might be in unexpected state

**Risk Factor:** 3/10 (isolated change, well-tested merge logic already exists)

**Evidence for Optimality:**
1. **Codebase evidence:** The comment "This is expected during orchestrator execution - skip merge for now" (line 237) indicates this was a temporary workaround
2. **Git official docs:** Detached HEAD is fully supported for merging operations
3. **Project conventions:** Git workflow in CONTRIBUTING.md uses standard branch-and-merge, no restrictions on detached HEAD
4. **External evidence:** CI/CD systems routinely merge on detached HEAD (GitHub Actions, GitLab CI)

**Blast Radius:**
- Direct changes: `scripts/orchestrate_v2/__main__.py` lines 236-241 (remove 6 lines)
- Potential ripple: None

---

### Issue #1: Basic Automated Recovery Agent

**Core Problem:** Segments stuck at PARTIAL/BLOCKED status even after workspace issues are fixed. Operators must manually retry each segment. No automated detection of when workspace health is restored or which segments were "victims" of cascade failures. This wastes operator attention and delays completion.

**Root Cause:** Orchestrator lacks workspace health monitoring between segment execution and wave completion. When S10 completed with broken stubs, S12/S18 failed, and orchestrator marked them PARTIAL/BLOCKED. Even after S10 was fixed, the orchestrator doesn't re-evaluate whether blocked segments should be retried. Statuses are sticky - only manual intervention can reset them.

**Proposed Fix:** Add lightweight recovery agent that runs after each wave:
1. Detect workspace health: Run `cargo check --workspace --message-format=json`
2. Compare to prior state: If wave had failures but workspace now passes, candidates exist
3. Identify victims: Check builder reports for "pre-existing errors", "blocked by S{X}" markers
4. Auto-retry victims: Call `state.reset_for_retry()` and re-run in mini-wave

**Existing Solutions Evaluated:**
1. **tenacity** (https://pypi.org/project/tenacity/) - Maintenance: Active, 50M+ downloads/month, License: Apache 2.0
   - **Adapt:** Use for improved retry logic with backoff, but not for cascade detection

2. **pytest-retry** - **Reject:** Not applicable to orchestration

3. **circus/supervisor** - **Reject:** Designed for daemons, not batch orchestration

**Alternatives Considered:**
1. Pre-flight checks only - Rejected: doesn't help with cascade failures during wave
2. Continuous monitoring - Rejected: overhead too high
3. Manual recovery guide only - Rejected: still requires manual work

**Pre-Mortem — What Could Go Wrong:**
- False positives: Agent retries segments that legitimately failed
- False negatives: Agent misses cascade patterns
- Retry loop: Auto-retry causes new failures (mitigate with attempt limits)
- Detection fragility: String matching in logs brittle to format changes

**Risk Factor:** 5/10 (touches retry logic, introduces detection heuristics)

**Evidence for Optimality:**
1. **Codebase evidence:** Existing `_extract_status()` in runner.py does pattern-based error detection
2. **Project conventions:** `.claude/commands/iterative-debugger.md` establishes protocol for programmatic debugging
3. **External evidence:** CI/CD systems use post-build health checks to trigger retries
4. **Existing solutions:** `tenacity` library widely adopted (50M+ downloads)

**Blast Radius:**
- Direct changes: Add `recovery.py` module, modify `__main__.py`, add config section
- Potential ripple: New state transitions, recovery events in dashboard

---

## Segment Briefs

### Segment S1: Fix Detached HEAD Merge

**Goal:** Enable worktree merges when orchestrator runs in detached HEAD state

**Depends on:** None

**Issues addressed:** Issue #0

**Cycle budget:** 10 (Low complexity)

**Scope:** Git merge function in orchestrator main module

**Key files and context:**
- `scripts/orchestrate_v2/__main__.py` lines 221-271: `_merge_worktree_changes()` function
- Currently skips merge when `git symbolic-ref --short HEAD` fails (lines 236-241)
- Must preserve existing conflict handling (lines 253-264)
- Must maintain "pass-merge-conflict" status for manual intervention
- Git operations run on main repository (not worktree paths)
- Line 504 TODO: detect current branch instead of hardcoding "main"

**Implementation approach:**
1. Remove detached HEAD check (lines 227-241)
2. Keep existing `git merge --no-ff` command (lines 243-251)
3. Keep existing conflict detection and abort logic (lines 253-264)
4. Update logging to indicate detached HEAD merge is proceeding
5. Test with orchestrator running from detached HEAD
6. Test with intentional merge conflict to verify abort works

**Alternatives ruled out:**
- Temporary branch creation: unnecessary complexity
- Update-ref approach: overly low-level
- Documenting requirement: defeats Claude Code workflow

**Pre-mortem risks:**
- Users might be confused by advancing detached HEAD
- Need clear logging when HEAD advances without branch name
- Test that subsequent segments get merged changes

**Segment-specific commands:**
- Build: `python -c "import scripts.orchestrate_v2.__main__"`
- Test (targeted): `pytest scripts/test_merge_integration.py -v`
- Test (regression): `pytest scripts/test_*.py -v`
- Test (full gate): `python -m scripts.orchestrate_v2 dry-run .claude/plans/phase3-tui-evolution`

**Exit criteria:**
1. Targeted test: `test_merge_integration.py::test_detached_head_merge` passes
2. Targeted test: `test_merge_conflict.py::test_conflict_abort` passes
3. Regression tests: All existing orchestrate_v2 tests pass
4. Full build gate: `python -c "import scripts.orchestrate_v2.__main__"` succeeds
5. Manual verification: Run orchestrator from detached HEAD, check git log shows merge commits
6. Self-review gate: No dead code, no commented blocks, no TODOs
7. Scope verification gate: Only `__main__.py` modified

**Risk factor:** 3/10
**Estimated complexity:** Low
**Commit message:** `fix(orchestrate): enable worktree merges on detached HEAD`

---

### Segment S2: Recovery Agent Infrastructure

**Goal:** Add recovery agent module with workspace health checking and victim detection

**Depends on:** S1 (so retried segments get merged changes)

**Issues addressed:** Issue #1 (detection only, no auto-retry yet)

**Cycle budget:** 15 (Medium complexity)

**Scope:** New recovery module with health check and cascade detection logic

**Key files and context:**
- Create `scripts/orchestrate_v2/recovery.py` (new file, ~250 lines)
- Reference `scripts/orchestrate_v2/state.py`: StateDB interface for segment queries
- Reference `scripts/orchestrate_v2/runner.py`: Status detection patterns to reuse (lines 150-174)
- Reference `scripts/orchestrate_v2/config.py`: Configuration structure
- Must use `cargo check --workspace --message-format=json` for health checks
- Must parse builder reports from `logs/S{N:02d}.log` files
- Must detect patterns: "pre-existing errors", "my code is correct", "blocked by S{X}"

**Implementation approach:**
1. Create `RecoveryAgent` class with `__init__(state: StateDB, config: OrchestrateConfig)`
2. Add `async def check_workspace_health() -> tuple[bool, list[str]]` method:
   - Runs `cargo check --workspace --message-format=json`
   - Parses JSON output for errors
   - Returns (healthy: bool, error_list: list)
3. Add `async def identify_cascade_victims(wave_segments: list) -> list[int]` method:
   - Reads builder reports from logs/
   - Searches for victim markers
   - Returns list of segment numbers to retry
4. Add config class `RecoveryConfig` with fields: `enabled`, `health_check_timeout`, `victim_markers`
5. Create test file `scripts/orchestrate_v2/test_recovery.py` with unit tests
6. Test with mocked cargo output and real builder report logs

**Alternatives ruled out:**
- Using `tenacity` for retry: Too early, this segment just detects
- Pre-flight checks: Addressed in future work, this focuses on post-wave recovery
- Continuous monitoring: Too heavyweight

**Pre-mortem risks:**
- Cargo check might timeout on large workspaces (add configurable timeout)
- Builder report parsing brittle if format changes (test with real logs)
- False positive detection (be conservative - only retry if strong signal)

**Segment-specific commands:**
- Build: `python -c "from scripts.orchestrate_v2.recovery import RecoveryAgent"`
- Test (targeted): `pytest scripts/orchestrate_v2/test_recovery.py::TestWorkspaceHealth -v`
- Test (regression): `pytest scripts/orchestrate_v2/test_*.py -v`
- Test (full gate): `python -m scripts.orchestrate_v2.__main__ --help`

**Exit criteria:**
1. Targeted test: `test_recovery.py::test_workspace_health_pass` passes
2. Targeted test: `test_recovery.py::test_workspace_health_fail` passes
3. Targeted test: `test_recovery.py::test_identify_victims` passes
4. Regression tests: All existing orchestrate_v2 tests pass
5. Full build gate: Module imports without errors
6. Unit tests: 100% coverage of RecoveryAgent methods
7. Self-review gate: No dead code, docstrings complete
8. Scope verification gate: Only `recovery.py` and `test_recovery.py` added

**Risk factor:** 5/10
**Estimated complexity:** Medium
**Commit message:** `feat(orchestrate): add recovery agent with health checks and victim detection`

---

### Segment S3: Integrate Recovery Agent with Wave Execution

**Goal:** Call recovery agent after each wave to auto-retry victim segments

**Depends on:** S2 (recovery agent exists)

**Issues addressed:** Issue #1 (integration + auto-retry)

**Cycle budget:** 15 (Medium complexity)

**Scope:** Orchestrator main loop integration

**Key files and context:**
- `scripts/orchestrate_v2/__main__.py` lines 597-617: After wave completes, before gate
- `scripts/orchestrate_v2/config.py`: Add `[recovery]` section to OrchestrateConfig
- Must handle recovery failures gracefully (don't break orchestration)
- Must log recovery attempts to events table
- Must send notifications for recovery actions
- Recovery mini-wave should respect max_parallel limits
- Circuit breaker: Limit recovery attempts per segment (prevent loops)

**Implementation approach:**
1. Add `[recovery]` section to `config.py`:
   ```python
   recovery_enabled: bool = True
   recovery_max_attempts: int = 1  # Per segment per wave
   health_check_timeout: int = 120
   ```
2. In `__main__.py`, after wave results gathered (line 604):
   ```python
   if config.recovery_enabled and any(status in ('partial', 'blocked') for _, status in results):
       recovery = RecoveryAgent(state, config)
       healthy, errors = await recovery.check_workspace_health()
       if healthy:
           victims = await recovery.identify_cascade_victims(wave_segs)
           if victims:
               log.info("Recovery: workspace healthy, retrying %d victims: %s", len(victims), victims)
               recovery_results = await _run_recovery_wave(victims, ...)
   ```
3. Add `_run_recovery_wave()` function (similar to `_run_wave` but limited scope)
4. Track recovery attempts in state.db segment_attempts table
5. Add recovery events: `recovery_triggered`, `recovery_retry`, `recovery_complete`
6. Update `orchestrate.toml` example with `[recovery]` section

**Alternatives ruled out:**
- Running recovery before gate: Gate might fail for unrelated reasons
- Retrying all failed segments: Too broad
- Immediate retry without health check: Might retry into same broken state

**Pre-mortem risks:**
- Recovery wave could introduce new failures (limit with max_attempts)
- Infinite loop if recovery keeps triggering (circuit breaker required)
- Performance overhead of cargo check (acceptable if wave already has failures)
- Recovery might conflict with operator manual retry (check pending status first)

**Segment-specific commands:**
- Build: `python -c "import scripts.orchestrate_v2.__main__"`
- Test (targeted): `pytest scripts/test_orchestrate_recovery_integration.py -v`
- Test (regression): `pytest scripts/test_*.py -v`
- Test (full gate): `python -m scripts.orchestrate_v2 dry-run .claude/plans/phase3-tui-evolution`

**Exit criteria:**
1. Targeted test: `test_orchestrate_recovery_integration.py::test_recovery_triggers_on_partial` passes
2. Targeted test: `test_orchestrate_recovery_integration.py::test_recovery_retries_victims` passes
3. Targeted test: `test_orchestrate_recovery_integration.py::test_recovery_circuit_breaker` passes
4. Regression tests: All orchestrate_v2 tests pass
5. Integration test: Run orchestrator with intentional cascade failure, verify recovery
6. Full build gate: Orchestrator runs end-to-end with recovery enabled
7. Self-review gate: Recovery logging clear, no dead code
8. Scope verification gate: Changes only in `__main__.py`, `config.py`, `orchestrate.toml` example

**Risk factor:** 6/10
**Estimated complexity:** Medium
**Commit message:** `feat(orchestrate): integrate recovery agent for automatic victim retry`

---

## Execution Log

| Segment | Started | Completed | Status | Commit | Notes |
|---------|---------|-----------|--------|--------|-------|
| S1 | 2026-03-11 | 2026-03-11 | PASS | d0fd5af | Cycles: 5/10. Removed detached HEAD check, added test suite. |
| S2 | 2026-03-11 | 2026-03-11 | PASS | c48b94b | Cycles: 8/15. Added RecoveryAgent with health checks and victim detection. |
| S3 | 2026-03-11 | 2026-03-11 | PASS | f6aa039 | Cycles: 5/15. Integrated recovery agent into wave execution with circuit breaker. |

---

## Execution Instructions

**Recommended approach:**
```bash
# Execute segments sequentially using iterative-builder subagents
# For each segment S1-S3:
#   1. Launch iterative-builder with segment brief
#   2. Verify exit criteria
#   3. Commit changes
#   4. Update execution log
#   5. Proceed to next segment
```

**After all segments complete:**
- Run `/deep-verify` to validate all exit criteria
- Test end-to-end: Run orchestrator with recovery enabled on phase3-tui-evolution
- Manually trigger cascade failure and verify recovery agent kicks in
- Document recovery agent behavior in README or orchestrator docs
