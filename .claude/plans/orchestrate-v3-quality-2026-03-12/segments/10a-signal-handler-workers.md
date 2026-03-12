---
segment: 10
title: "Extract SignalHandler and background workers"
depends_on: [2, 3, 4, 5, 6, 7, 8, 9]
cycle_budget: 15
risk: 5
complexity: "Medium"
commit_message: "refactor(orchestrate): Extract SignalHandler and background workers"
---

# Segment 10: Extract SignalHandler and background workers

## Goal

Extract SignalHandler class and background worker functions (_heartbeat_loop, _notification_worker) from __main__.py to reduce coupling before major Orchestrator extraction.

## Context

__main__.py (1,399 lines) contains two independent concerns that can be extracted safely:
1. Signal handling (shutdown events)
2. Background workers (heartbeat and notification processing)

Extracting these first simplifies S10b (Orchestrator coordinator extraction).

## Scope

- **Create:** `signal_handler.py` (~50 lines: SignalHandler class)
- **Create:** `background_workers.py` (~100 lines: _heartbeat_loop, _notification_worker)
- **Modify:** `__main__.py` (remove extracted code, add imports)
- **Create:** `test_signal_handler.py` (~50 lines)
- **Create:** `test_background_workers.py` (~80 lines)

## Implementation Approach

1. **Create `signal_handler.py`:**
   ```python
   class SignalHandler:
       """Manages graceful shutdown via SIGINT/SIGTERM."""

       def __init__(self):
           self.shutting_down = asyncio.Event()
           self.worker_stop = asyncio.Event()

       def register_handlers(self, loop: asyncio.AbstractEventLoop) -> None:
           """Register signal handlers for current event loop."""
           for sig in (signal.SIGINT, signal.SIGTERM):
               loop.add_signal_handler(sig, self.shutdown)

       def shutdown(self) -> None:
           """Trigger graceful shutdown."""
           self.shutting_down.set()
           self.worker_stop.set()

       def is_shutting_down(self) -> bool:
           return self.shutting_down.is_set()
   ```

2. **Create `background_workers.py`:**
   ```python
   async def heartbeat_loop(
       state: StateDB,
       notifier: Notifier,
       interval: int,
       stop_event: asyncio.Event,
       config: OrchestrateConfig
   ) -> None:
       """Monitor running segments and update progress."""
       # Move _heartbeat_loop from __main__.py (lines ~150-220)

   async def notification_worker(
       notifier: Notifier,
       state: StateDB,
       stop_event: asyncio.Event
   ) -> None:
       """Process queued notifications."""
       # Move _notification_worker from __main__.py (lines ~222-250)
   ```

3. **Update `__main__.py`:**
   - Remove extracted functions
   - Import: `from .signal_handler import SignalHandler`
   - Import: `from .background_workers import heartbeat_loop, notification_worker`
   - In `_orchestrate_inner()`:
     ```python
     signal_handler = SignalHandler()
     signal_handler.register_handlers(loop)

     heartbeat_task = asyncio.create_task(
         heartbeat_loop(state, notifier, config.heartbeat_interval,
                       signal_handler.worker_stop, config)
     )
     notif_task = asyncio.create_task(
         notification_worker(notifier, state, signal_handler.worker_stop)
     )
     ```

4. **Write tests:**
   - `test_signal_handler.py`: Test shutdown event triggering
   - `test_background_workers.py`: Test heartbeat monitoring, notification processing

## Alternatives Ruled Out

- **Extract directly into Orchestrator class:** Rejected (S10b already complex)
- **Keep in __main__.py:** Rejected (reduces testability)

## Pre-Mortem Risks

- **Event loop registration timing:** Signal handlers must be registered after loop is running
  - Mitigation: `register_handlers()` called in `_orchestrate_inner()` after `asyncio.get_running_loop()`
- **Background task cleanup:** Tasks might not cancel properly
  - Mitigation: Use `stop_event` properly, test cancellation

## Exit Criteria

1. **Targeted tests:** test_signal_handler.py and test_background_workers.py pass (10+ tests)
2. **Regression tests:** **ALL existing tests still pass**
3. **Full build gate:** No syntax errors
4. **Full test gate:** Full pytest suite passes
5. **Self-review gate:** All signal handling tested, background workers tested
6. **Scope verification gate:** Only signal_handler.py, background_workers.py, test files, __main__.py modified

## Commands

```bash
# Build
python -m py_compile scripts/orchestrate_v3/signal_handler.py \
  scripts/orchestrate_v3/background_workers.py

# Test (targeted)
pytest scripts/orchestrate_v3/test_signal_handler.py \
  scripts/orchestrate_v3/test_background_workers.py -v

# Test (regression) - CRITICAL
pytest scripts/orchestrate_v3/ -v

# Test (full gate)
pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3 --cov-report=term
```
