# Orchestrator v2 Analysis & Fixes
**Date**: 2026-03-11
**Session**: phase3-tui-evolution
**Status**: 13 PASS, 7 UNKNOWN, 3 RUNNING, 1 PENDING, 1 PARTIAL, 1 BLOCKED

---

## Critical Issues Found

### 🔴 ISSUE #1: CLAUDECODE Environment Variable Leak (ROOT CAUSE)
**Impact**: Wave 4-5 segments failing with "unknown" status
**Severity**: CRITICAL - Blocks 7+ segments

**Root Cause**:
- The orchestrator inherits `CLAUDECODE=1` from the parent Claude Code session
- In `scripts/orchestrate_v2/runner.py:66`, `_build_env()` does:
  ```python
  env = dict(os.environ)  # ← Copies ALL env vars including CLAUDECODE
  env.update(config.auth_env)
  env.update(_resolve_isolation_env(seg_num, config))
  ```
- When spawning segment subprocesses (line 280), this environment is passed to the `claude` CLI
- Claude Code detects `CLAUDECODE=1` and refuses to start: "Claude Code cannot be launched inside another Claude Code session"
- The segment exits immediately with no output
- `_extract_status()` finds no "**Status:**" marker → defaults to "unknown"

**Evidence**:
```
Segment S13-17, S23-24 (Wave 4-5):
  status: "unknown"
  error: "Claude Code cannot be launched inside another Claude Code session..."
  attempts: 5 each (all failed the same way)
```

**Fix**:
```python
# In scripts/orchestrate_v2/runner.py, line 64-69:
def _build_env(seg_num: int, config: OrchestrateConfig) -> dict[str, str]:
    """Build environment dict: inherit shell + auth + isolation."""
    env = dict(os.environ)

    # CRITICAL FIX: Remove CLAUDECODE to prevent nested session detection
    env.pop('CLAUDECODE', None)

    env.update(config.auth_env)
    env.update(_resolve_isolation_env(seg_num, config))
    return env
```

---

### 🟡 ISSUE #2: Cascade Compilation Failures
**Impact**: Wave 3 segments blocked
**Severity**: HIGH - Prevents testing of 3 segments

**Root Cause**:
- Segment S10 (Export & Clipboard) left incomplete code stubs
- S12 (AI Explain Panel) and S18 (Live Capture Config UI) have functional code but can't compile workspace
- Both marked as PARTIAL/BLOCKED despite their code being correct

**Evidence** (from S12.log and S18.log):
```
S12: "All S12 exit criteria met for the AI panel code itself"
     "14 compilation errors in app.rs lines 1073-1188 (export/copy code from incomplete S10)"
S18: "My code has ZERO compilation errors. All errors are in code that predates this segment"
     "14 pre-existing workspace errors in app.rs (copy mode, AI panel integration)"
```

**Fix Options**:
1. **Quick**: Mark S12/S18 as PASS (their code is correct) and fix S10 in isolation
2. **Proper**: Retry S10 with explicit instruction to complete all stubs or revert partial work
3. **Best**: Add pre-segment build check to fail-fast on workspace errors before starting work

---

### 🟡 ISSUE #3: Inadequate Error Detection
**Impact**: 7 segments marked "unknown" instead of "failed"
**Severity**: MEDIUM - Prevents proper retry logic

**Root Cause**:
- `_extract_status()` only recognizes builder report markers: "**Status:** PASS/PARTIAL/BLOCKED"
- When segment crashes before producing output, no marker exists → "unknown"
- Retry logic doesn't handle "unknown" status properly

**Fix**:
```python
# In scripts/orchestrate_v2/runner.py, line 139-150:
def _extract_status(log_text: str) -> str:
    """Extract PASS/PARTIAL/BLOCKED from the builder report in log text."""
    # Check for explicit status markers first
    for marker in ("**Status:** PASS", "Status: PASS"):
        if marker in log_text:
            return "pass"
    for marker in ("**Status:** PARTIAL", "Status: PARTIAL"):
        if marker in log_text:
            return "partial"
    for marker in ("**Status:** BLOCKED", "Status: BLOCKED"):
        if marker in log_text:
            return "blocked"

    # NEW: Detect specific error patterns
    if "Claude Code cannot be launched inside another Claude Code session" in log_text:
        return "failed"  # Environment issue, should trigger retry

    if "Error:" in log_text or "FATAL:" in log_text:
        return "failed"  # Generic errors

    # Check for completely empty logs
    if not log_text or len(log_text.strip()) < 50:
        return "failed"  # Immediate crash

    return "unknown"
```

---

### 🟢 ISSUE #4: Retry Logic Improvements
**Impact**: Wasted token/time on non-transient failures
**Severity**: LOW - Optimization opportunity

**Current Behavior**:
- max_retries=2, so segments attempt up to 3 times (initial + 2 retries)
- S13-17 attempted 5 times each (initial + 4 operator retries)
- All failed with same CLAUDECODE error → predictable failure

**Improvement**:
```python
# In scripts/orchestrate_v2/__main__.py, around line 322-327:
while attempts <= config.max_retries:
    attempts = await state.increment_attempts(seg.num)
    status, summary = await run_segment(...)

    # NEW: Don't retry on environment errors (will fail same way)
    if status in ("pass", "timeout"):
        break
    if "cannot be launched inside another Claude Code" in summary:
        log.warning("S%02d failed due to environment issue, stopping retries", seg.num)
        break

    if attempts > config.max_retries:
        break
```

---

## Performance Optimizations

### ✅ Already Optimized:
1. **Worktree Isolation**: Using git worktree pool (4 worktrees max)
2. **Parallelism**: max_parallel=6 with proper semaphore
3. **Async I/O**: Using aiofiles for monitor (commit 02525f7)
4. **Timeout**: 3600s per segment (reasonable for TUI work)
5. **Heartbeat**: 300s interval with stall detection at 1800s

### 🔧 Optimization Opportunities:

#### OPT-1: Segment Pre-flight Check
Add lightweight build check before segment execution to fail-fast on workspace errors:
```python
# In scripts/orchestrate_v2/runner.py, before line 280:
async def _preflight_check(config: OrchestrateConfig, cwd: Path) -> tuple[bool, str]:
    """Quick build check to detect workspace errors before starting segment."""
    if not config.gate_command:
        return True, ""

    proc = await asyncio.create_subprocess_shell(
        "cargo check --workspace --quiet",
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
        cwd=cwd,
    )
    stdout, stderr = await asyncio.wait_for(proc.communicate(), timeout=120)

    if proc.returncode != 0:
        error = stderr.decode()[:500]
        return False, f"Workspace has pre-existing errors: {error}"

    return True, ""
```

#### OPT-2: Token Usage Tracking
Already implemented (line 212-232) but not exposed in dashboard - add to monitor.

#### OPT-3: Parallel Gate Checks
Gate runs sequentially after each wave. For independent crates, could run checks in parallel:
```python
# cargo check -p prb-tui & cargo check -p prb-ai & wait
```

---

## Immediate Action Plan

### Priority 1: Fix CLAUDECODE Issue (CRITICAL)
```bash
# Edit runner.py to remove CLAUDECODE from environment
# This will fix 7 "unknown" segments in Wave 4-5
```

### Priority 2: Resume Orchestration
```bash
# Kill current stalled orchestrator
kill 48092

# Restart with fix
python -m scripts.orchestrate_v2 run .claude/plans/phase3-tui-evolution
```

### Priority 3: Handle S10 Compilation Issues
- Option A: Fix S10 manually and mark S12/S18 as PASS
- Option B: Retry S10 with explicit completion requirements
- Option C: Revert S10 changes and retry from clean state

---

## Expected Results After Fix

| Wave | Before | After (Estimated) |
|------|--------|-------------------|
| 1    | 5 PASS | 5 PASS (no change) |
| 2    | 8 PASS | 8 PASS (no change) |
| 3    | 3 RUNNING/PARTIAL/BLOCKED | 5 PASS (S12/S18 working, S10 needs fix) |
| 4    | 6 UNKNOWN | 6 RUNNING → PASS/PARTIAL (can now execute) |
| 5    | 1 UNKNOWN | 1 RUNNING → PASS/PARTIAL (can now execute) |

**Token Savings**: ~200K tokens (7 segments × ~30K avg no longer failing immediately)
**Time Savings**: ~45-60 minutes (eliminating failed retry cycles)

---

## Monitoring Recommendations

1. **Add to state.db schema**: Track distinct failure patterns
2. **Dashboard Enhancement**: Show retry reason (timeout vs error vs environment)
3. **Alert Tuning**: Don't alert on transient network issues in first 60s
4. **Gate Optimization**: Cache gate results if no files changed

---

## Long-term Improvements

1. **Nested Session Detection**: Add explicit check in orchestrator startup
2. **Build Health Check**: Add `preflight_check()` before segment dispatch
3. **Smart Retry**: Don't retry environment errors, do retry transient failures
4. **Status Taxonomy**: Extend beyond pass/partial/blocked/unknown to include "failed-environment", "failed-compilation", etc.
5. **Parallel Gates**: Split gate command by crate for faster feedback

---

## Commands to Execute Now

```bash
# 1. Stop current orchestrator
kill 48092

# 2. Apply CLAUDECODE fix to runner.py
# (See fix in ISSUE #1 above)

# 3. Restart orchestrator
python -m scripts.orchestrate_v2 run .claude/plans/phase3-tui-evolution --monitor 8089

# 4. Monitor progress
# Dashboard: http://localhost:8089
# Status: python -m scripts.orchestrate_v2 status .claude/plans/phase3-tui-evolution
```
