# Segment 5: Async File I/O for Monitor

**Goal**: Eliminate event loop blocking in monitor SSE handlers by replacing synchronous file I/O with aiofiles.

**Complexity**: Low | **Risk**: 4/10 | **Cycle Budget**: 10

## Issues Addressed
- Issue 3: Async file I/O blocking event loop

## Key Files
- `scripts/orchestrate_v2/monitor.py:158-177` - _handle_log_sse()
- `scripts/orchestrate_v2/requirements.txt`

## Implementation

### Add Dependency
In `requirements.txt`:
```
aiofiles>=24.1.0
```

### Replace Synchronous Operations
In `monitor.py:_handle_log_sse()`:
```python
import aiofiles
import aiofiles.os

# Line 158: Replace target.exists()
if await aiofiles.os.path.exists(str(target)):

# Line 159: Replace target.read_bytes()
async with aiofiles.open(target, 'rb') as f:
    raw = await f.read()
```

## Exit Criteria
1. ✓ Syntax check: `python -m py_compile scripts/orchestrate_v2/monitor.py`
2. ✓ Regression test: Dashboard loads, SSE endpoints work
3. ✓ Manual test: Connect 5 concurrent SSE clients, verify no blocking
4. ✓ Self-review: Only file I/O operations changed, SSE logic unchanged
