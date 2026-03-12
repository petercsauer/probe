---
segment: 11
title: "Extract Orchestrator coordinator class"
depends_on: [10]
cycle_budget: 25
risk: 8
complexity: "High"
commit_message: "refactor(orchestrate): Extract Orchestrator coordinator class with DI"
---

# Segment 11: Extract Orchestrator coordinator class

## Goal

Extract high-level orchestration logic from __main__.py into Orchestrator class with dependency injection.

## Context

__main__.py (1,399 lines) is a god module. After S10 extracted SignalHandler and background workers, need to extract orchestration coordinator with recovery agent, gate checks, and worktree pool integration. Protected by 70%+ test coverage from S2-S9.

## Scope

- **Create:** `orchestrator.py` (~450 lines: Orchestrator class with recovery/gates)
- **Modify:** `__main__.py` (remove _orchestrate_inner, keep _run_wave/_run_gate for now)
- **Create:** `test_orchestrator.py` (~200 lines)

## Implementation Approach

1. **Create `orchestrator.py` with Orchestrator class:**
   ```python
   class Orchestrator:
       def __init__(self, state: StateDB, config: OrchestrateConfig,
                    notifier: Notifier, monitor: MonitorServer,
                    signal_handler: SignalHandler,
                    recovery_agent: RecoveryAgent | None = None,
                    pool: WorktreePool | None = None):
           self.state = state
           self.config = config
           self.notifier = notifier
           self.monitor = monitor
           self.signal_handler = signal_handler
           self.recovery_agent = recovery_agent
           self.pool = pool

       async def run(self, segments: list[Segment],
                     waves: dict[int, list[Segment]],
                     max_wave: int, meta: PlanMeta,
                     log_dir: Path) -> None:
           """Main orchestration loop with recovery and gate checks."""
           await self.monitor.start()
           await self.notifier.started(meta.title, len(segments), max_wave)

           try:
               for wave_num in range(1, max_wave + 1):
                   if self.signal_handler.is_shutting_down():
                       break

                   wave_segs = waves.get(wave_num, [])
                   if not wave_segs:
                       continue

                   # Resume support: skip already-passed segments
                   pending = await self._filter_pending_segments(wave_segs)
                   if not pending:
                       continue

                   # Pre-flight health check
                   if self.config.enable_preflight_checks:
                       healthy, errors = await self._pre_wave_health_check(wave_num)
                       if not healthy:
                           await self._handle_preflight_failure(wave_num, errors)
                           break

                   # Execute wave (delegates to _run_wave in __main__.py for now)
                   results = await _run_wave(
                       wave_num, pending, self.config, self.state,
                       self.notifier, log_dir, self.signal_handler.shutting_down,
                       self.pool, segments
                   )

                   await self.notifier.wave_complete(wave_num, max_wave, results)

                   # Recovery: Auto-retry cascade victims
                   if self.config.recovery_enabled and self.recovery_agent:
                       await self._attempt_recovery(wave_num, pending, results, log_dir)

                   # Gate check
                   if self.config.gate_command:
                       gate_ok = await self._run_gate_check(wave_num, log_dir)
                       if not gate_ok and self.config.gate_required:
                           break

           finally:
               await self.cleanup()

       async def cleanup(self):
           """Clean up resources."""
           if self.pool:
               await self.pool.cleanup()
           await self.monitor.stop()
           await self.state.close()
   ```

2. **Refactor `__main__.py`:**
   - Replace `_orchestrate_inner()` body with Orchestrator instantiation:
     ```python
     async def _orchestrate_inner(...):
         state = await StateDB.create(db_path)
         # ... initialization ...

         signal_handler = SignalHandler()
         signal_handler.register_handlers(loop)

         recovery_agent = RecoveryAgent(state, config) if config.recovery_enabled else None

         orchestrator = Orchestrator(
             state, config, notifier, monitor,
             signal_handler, recovery_agent, pool
         )

         await orchestrator.run(segments, waves, max_wave, meta, log_dir)
     ```
   - Keep `_run_wave()`, `_run_one()`, `_run_gate()` as module-level (move in S11)

3. **Create `test_orchestrator.py`:**
   - Test Orchestrator.run with mocked wave execution
   - Test pre-flight health checks
   - Test recovery agent integration
   - Test gate check flow
   - Test cleanup on error

## Pre-Mortem Risks

- **Breaking wave execution:** _run_wave has many implicit dependencies
  - Mitigation: Pass all params explicitly, verify with tests
- **Recovery agent integration:** Complex retry logic
  - Mitigation: Mock RecoveryAgent in tests, preserve existing behavior
- **Gate check integration:** Gate failures might break flow
  - Mitigation: Test gate pass/fail scenarios
- **Worktree pool cleanup:** Pool might not clean up on error
  - Mitigation: Ensure cleanup() in finally block always runs
- **Test suite catches regressions:** 70% coverage protection
  - Mitigation: Run full suite after refactor

## Exit Criteria

1. **Targeted tests:** test_orchestrator.py passes (15+ tests covering recovery/gates)
2. **Regression tests:** **ALL existing tests still pass**
3. **Full build gate:** No syntax errors
4. **Full test gate:** Full pytest suite passes, coverage ≥70%
5. **Self-review gate:** All resource cleanup in finally blocks, recovery agent behavior preserved
6. **Scope verification gate:** Only orchestrator.py, test_orchestrator.py, __main__.py modified

## Commands

```bash
# Build
python -m py_compile scripts/orchestrate_v3/orchestrator.py

# Test (targeted)
pytest scripts/orchestrate_v3/test_orchestrator.py -v

# Test (regression) - CRITICAL
pytest scripts/orchestrate_v3/ -v

# Test (full gate)
pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3 --cov-report=term
```
