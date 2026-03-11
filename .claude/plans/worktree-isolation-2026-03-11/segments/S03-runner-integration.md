# Segment 3: Runner Worktree Integration

**Goal**: Integrate WorktreePool into the runner so segments execute in isolated worktrees when isolation_strategy="worktree".

**Complexity**: Medium | **Risk**: 5/10 | **Cycle Budget**: 15

## Issues Addressed
- Issue 2 (part 2): Runner-level worktree integration

## Key Files
- `scripts/orchestrate_v2/runner.py:235-350` - run_segment() function
- `scripts/orchestrate_v2/runner.py:64-69` - _build_env() function

## Implementation

### Step 1: Import WorktreePool
At top of `runner.py`:
```python
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .config import OrchestrateConfig
    from .planner import Segment
    from .state import StateDB
    from .worktree_pool import WorktreePool  # NEW
```

### Step 2: Add worktree_pool parameter to run_segment()
At line 235, modify signature:
```python
async def run_segment(
    seg: "Segment",
    config: "OrchestrateConfig",
    state: "StateDB",
    log_dir: Path,
    notifier=None,
    attempt_num: int = 1,
    register_pid=None,
    unregister_pid=None,
    worktree_pool: "WorktreePool | None" = None,  # NEW
) -> tuple[str, str]:
```

### Step 3: Acquire worktree and set working directory
After line 273, before subprocess creation:
```python
    heartbeat: asyncio.Task | None = None
    worktree = None  # NEW
    cwd = None  # NEW

    try:
        # NEW: Acquire worktree if pool provided
        if worktree_pool and config.isolation_strategy == "worktree":
            worktree = await worktree_pool.acquire(seg.num)
            cwd = worktree.path
            log.info("S%02d acquired worktree: %s", seg.num, cwd)

        proc = await asyncio.create_subprocess_exec(
            "claude",
            "-p", prompt,
            "--dangerously-skip-permissions",
            "--verbose",
            "--output-format", "stream-json",
            stdin=asyncio.subprocess.DEVNULL,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.STDOUT,
            env=env,
            cwd=cwd,  # MODIFIED: pass worktree path
            start_new_session=True,
            limit=2**22,
        )
```

### Step 4: Release worktree in finally block
In the existing finally block (around line 329-337), add worktree release:
```python
    finally:
        try:
            if unregister_pid:
                unregister_pid(seg.num)
        except Exception:
            log.exception("Failed to unregister PID for S%02d", seg.num)
        finally:
            if heartbeat is not None:
                heartbeat.cancel()
                try:
                    await heartbeat
                except asyncio.CancelledError:
                    pass
            # NEW: Release worktree back to pool
            if worktree_pool and worktree:
                try:
                    await worktree_pool.release(worktree)
                    log.info("S%02d released worktree: %s", seg.num, worktree.path)
                except Exception:
                    log.exception("Failed to release worktree for S%02d", seg.num)
```

**Wait**: WorktreePool.acquire() is a context manager! We should use it properly. Let me revise Step 3 and Step 4:

### Step 3 (Revised): Use async context manager
Replace the try block structure:
```python
    heartbeat: asyncio.Task | None = None

    # NEW: Context manager for worktree lifecycle
    if worktree_pool and config.isolation_strategy == "worktree":
        async with worktree_pool.acquire(seg.num) as wt:
            cwd = wt.path
            log.info("S%02d acquired worktree: %s", seg.num, cwd)
            return await _run_in_directory(
                seg, config, state, log_dir, notifier, attempt_num,
                register_pid, unregister_pid, cwd, prompt, env, segment_timeout,
                stall_threshold, started_at
            )
    else:
        # Original path: no worktree
        return await _run_in_directory(
            seg, config, state, log_dir, notifier, attempt_num,
            register_pid, unregister_pid, None, prompt, env, segment_timeout,
            stall_threshold, started_at
        )
```

**Actually**, this would require extracting a large helper function. Let's keep it simpler and handle the context manager in the caller (__main__.py). For the runner, we'll just pass worktree path directly.

### Step 3 (Simplified): Accept optional cwd parameter
Modify signature again:
```python
async def run_segment(
    seg: "Segment",
    config: "OrchestrateConfig",
    state: "StateDB",
    log_dir: Path,
    notifier=None,
    attempt_num: int = 1,
    register_pid=None,
    unregister_pid=None,
    cwd: Path | None = None,  # NEW: working directory for subprocess
) -> tuple[str, str]:
```

And use it in subprocess creation:
```python
        proc = await asyncio.create_subprocess_exec(
            "claude",
            "-p", prompt,
            "--dangerously-skip-permissions",
            "--verbose",
            "--output-format", "stream-json",
            stdin=asyncio.subprocess.DEVNULL,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.STDOUT,
            env=env,
            cwd=cwd,  # NEW: use provided cwd
            start_new_session=True,
            limit=2**22,
        )
```

This keeps the runner simple - it just accepts a working directory. The orchestrator will handle worktree acquire/release.

## Exit Criteria
1. ✓ Syntax check: `python -m py_compile scripts/orchestrate_v2/runner.py`
2. ✓ Backward compatibility test: Run with isolation_strategy="none" - segments run in main repo
3. ✓ Backward compatibility test: Run with isolation_strategy="env" - segments use CARGO_TARGET_DIR
4. ✓ Manual test: Set isolation_strategy="worktree", verify subprocess starts in worktree path
5. ✓ Self-review: Changes minimal, no logic duplication, cwd parameter well-typed
