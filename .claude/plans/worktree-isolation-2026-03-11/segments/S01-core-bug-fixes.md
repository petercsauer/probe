# Segment 1: Core Bug Fixes Bundle

**Goal**: Fix three critical error handling bugs: lost segment identity in gather exceptions, silent process kill failures, and heartbeat task leak.

**Complexity**: Low | **Risk**: 3/10 | **Cycle Budget**: 10

## Issues Addressed
- Issue 1: Lost segment identity in asyncio.gather() exception handling
- Issue 4: Process kill failure silent swallowing
- Issue 5: Heartbeat task cleanup leak

## Key Files
- `scripts/orchestrate_v2/__main__.py:278-284` - gather exception handling
- `scripts/orchestrate_v2/runner.py:72-83` - _kill_tree() function
- `scripts/orchestrate_v2/runner.py:329-337` - heartbeat cleanup

## Implementation

### Fix 1: Preserve Segment Identity
At `__main__.py:277`, store segment numbers with tasks:
```python
task_map = [(seg.num, asyncio.create_task(_run_one(seg), name=f"S{seg.num:02d}")) for seg in segments]
tasks = [t for _, t in task_map]
done = await asyncio.gather(*tasks, return_exceptions=True)
for (seg_num, task), result in zip(task_map, done):
    if isinstance(result, Exception):
        results.append((seg_num, "error"))  # NOT (0, "error")
```

### Fix 2: Log Process Kill Failures
In `runner.py:_kill_tree()`:
```python
def _kill_tree(pid: int) -> bool:
    try:
        os.killpg(os.getpgid(pid), signal.SIGTERM)
        return True
    except ProcessLookupError:
        log.info("Process %d already terminated", pid)
        return True
    except PermissionError:
        log.warning("Cannot kill process %d: permission denied", pid)
        return False
    except Exception as e:
        log.error("Failed to kill process %d: %s", pid, e)
        return False
```

### Fix 3: Guarantee Heartbeat Cleanup
In `runner.py:run_segment()` finally block:
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
```

## Exit Criteria
1. ✓ Syntax check passes: `python -m py_compile scripts/orchestrate_v2/__main__.py scripts/orchestrate_v2/runner.py`
2. ✓ Regression test: `python -m scripts.orchestrate_v2 dry-run .claude/plans/phase3-tui-evolution`
3. ✓ Manual test: Trigger timeout, verify correct segment number logged
4. ✓ Self-review: No dead code, changes isolated to error handling
