---
segment: 12
title: "Extract WaveRunner and SegmentExecutor classes"
depends_on: [11]
cycle_budget: 20
risk: 8
complexity: "High"
commit_message: "refactor(orchestrate): Extract WaveRunner and SegmentExecutor classes"
---

# Segment 12: Extract WaveRunner and SegmentExecutor classes

## Goal

Complete god module decomposition by extracting WaveRunner and SegmentExecutor classes, removing procedural _run_wave and _run_one functions. Final __main__.py size: ~450 lines (down from 1,399).

## Context

After S11, __main__.py still contains _run_wave (~380 lines) and _run_one (~140 lines). Need to extract into testable classes.

## Scope

- **Create:** `wave_runner.py` (~250 lines: WaveRunner class)
- **Create:** `segment_executor.py` (~150 lines: SegmentExecutor class)
- **Modify:** `orchestrator.py` (use WaveRunner)
- **Modify:** `__main__.py` (remove _run_wave, _run_one, _merge_worktree_changes)
- **Create:** `test_wave_runner.py`, `test_segment_executor.py`

## Implementation Approach

1. **Create `segment_executor.py`:**
   ```python
   class SegmentExecutor:
       def __init__(self, config: OrchestrateConfig, state: StateDB,
                    notifier: Notifier, log_dir: Path):
           self.config = config
           self.state = state
           self.notifier = notifier
           self.log_dir = log_dir
           self.circuit_breaker = CircuitBreaker()

       async def execute(self, segment: Segment,
                        worktree_path: Path | None = None) -> tuple[str, str]:
           """Execute single segment with retry logic."""
           attempts = 0
           while attempts <= self.config.max_retries:
               attempts = await self.state.increment_attempts(segment.num)

               status, summary = await run_segment(...)

               if status in ("pass", "timeout"):
                   break

               if not self.config.retry_policy.should_retry(status):
                   break

               should_retry, reason = self.circuit_breaker.should_retry(summary)
               if not should_retry:
                   break

               delay = self.config.retry_policy.get_delay(attempts - 1)
               await asyncio.sleep(delay)

           return status, summary
   ```

2. **Create `wave_runner.py`:**
   ```python
   class WaveRunner:
       def __init__(self, state: StateDB, config: OrchestrateConfig,
                    notifier: Notifier, log_dir: Path,
                    pool: WorktreePool | None = None):
           self.state = state
           self.config = config
           self.notifier = notifier
           self.log_dir = log_dir
           self.pool = pool
           self.segment_executor = SegmentExecutor(config, state,
                                                   notifier, log_dir)

       async def execute(self, wave_num: int, segments: list[Segment],
                        shutting_down: asyncio.Event) -> list[tuple[int, str]]:
           """Execute all segments in wave with bounded parallelism."""
           sem = asyncio.Semaphore(self.config.max_parallel)

           async def _run_one_segment(seg: Segment) -> tuple[int, str]:
               # Dependency validation, skip checks, execution
               pass

           tasks = [_run_one_segment(seg) for seg in segments]
           return await asyncio.gather(*tasks)
   ```

3. **Update `orchestrator.py`:**
   - Constructor creates WaveRunner
   - `run()` delegates to `wave_runner.execute()`

4. **Update `__main__.py`:**
   - Remove `_run_wave()` (228 lines)
   - Remove `_run_one()` (142 lines)
   - Remove `_merge_worktree_changes()` (93 lines)
   - Keep: CLI parsing, _run_gate, _claude_summarise, workers
   - Final size: ~400 lines

5. **Write tests:**
   - `test_segment_executor.py` - Retry logic, circuit breaker
   - `test_wave_runner.py` - Parallel execution, dependencies

## Pre-Mortem Risks

- **Retry loop logic changed:** Subtle bugs
  - Mitigation: Comprehensive tests, compare to original
- **Worktree merge breaks:** Git operations tricky
  - Mitigation: Copy logic exactly, reuse tests
- **Parallel execution semantics:** asyncio.gather differences
  - Mitigation: Integration tests

## Exit Criteria

1. **Targeted tests:** test_wave_runner.py and test_segment_executor.py pass (30+ tests)
2. **Regression tests:** **ALL existing tests still pass**
3. **Full build gate:** No syntax errors
4. **Full test gate:** Full pytest suite passes, coverage ≥70%
5. **Self-review gate:** __main__.py reduced to ~400 lines
6. **Scope verification gate:** 2 new modules, orchestrator.py and __main__.py refactored

## Commands

```bash
# Build
python -m py_compile scripts/orchestrate_v3/wave_runner.py \
  scripts/orchestrate_v3/segment_executor.py

# Test (targeted)
pytest scripts/orchestrate_v3/test_wave_runner.py \
  scripts/orchestrate_v3/test_segment_executor.py -v

# Test (regression) - CRITICAL
pytest scripts/orchestrate_v3/ -v

# Test (full gate)
pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3 --cov-report=term

# Verify coverage threshold
pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3 --cov-fail-under=70
```
