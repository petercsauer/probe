# Segment 4: Orchestrator Integration + Merge Automation

**Goal**: Integrate WorktreePool lifecycle into orchestrator main loop, with automatic merge-back of successful segment changes.

**Complexity**: High | **Risk**: 7/10 | **Cycle Budget**: 15

## Issues Addressed
- Issue 2 (part 3): Orchestrator-level pool management and auto-merge

## Key Files
- `scripts/orchestrate_v2/__main__.py:150-280` - main orchestration loop
- `scripts/orchestrate_v2/worktree_pool.py:60-70` - cleanup() method (may need merge logic)

## Implementation

### Step 1: Import WorktreePool
At top of `__main__.py`:
```python
from .worktree_pool import WorktreePool
```

### Step 2: Create pool before wave execution
In the main orchestration function (around line 200-220), after loading config and before wave loop:
```python
async def _run(args, config: OrchestrateConfig) -> int:
    # ... existing setup code ...

    # NEW: Create worktree pool if isolation strategy requires it
    pool: WorktreePool | None = None
    if config.isolation_strategy == "worktree":
        pool_size = min(config.max_parallel, 4)  # Never exceed 4 worktrees
        pool = WorktreePool(
            repo_root=Path.cwd(),
            pool_size=pool_size,
            target_branch="main",  # TODO: detect current branch
        )
        await pool.create()
        log.info("Created worktree pool with %d worktrees", pool_size)

    try:
        # ... wave execution loop ...
    finally:
        # NEW: Cleanup pool
        if pool:
            await pool.cleanup()
            log.info("Cleaned up worktree pool")
```

### Step 3: Pass worktree context to segment execution
In the wave execution loop where `_run_one()` is defined (around line 277):
```python
async def _run_one(seg: Segment) -> tuple[int, str]:
    """Run one segment, wrapping with worktree acquire/release if needed."""
    cwd = None

    # NEW: Acquire worktree if pool exists
    if pool and config.isolation_strategy == "worktree":
        async with pool.acquire(seg.num) as wt:
            cwd = wt.path
            status, summary = await run_segment(
                seg, config, state, log_dir, notifier,
                register_pid=lambda n, p: _running_pids.__setitem__(n, p),
                unregister_pid=lambda n: _running_pids.pop(n, None),
                cwd=cwd,  # NEW: pass worktree path
            )

            # NEW: Auto-merge on success
            if status == "pass" and config.isolation_strategy == "worktree":
                merge_ok = await _merge_worktree_changes(wt, seg)
                if not merge_ok:
                    log.warning("S%02d passed but merge failed - manual intervention needed", seg.num)
                    return seg.num, "pass-merge-conflict"

            return seg.num, status
    else:
        # Original path: no worktree
        status, summary = await run_segment(
            seg, config, state, log_dir, notifier,
            register_pid=lambda n, p: _running_pids.__setitem__(n, p),
            unregister_pid=lambda n: _running_pids.pop(n, None),
        )
        return seg.num, status
```

### Step 4: Implement merge helper
Add new helper function before the main orchestration function:
```python
async def _merge_worktree_changes(wt: "Worktree", seg: "Segment") -> bool:
    """Merge successful segment changes from worktree branch back to main.

    Returns True on clean merge, False on conflict.
    """
    try:
        # Ensure we're on main branch in main worktree
        proc = await asyncio.create_subprocess_exec(
            "git", "checkout", "main",
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        await proc.wait()
        if proc.returncode != 0:
            log.error("Failed to checkout main branch")
            return False

        # Merge the worktree branch
        proc = await asyncio.create_subprocess_exec(
            "git", "merge", "--no-ff", "-m",
            f"Merge segment S{seg.num:02d}: {seg.title}",
            wt.branch,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        stdout, stderr = await proc.communicate()

        if proc.returncode != 0:
            # Merge conflict - abort merge and log
            await asyncio.create_subprocess_exec(
                "git", "merge", "--abort",
                stdout=asyncio.subprocess.DEVNULL,
                stderr=asyncio.subprocess.DEVNULL,
            )
            log.error(
                "Merge conflict for S%02d:\nstdout: %s\nstderr: %s",
                seg.num, stdout.decode(), stderr.decode()
            )
            return False

        log.info("Successfully merged S%02d changes from %s", seg.num, wt.branch)
        return True

    except Exception as e:
        log.exception("Exception during merge for S%02d: %s", seg.num, e)
        return False
```

### Step 5: Handle pass-merge-conflict status
In the results processing loop (after asyncio.gather), handle new status:
```python
for (seg_num, task), result in zip(task_map, done):
    if isinstance(result, Exception):
        results.append((seg_num, "error"))
        log.error("S%02d raised exception: %s", seg_num, result)
    else:
        num, status = result
        results.append((num, status))

        # NEW: Treat merge conflicts as partial success
        if status == "pass-merge-conflict":
            log.warning("S%02d completed but has merge conflicts - review worktree branch %s",
                       num, f"wt/pool-{num:02d}")
```

## Exit Criteria
1. ✓ Syntax check: `python -m py_compile scripts/orchestrate_v2/__main__.py`
2. ✓ Pool lifecycle test: Verify pool created at start, cleaned at end
3. ✓ Merge test: Successful segment merges cleanly to main
4. ✓ Conflict handling test: Segment with conflict reports "pass-merge-conflict" and leaves branch for manual review
5. ✓ Backward compatibility: isolation_strategy="none" and "env" work unchanged
6. ✓ Self-review: Worktree acquire/release properly scoped in async context manager
