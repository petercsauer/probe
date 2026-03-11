# Segment 2: Worktree Pool Infrastructure

**Goal**: Create standalone worktree pool module that manages a fixed pool of git worktrees for segment isolation.

**Complexity**: Medium | **Risk**: 5/10 | **Cycle Budget**: 15

## Issues Addressed
- Issue 2 (part 1): Worktree isolation foundation

## Key Files
- `scripts/orchestrate_v2/worktree_pool.py` (NEW)
- `scripts/orchestrate_v2/config.py` (doc update)

## Implementation

### WorktreePool Class
```python
from dataclasses import dataclass
from pathlib import Path
import asyncio
from contextlib import asynccontextmanager

@dataclass
class Worktree:
    pool_id: int
    path: Path
    branch: str
    current_segment: int | None = None

class WorktreePool:
    def __init__(self, repo_root: Path, pool_size: int, target_branch: str = "main"):
        self._repo_root = repo_root
        self._pool_size = pool_size
        self._target_branch = target_branch
        self._worktrees: list[Worktree] = []
        self._queue: asyncio.Queue[Worktree] = asyncio.Queue()

    async def create(self) -> None:
        """Create pool of worktrees, reuse existing."""
        # 1. Run: git worktree prune
        # 2. For i in range(pool_size):
        #    Check if .claude/worktrees/pool-{i:02d} exists
        #    If not: git worktree add .claude/worktrees/pool-{i:02d} -b wt/pool-{i:02d} HEAD
        #    Add Worktree to queue

    @asynccontextmanager
    async def acquire(self, seg_num: int = 0) -> Worktree:
        """Acquire worktree from pool (blocks if all in use)."""
        wt = await self._queue.get()
        wt.current_segment = seg_num
        try:
            # Reset: git reset --hard {target_branch} && git clean -fdx
            yield wt
        finally:
            wt.current_segment = None
            self._queue.put_nowait(wt)

    async def cleanup(self) -> None:
        """Remove all worktrees."""
        # For each: git worktree remove --force {path}
```

### Config Update
Update `config.py` docstring for `isolation_strategy`:
- "none": No isolation (default)
- "env": Per-segment environment variables
- "worktree": Git worktree pool isolation (NEW)

## Exit Criteria
1. ✓ Syntax check: `python -m py_compile scripts/orchestrate_v2/worktree_pool.py`
2. ✓ Standalone test: Create test script that creates pool, acquires, releases, cleans up
3. ✓ Manual verification: `git worktree list` shows pool worktrees after create()
4. ✓ Self-review: WorktreePool is self-contained, no coupling to runner yet
