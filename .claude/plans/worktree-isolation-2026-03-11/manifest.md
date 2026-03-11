# Worktree Isolation Plan - 2026-03-11

## Overview

**Approach**: Implement worktree isolation for the Python orchestration system through incremental delivery: first fixing critical bugs to improve baseline reliability, then building worktree infrastructure layer-by-layer (foundation → runner → orchestrator), and optimizing monitor performance in parallel.

**Delivery Order**: Dependency-order (topological) with Wave 1 parallelization to maximize throughput while respecting integration dependencies.

**Total Scope**: 5 segments across 3 waves, ~65 minutes with parallel execution

---

## Dependency Diagram

```
Wave 1 (parallel):
  ├─ S1: Core Bug Fixes Bundle (Risk 3/10)
  ├─ S2: Worktree Pool Infrastructure (Risk 5/10)
  └─ S5: Async File I/O for Monitor (Risk 4/10)

Wave 2:
  └─ S3: Runner Worktree Integration (depends on S2, Risk 5/10)

Wave 3:
  └─ S4: Orchestrator Integration + Merge Automation (depends on S3, Risk 7/10)
```

---

## Segments

### Segment 1: Core Bug Fixes Bundle (Wave 1)
- **Issues**: 1, 4, 5
- **Complexity**: Low
- **Files**: `__main__.py`, `runner.py`
- **Exit Criteria**: Error handling preserves segment identity, process kills logged, heartbeat cleanup guaranteed

### Segment 2: Worktree Pool Infrastructure (Wave 1)
- **Issues**: 2 (part 1)
- **Complexity**: Medium
- **Files**: `worktree_pool.py` (new), `config.py`
- **Exit Criteria**: Pool creates/acquires/releases worktrees, standalone test passes

### Segment 3: Runner Worktree Integration (Wave 2)
- **Issues**: 2 (part 2)
- **Complexity**: Medium
- **Files**: `runner.py`
- **Exit Criteria**: Segments run in isolated worktrees, backward compatibility maintained

### Segment 4: Orchestrator Integration + Merge Automation (Wave 3)
- **Issues**: 2 (part 3)
- **Complexity**: High
- **Files**: `__main__.py`, `worktree_pool.py`
- **Exit Criteria**: Pool lifecycle integrated, auto-merge works, conflicts handled gracefully

### Segment 5: Async File I/O for Monitor (Wave 1)
- **Issues**: 3
- **Complexity**: Low
- **Files**: `monitor.py`, `requirements.txt`
- **Exit Criteria**: SSE handlers non-blocking, concurrent clients supported

---

See segment brief files in this directory for full implementation details.
