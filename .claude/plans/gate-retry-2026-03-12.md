# Gate Retry with Downstream Reset - Deep Plan

**Plan Date:** 2026-03-12
**Approach:** Per-wave gate retry with cascade reset of future waves
**Total Segments:** 5
**Estimated Time:** 3-4 hours
**Risk Budget:** 32.5% (13/40 points)

---

## Goal

Add capability for operators to retry failed wave validation gates via dashboard. When a gate is retried, all segments in future waves (waves after the gate's wave) should reset to pending status, unless they have already passed. This ensures downstream work gets re-validated after fixing issues that caused gate failure.

---

## Approach Overview

**Per-Wave Gate Retry Pattern:**
1. Operator clicks "Retry Gate" button on failed gate bar in dashboard
2. System executes gate command again for that wave
3. Gate result logged with attempt number
4. If gate passes: Reset all segments in future waves (except pass/complete) to pending
5. Orchestrator automatically continues to next wave
6. Dashboard shows gate retry count and streams new log

**Why this approach:**
- Matches Azure DevOps manual retry pattern (industry standard)
- Leverages existing async execution infrastructure
- Adapts proven _mark_dependents_skipped cascade logic
- Low blast radius - only affects future waves, not current/past
- Clear audit trail via events table + gate_attempts table

---

## Dependency Graph

```
Issue 1 (Database Schema)
    ↓
Issue 2 (Gate Execution Refactor) ─┬→ Issue 3 (Cascade Reset Logic)
                                    │        ↓
                                    └→ Issue 4 (API Endpoint)
                                            ↓
                                       Issue 5 (Dashboard UI)
```

**Parallelization:** Issues 2 and 3 can start after Issue 1, but Issue 4 requires both 2 and 3. Issue 5 requires Issue 4.

---

## Issue Analysis Briefs

### Issue 1: Gate Retry Tracking in Database

**Core Problem:** Gate results are currently logged to events table with minimal metadata (just "Wave N gate: PASS/FAIL" string). There's no structured storage of attempt counts, exit codes, or historical tracking per wave. This makes it impossible to display "Attempt 2/3" in UI or implement retry limits.

**Root Cause:** Original design assumed gates would only run once per wave (during normal orchestration flow). Operator retry capability was not part of initial requirements.

**Proposed Fix:** Add `gate_attempts` table to track each gate execution with structured metadata:
- Schema: `wave, attempt, started_at, finished_at, passed, exit_code, log_file`
- New StateDB methods: `record_gate_attempt()`, `get_gate_attempts(wave)`, `get_latest_gate_attempt(wave)`
- Continue logging to events table for backwards compatibility
- Gate logs get unique names: `gate-W{wave}-attempt{N}.log`

**Existing Solutions Evaluated:**
- **SQLite native features**: Use triggers to auto-track attempts → Rejected: Adds complexity, harder to debug
- **Time-series DB (InfluxDB)**: Store gate metrics → Rejected: Overkill for this use case, adds dependency
- **File-based tracking**: Store attempts in JSON files → Rejected: Less queryable than DB
- **Events table only**: Continue storing in events table with structured detail JSON → Adopted: Add dedicated table for structured queries while keeping event logs

**Alternatives Considered:**
- Embed attempt count in segments table → Rejected: Gates are not segments
- Store in run_meta as JSON → Rejected: Not queryable, no per-wave isolation

**Pre-Mortem - What Could Go Wrong:**
1. **Migration fails on existing databases** → Add IF NOT EXISTS, test on copy of production state.db
2. **Concurrent gate attempts** → Use attempt number from auto-increment ID, not COUNT(*)
3. **Log file collisions** → Include timestamp in filename if attempt numbers collide
4. **Unbounded growth** → Add cleanup logic to prune old attempts (keep last 10 per wave)

**Risk Factor:** 2/10 (isolated schema addition, low coupling)

**Evidence for Optimality:**
- **Codebase**: `segment_interjections` table (state.py:50-57) follows same pattern for operator actions
- **Project conventions**: All orchestrator state in SQLite, WAL mode for concurrent access
- **Industry standard**: Separate audit tables for retryable actions (Azure DevOps retention policies)

**Blast Radius:**
- Direct changes: `state.py` (+40 lines)
- Potential ripple: Dashboard queries, gate log display logic

---

### Issue 2: Refactor Gate Execution for Reusability

**Core Problem:** `_run_gate()` function is defined in `__main__.py` and tightly coupled to orchestration loop context. Monitor API handlers in `monitor.py` cannot call it because: (1) it's not imported/importable, (2) monitor doesn't have access to `config.gate_command`, (3) creates circular import if monitor imports from __main__.

**Root Cause:** Gate execution was designed as orchestration-internal logic, not as reusable infrastructure. Monitor was never intended to trigger long-running operations.

**Proposed Fix:**
1. Create new module: `scripts/orchestrate_v2/gate_runner.py`
2. Move `_run_gate()` function to this module, rename to `run_gate()`
3. Accept `gate_command` as parameter (not config object)
4. Return structured result: `GateResult(passed, exit_code, output, log_file)`
5. Update __main__.py to import and call `run_gate()`
6. Pass `config` object to monitor via `request.app["config"]`
7. Monitor can now call `run_gate(config.gate_command, log_dir, wave)`

**Existing Solutions Evaluated:**
- **Direct import from __main__**: `from . import __main__` → Rejected: Circular import risk, violates module boundaries
- **RPC/message queue**: Use asyncio queues to send gate requests → Rejected: Over-engineered for single-process app
- **Subprocess runner library**: Use external tool (Fabric, Invoke) → Rejected: Adds dependency, doesn't integrate with logging
- **Shared utility module**: Extract to `gate_runner.py` → Adopted: Standard Python pattern for shared async functions

**Alternatives Considered:**
- Keep in __main__, expose via app context callback → Rejected: Messy, hard to test
- Duplicate gate logic in monitor → Rejected: DRY violation, maintenance burden

**Pre-Mortem - What Could Go Wrong:**
1. **Circular import if gate_runner imports StateDB** → Mitigation: Only pass primitives (str, Path), not objects
2. **Monitor calls gate while orchestration also running gate** → Mitigation: SQLite WAL + asyncio single-thread prevents collision
3. **Config not available in monitor context** → Mitigation: Modify monitor initialization to pass config
4. **Gate log file conflicts** → Mitigation: Attempt numbering from Issue 1 prevents collisions

**Risk Factor:** 4/10 (touches core execution path, potential for import issues)

**Evidence for Optimality:**
- **Codebase**: `runner.py` already extracted for segment execution, follows same pattern
- **Project conventions**: Async subprocess execution via asyncio.create_subprocess_shell (used in runner.py:449-460)
- **Python best practices**: Extract shared functions to utility modules (PEP 8 module organization)

**Blast Radius:**
- Direct changes: New `gate_runner.py` (+80 lines), `__main__.py` (-40 lines, +5 import), `monitor.py` (+10 lines setup)
- Potential ripple: Any other code calling `_run_gate()` (currently none)

---

### Issue 3: Cascade Reset Logic for Future Waves

**Core Problem:** When a gate is retried and passes, segments in future waves (waves > gate's wave) may have been skipped or failed due to the original gate failure. These segments need to be reset to pending so they can execute when orchestration resumes. Current cascade logic (`_mark_dependents_skipped`) only handles forward-propagation of failures, not backward-propagation of fixes.

**Root Cause:** Original design assumed gates would stop orchestration permanently on failure. No recovery/retry path was implemented, so there's no "un-skip" or "reset future work" logic.

**Proposed Fix:**
1. Create `_reset_future_wave_segments()` function in `__main__.py`
2. Query all segments with `wave > gate_wave`
3. Filter to segments with status in: `skipped-gate-failed`, `blocked`, `failed`, `partial`
4. Exclude segments with status: `pass`, `running`, `pending` (preserve completed/in-progress work)
5. Call `state.reset_for_retry(seg_num)` for each eligible segment
6. Log event: `gate_retry_cascade` with count of reset segments
7. Return list of reset segment numbers for API response

**Existing Solutions Evaluated:**
- **Transitive dependency walk**: Use `_mark_dependents_skipped` pattern → Rejected: Not needed - wave-based reset is simpler
- **Status machine state transitions**: Define explicit FSM for segment status → Rejected: Over-engineered for this use case
- **Database CASCADE DELETE triggers**: SQLite triggers to auto-reset → Rejected: Doesn't fit use case (no deletes)
- **Simple wave-based query**: Filter by wave number and status → Adopted: Matches wave-based execution model

**Alternatives Considered:**
- Reset ALL future segments unconditionally → Rejected: Loses completed work, user explicitly wants to preserve pass
- Reset only segments that directly depend on gate's wave → Rejected: Misses transitive dependencies
- Queue resets for next orchestrator cycle → Rejected: User wants immediate reset

**Pre-Mortem - What Could Go Wrong:**
1. **Reset segment that's currently running** → Mitigation: Exclude status="running" from reset filter
2. **Race with orchestrator starting next wave** → Mitigation: Orchestrator validates dependencies at segment start (existing logic)
3. **Reset too many segments** → Mitigation: Clear logging of what was reset, operator can skip individual segments if needed
4. **Database transaction fails mid-reset** → Mitigation: Use existing auto-commit pattern - partial resets are safe
5. **Future wave already completed** → Mitigation: Filter by status - pass segments won't be reset

**Risk Factor:** 5/10 (modifies orchestration state, potential for unintended resets)

**Evidence for Optimality:**
- **Codebase**: `_mark_dependents_skipped` (lines 407-455) provides proven cascade pattern
- **Codebase**: `reset_for_retry` (state.py:365-377) is idempotent - safe to call multiple times
- **Industry standard**: Azure DevOps resets downstream stages when approvals are retried
- **Research**: Wave-based reset simpler than dependency graph traversal for this use case

**Blast Radius:**
- Direct changes: `__main__.py` (+60 lines new function)
- Potential ripple: Segment status transitions, orchestrator resume logic

---

### Issue 4: API Endpoint for Gate Retry

**Core Problem:** No API endpoint exists for operators to trigger gate retry. Current `/api/control` actions (skip, retry, kill, interject, set_status) operate on segments, not gates. Need new action type that accepts wave number instead of seg_num.

**Root Cause:** Gates were designed as orchestration-internal checkpoints, not operator-controllable actions.

**Proposed Fix:**
1. Add `retry_gate` action to `/api/control` endpoint in `monitor.py`
2. Request format: `{"action": "retry_gate", "wave": N}`
3. Validation: Check wave number is valid (1 <= wave <= max_wave)
4. Check gate_command is configured: `if not config.gate_command: return 400`
5. Execute gate via `run_gate()` from gate_runner module
6. Record attempt via `state.record_gate_attempt()`
7. If gate passes: Call `_reset_future_wave_segments()` (or expose via state method)
8. Log event: `operator_retry_gate` with result
9. Response: `{"ok": true, "wave": N, "passed": bool, "reset_count": M}`

**Existing Solutions Evaluated:**
- **Separate /api/gates/retry endpoint**: REST-ful approach → Rejected: Adds endpoint, all control actions in /api/control
- **GET request with query params**: `/api/gates/W1/retry` → Rejected: GET shouldn't have side effects
- **WebSocket for async execution**: Real-time gate status → Rejected: Existing SSE pattern sufficient
- **Extend /api/control with gate actions**: Add to existing endpoint → Adopted: Consistent with segment control pattern

**Alternatives Considered:**
- Async return (return immediately, execute in background) → Rejected: User wants to see immediate result
- Blocking return (wait for gate to complete) → Adopted: Matches existing interject pattern, gates are fast (<2 min)

**Pre-Mortem - What Could Go Wrong:**
1. **Gate execution times out** → Mitigation: Use existing subprocess timeout pattern (if implemented), or return after 5 minutes
2. **Multiple concurrent retry requests** → Mitigation: Asyncio single-thread prevents true concurrency, WAL allows queuing
3. **Gate passes but reset fails** → Mitigation: Return success with error details in response: `{"ok": true, "passed": true, "reset_error": "..."}`
4. **Config not available in monitor** → Mitigation: Issue 2 already addresses this
5. **Circular import for _reset_future_wave_segments** → Mitigation: Move to state.py as `reset_future_wave_segments(gate_wave)` method

**Risk Factor:** 6/10 (complex action combining multiple operations, potential for partial failures)

**Evidence for Optimality:**
- **Codebase**: `interject` action (monitor.py:103-148) shows pattern for multi-step operations in control endpoint
- **Codebase**: All operator actions go through `/api/control` for consistency
- **Industry standard**: POST to control endpoint matches Azure DevOps retry API
- **Research**: Blocking return acceptable for gates (<2 min execution time)

**Blast Radius:**
- Direct changes: `monitor.py` (+70 lines), `state.py` (+20 lines if adding reset method)
- Potential ripple: Dashboard API calls, event logging

---

### Issue 5: Dashboard UI for Gate Retry

**Core Problem:** Dashboard currently displays gate status (pass/fail/pending/running) in timeline between waves, but there's no interactive UI element to trigger retry. Operators cannot recover from gate failures without restarting orchestrator manually.

**Root Cause:** Gates were designed as informational checkpoints, not interactive control points.

**Proposed Fix:**
1. Add "Retry" button to failed gate bars in timeline (only show when status=fail)
2. Button placement: Right side of gate bar, similar to segment action buttons
3. Click handler: `window.retryGate = async function(wave) { ... }`
4. Prompt for confirmation: "Retry gate for Wave N? This will re-run validation and reset future waves."
5. POST to `/api/control` with `action: "retry_gate", wave: N`
6. Show loading spinner during execution (gates can take 30s-2min)
7. On success:
   - Update gate status in timeline (pass/fail)
   - Show alert: "Gate passed! N segments reset in future waves."
   - Refresh state via `refreshState()`
8. Display attempt count on gate bar: "Gate W1 (attempt 2)" if retry has happened
9. Gate log viewer: Show dropdown to select attempt: "Attempt 1", "Attempt 2", etc.

**Existing Solutions Evaluated:**
- **Inline retry in gate bar**: Button directly in gate HTML → Adopted: Matches segment button pattern
- **Modal dialog**: Full-screen modal for retry → Rejected: Over-engineered for simple action
- **Context menu**: Right-click for gate actions → Rejected: Not discoverable
- **Automatic retry polling**: Dashboard auto-retries failed gates → Rejected: User wants manual control

**Alternatives Considered:**
- Show retry button always (even for passing gates) → Rejected: Confusing UX, retry shouldn't be needed for passing gates
- Retry button in header/toolbar → Rejected: Not obvious which gate it applies to
- Keyboard shortcut (e.g., 'r' key on selected gate) → Deferred: Can add later for power users

**Pre-Mortem - What Could Go Wrong:**
1. **User clicks retry while gate is already running** → Mitigation: Disable button when gate status=running
2. **API call times out** → Mitigation: Show timeout message after 5 minutes, let user refresh to see result
3. **Gate passes but UI doesn't update** → Mitigation: Aggressive `refreshState()` after response, poll every 2s during retry
4. **Attempt count doesn't display** → Mitigation: Backend includes attempt_num in /api/state response
5. **Log viewer doesn't show latest attempt** → Mitigation: Default to latest attempt, allow selection via dropdown

**Risk Factor:** 4/10 (UI changes, potential for confusing UX if not tested well)

**Evidence for Optimality:**
- **Codebase**: Existing segment action buttons (skip/kill/retry) at dashboard.html:845-850 provide UI pattern
- **Codebase**: `interjectSeg()` function (dashboard.html:915-964) shows async POST pattern with confirmation
- **UI/UX research**: Inline action buttons standard for item-level operations (GitHub PR buttons, Jira issue actions)
- **Industry standard**: Azure DevOps shows retry button on failed stage checks

**Blast Radius:**
- Direct changes: `dashboard.html` (+100 lines JS/CSS)
- Potential ripple: Gate status rendering, log viewer, state polling

---

## Segment Index

| # | Slug | Title | Status | Risk | Complexity | LOC | Dependencies |
|---|------|-------|--------|------|------------|-----|--------------|
| 1 | gate-attempts-table | Gate Attempts Tracking Table | pending | 2/10 | Low | 50 | None |
| 2 | gate-runner-module | Gate Runner Module Refactor | pending | 4/10 | Low | 95 | None |
| 3 | cascade-reset-logic | Future Wave Cascade Reset | pending | 5/10 | Medium | 70 | 1 |
| 4 | retry-gate-endpoint | Retry Gate API Endpoint | pending | 6/10 | Medium | 90 | 2, 3 |
| 5 | retry-gate-ui | Retry Gate Dashboard UI | pending | 4/10 | Low | 110 | 4 |

**Total Estimated Lines:** ~415 lines across 5 files

---

## Execution Order

1. **Segment 1** - Gate Attempts Table (foundation, enables retry tracking)
2. **Segments 2 & 3** in parallel:
   - Segment 2 - Gate Runner Module (enables API calls)
   - Segment 3 - Cascade Reset Logic (enables downstream reset)
3. **Segment 4** - Retry Gate Endpoint (combines 2 & 3)
4. **Segment 5** - Retry Gate UI (consumes API from 4)

**Parallelization opportunity:** Segments 2 and 3 can run concurrently after Segment 1.

---

## Segment Briefs

### Segment 1: Gate Attempts Tracking Table

**Goal:** Add database schema to track gate retry attempts with structured metadata.

**Depends on:** None

**Issues addressed:** Issue 1

**Cycle budget:** 10 (Low complexity)

**Scope:** Database schema in `scripts/orchestrate_v2/state.py`

**Key files and context:**
- `state.py` lines 13-73: Current schema definition with CREATE TABLE statements
- `state.py` line 50-57: Example of operator action tracking (`segment_interjections` table)
- `state.py` lines 100-124: `create()` method with schema initialization
- Database uses SQLite with WAL mode for concurrent access
- Tables use AUTOINCREMENT for IDs, REAL for timestamps (Unix epoch)

**Implementation approach:**

1. **Add gate_attempts table** to schema (after line 57):
```sql
CREATE TABLE IF NOT EXISTS gate_attempts (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    wave        INTEGER NOT NULL,
    attempt     INTEGER NOT NULL,
    started_at  REAL NOT NULL,
    finished_at REAL,
    passed      INTEGER NOT NULL DEFAULT 0,
    exit_code   INTEGER,
    log_file    TEXT NOT NULL,
    UNIQUE(wave, attempt)
);
CREATE INDEX IF NOT EXISTS idx_gate_attempts_wave ON gate_attempts(wave);
```

2. **Add database methods** for gate attempt tracking:

```python
async def record_gate_attempt(
    self,
    wave: int,
    attempt: int,
    started_at: float,
    finished_at: float,
    passed: bool,
    exit_code: int,
    log_file: str,
) -> int:
    """Record a gate execution attempt."""
    cur = await self._conn.execute(
        """INSERT INTO gate_attempts
           (wave, attempt, started_at, finished_at, passed, exit_code, log_file)
           VALUES (?, ?, ?, ?, ?, ?, ?)""",
        (wave, attempt, started_at, finished_at, 1 if passed else 0, exit_code, log_file),
    )
    await self._conn.commit()
    return cur.lastrowid

async def get_gate_attempts(self, wave: int, limit: int = 10) -> list[dict]:
    """Get all attempts for a wave, most recent first."""
    cur = await self._conn.execute(
        """SELECT id, wave, attempt, started_at, finished_at, passed, exit_code, log_file
           FROM gate_attempts
           WHERE wave = ?
           ORDER BY attempt DESC
           LIMIT ?""",
        (wave, limit),
    )
    rows = await cur.fetchall()
    return [
        {
            "id": r[0],
            "wave": r[1],
            "attempt": r[2],
            "started_at": r[3],
            "finished_at": r[4],
            "passed": bool(r[5]),
            "exit_code": r[6],
            "log_file": r[7],
        }
        for r in rows
    ]

async def get_latest_gate_attempt(self, wave: int) -> dict | None:
    """Get the most recent gate attempt for a wave."""
    attempts = await self.get_gate_attempts(wave, limit=1)
    return attempts[0] if attempts else None
```

3. **Update all_as_dict()** to include gate attempts in state export (around line 464):
```python
gate_attempts_raw = await self._conn.execute(
    "SELECT wave, attempt, started_at, finished_at, passed, exit_code, log_file FROM gate_attempts ORDER BY wave, attempt"
)
gate_attempts = [
    {"wave": r[0], "attempt": r[1], "started_at": r[2], "finished_at": r[3],
     "passed": bool(r[4]), "exit_code": r[5], "log_file": r[6]}
    for r in await gate_attempts_raw.fetchall()
]
```

4. **Migration safety:**
- Uses `IF NOT EXISTS` - safe to run on existing databases
- No data migration needed (new table, no existing data)
- Index creation is also `IF NOT EXISTS`

**Alternatives ruled out:**
- Store in events table only: Not queryable, requires string parsing
- Store in run_meta as JSON: Not queryable per wave
- Add to segments table: Gates are not segments

**Pre-mortem risks:**
- Schema migration fails on locked database → Test on copy first
- UNIQUE constraint conflicts → Use INSERT OR IGNORE pattern
- Unbounded growth → Add cleanup logic (keep last 10 attempts per wave)

**Segment-specific commands:**

**Build:** `cargo build --workspace`

**Test (targeted):**
```python
python3 -c "
import asyncio
from pathlib import Path
from scripts.orchestrate_v2.state import StateDB

async def test():
    db_path = Path('/tmp/test_gate_attempts.db')
    db_path.unlink(missing_ok=True)

    db = await StateDB.create(db_path)

    # Test record_gate_attempt
    id1 = await db.record_gate_attempt(
        wave=1,
        attempt=1,
        started_at=1234567890.0,
        finished_at=1234567920.0,
        passed=False,
        exit_code=1,
        log_file='gate-W1-attempt1.log'
    )
    print(f'✓ Recorded gate attempt: {id1}')

    # Test get_gate_attempts
    attempts = await db.get_gate_attempts(wave=1)
    assert len(attempts) == 1
    assert attempts[0]['wave'] == 1
    assert attempts[0]['attempt'] == 1
    assert attempts[0]['passed'] == False
    assert attempts[0]['exit_code'] == 1
    print('✓ Query gate attempts works')

    # Test get_latest_gate_attempt
    latest = await db.get_latest_gate_attempt(wave=1)
    assert latest is not None
    assert latest['attempt'] == 1
    print('✓ Get latest attempt works')

    # Test multiple attempts
    id2 = await db.record_gate_attempt(
        wave=1,
        attempt=2,
        started_at=1234567950.0,
        finished_at=1234567980.0,
        passed=True,
        exit_code=0,
        log_file='gate-W1-attempt2.log'
    )

    latest = await db.get_latest_gate_attempt(wave=1)
    assert latest['attempt'] == 2
    assert latest['passed'] == True
    print('✓ Multiple attempts tracked correctly')

    # Test UNIQUE constraint
    try:
        await db.record_gate_attempt(
            wave=1, attempt=2, started_at=1.0, finished_at=2.0,
            passed=False, exit_code=1, log_file='dupe.log'
        )
        assert False, 'Should have raised UNIQUE constraint error'
    except Exception as e:
        assert 'UNIQUE' in str(e)
        print('✓ UNIQUE constraint enforced')

    # Test all_as_dict includes gate_attempts
    state_dict = await db.all_as_dict()
    assert 'gate_attempts' in state_dict
    assert len(state_dict['gate_attempts']) == 2
    print('✓ all_as_dict() includes gate_attempts')

    await db.close()
    db_path.unlink()
    print('All tests passed!')

asyncio.run(test())
"
```

**Test (regression):**
```bash
# Verify existing state methods still work
python3 -c "
import asyncio
from pathlib import Path
from scripts.orchestrate_v2.state import StateDB

async def test():
    db = await StateDB.create(Path('/tmp/test_regression.db'))
    # Test existing table queries
    tables = await db._conn.execute(\"SELECT name FROM sqlite_master WHERE type='table'\")
    table_names = [r[0] for r in await tables.fetchall()]
    expected = ['events', 'notifications', 'run_meta', 'segment_attempts', 'segment_interjections', 'segments', 'gate_attempts']
    # Allow for sqlite_sequence
    for t in expected:
        assert t in table_names, f'Missing table: {t}'
    print('✓ All expected tables exist')
    await db.close()
    Path('/tmp/test_regression.db').unlink()

asyncio.run(test())
"
```

**Test (full gate):**
```bash
# Integration test with real orchestrator
# 1. Copy existing state.db to backup
# 2. Start orchestrator on small plan
# 3. Let it complete at least one wave
# 4. Check gate_attempts table is populated
# 5. Verify schema upgrade on existing database
```

**Exit criteria:**
1. [ ] gate_attempts table created with correct schema
2. [ ] Unique index on (wave, attempt)
3. [ ] record_gate_attempt() inserts rows correctly
4. [ ] get_gate_attempts() returns attempts in DESC order
5. [ ] get_latest_gate_attempt() returns most recent
6. [ ] UNIQUE constraint enforced (duplicate attempts fail)
7. [ ] all_as_dict() includes gate_attempts
8. [ ] Targeted test passes all assertions
9. [ ] Regression test confirms no table regressions
10. [ ] Schema safe to run on existing state.db files

**Risk factor:** 2/10

**Estimated complexity:** Low

**Commit message:**
```
feat(orchestrate): add gate_attempts table for retry tracking

Add structured database table to track gate execution attempts.
Enables UI to display retry counts and historical logs.

- Add gate_attempts table with wave, attempt, timestamps, result
- Add record_gate_attempt(), get_gate_attempts() methods
- Include gate_attempts in all_as_dict() state export
- UNIQUE constraint on (wave, attempt) prevents duplicates
- Index on wave for fast queries
```

---

### Segment 2: Gate Runner Module Refactor

**Goal:** Extract gate execution logic to reusable module accessible from both orchestrator and monitor.

**Depends on:** None (can run in parallel with Segment 1 and 3)

**Issues addressed:** Issue 2

**Cycle budget:** 10 (Low complexity - mostly code movement)

**Scope:** New module `scripts/orchestrate_v2/gate_runner.py`, refactor `__main__.py` and `monitor.py`

**Key files and context:**
- `__main__.py` lines 33-76: Current `_run_gate()` implementation
- `monitor.py` lines 29-36: `_create_app()` function that sets up request.app context
- `monitor.py` line 300: `MonitorServer.__init__()` signature
- `__main__.py` line 901: Monitor instantiation
- `config.py` lines 97-124: OrchestrateConfig dataclass with gate_command field
- Async subprocess pattern used throughout codebase (runner.py:449-460)

**Implementation approach:**

1. **Create gate_runner.py module** with extracted logic:

```python
"""Gate execution for orchestrator validation."""
import asyncio
import logging
from pathlib import Path
from dataclasses import dataclass

log = logging.getLogger(__name__)

@dataclass
class GateResult:
    """Result of gate execution."""
    passed: bool
    exit_code: int
    output: str
    log_file: Path
    duration: float

async def run_gate(
    gate_command: str,
    log_dir: Path,
    wave: int,
    attempt: int = 1,
) -> GateResult:
    """Execute gate command and stream output to log file.

    Args:
        gate_command: Shell command to execute
        log_dir: Directory for log files
        wave: Wave number (for log filename)
        attempt: Attempt number (for log filename)

    Returns:
        GateResult with execution details
    """
    if not gate_command:
        return GateResult(
            passed=True,
            exit_code=0,
            output="no gate configured",
            log_file=log_dir / "no-gate.log",
            duration=0.0,
        )

    log_file = log_dir / f"gate-W{wave}-attempt{attempt}.log"
    log.info("Running gate (wave=%d, attempt=%d): %s", wave, attempt, gate_command)

    start_time = asyncio.get_event_loop().time()

    proc = await asyncio.create_subprocess_shell(
        gate_command,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.STDOUT,
    )

    lines = []
    with log_file.open("w", encoding="utf-8") as f:
        async for line_bytes in proc.stdout:
            line = line_bytes.decode("utf-8", errors="replace").rstrip()
            lines.append(line)
            f.write(line + "\n")
            f.flush()

    await proc.wait()

    duration = asyncio.get_event_loop().time() - start_time
    passed = proc.returncode == 0

    log.info(
        "Gate %s (exit=%d, duration=%.1fs)",
        "PASSED" if passed else "FAILED",
        proc.returncode,
        duration,
    )

    return GateResult(
        passed=passed,
        exit_code=proc.returncode,
        output="\n".join(lines),
        log_file=log_file,
        duration=duration,
    )
```

2. **Update __main__.py** to use new module:

Replace `_run_gate()` function (lines 33-76) with:
```python
from .gate_runner import run_gate, GateResult

# Remove _run_gate() definition

# Update call site (line 1101):
gate_result = await run_gate(
    config.gate_command,
    log_dir,
    wave_num,
    attempt=1,  # First attempt during normal orchestration
)
gate_ok = gate_result.passed

# Update logging (line 1105):
await state.log_event(
    "gate_result",
    f"Wave {wave_num} gate: {'PASS' if gate_result.passed else 'FAIL'}",
)

# Record in database (new):
await state.record_gate_attempt(
    wave=wave_num,
    attempt=1,
    started_at=start_time,
    finished_at=asyncio.get_event_loop().time(),
    passed=gate_result.passed,
    exit_code=gate_result.exit_code,
    log_file=str(gate_result.log_file.name),
)
```

3. **Pass config to monitor**:

Update `_create_app()` signature (line 29):
```python
def _create_app(
    state: StateDB,
    log_dir: Path,
    plan_root: Path,
    running_pids: dict,
    config: "OrchestrateConfig",  # NEW
) -> web.Application:
    app = web.Application()
    app["state"] = state
    app["log_dir"] = log_dir
    app["plan_root"] = plan_root
    app["running_pids"] = running_pids
    app["config"] = config  # NEW
    # ... rest of setup
```

Update MonitorServer.__init__() (line 300):
```python
def __init__(
    self,
    state: StateDB,
    log_dir: Path,
    plan_root: Path,
    running_pids: dict,
    config: "OrchestrateConfig",  # NEW
):
    self._state = state
    self._log_dir = log_dir
    self._plan_root = plan_root
    self._running_pids = running_pids
    self._config = config  # NEW
    self._app = _create_app(state, log_dir, plan_root, running_pids, config)  # Pass config
```

Update instantiation (line 901):
```python
monitor = MonitorServer(state, log_dir, plan_root, _running_pids, config)  # Add config
```

4. **Verify imports don't create cycles:**
- gate_runner.py only imports stdlib (asyncio, logging, pathlib, dataclasses)
- No imports from other orchestrate_v2 modules
- __main__.py imports from gate_runner (fine - __main__ is top-level)
- monitor.py will import gate_runner in next segment (also fine)

**Alternatives ruled out:**
- Keep _run_gate in __main__, pass via callback: Messy, hard to test
- Duplicate logic in monitor: DRY violation
- Use RPC/queues: Over-engineered

**Pre-mortem risks:**
- Circular import if gate_runner imports StateDB: Mitigated by passing primitives only
- Config not available in monitor: Fixed by passing in __init__
- Gate log file naming conflicts: Mitigated by attempt numbering

**Segment-specific commands:**

**Build:** `cargo build --workspace`

**Test (targeted):**
```python
python3 -c "
import asyncio
from pathlib import Path
from scripts.orchestrate_v2.gate_runner import run_gate, GateResult

async def test():
    log_dir = Path('/tmp/test_gate_runner')
    log_dir.mkdir(exist_ok=True)

    # Test successful gate
    result = await run_gate('echo test && exit 0', log_dir, wave=1, attempt=1)
    assert result.passed == True
    assert result.exit_code == 0
    assert 'test' in result.output
    assert result.log_file.exists()
    print('✓ Successful gate execution')

    # Test failed gate
    result = await run_gate('echo failed && exit 1', log_dir, wave=1, attempt=2)
    assert result.passed == False
    assert result.exit_code == 1
    assert 'failed' in result.output
    print('✓ Failed gate execution')

    # Test no gate configured
    result = await run_gate('', log_dir, wave=1, attempt=3)
    assert result.passed == True
    assert result.exit_code == 0
    assert 'no gate configured' in result.output
    print('✓ No gate configured')

    # Test log file naming
    result = await run_gate('echo attempt3', log_dir, wave=2, attempt=3)
    assert result.log_file.name == 'gate-W2-attempt3.log'
    print('✓ Log file naming correct')

    # Test duration tracking
    result = await run_gate('sleep 0.1', log_dir, wave=1, attempt=4)
    assert result.duration >= 0.1
    print('✓ Duration tracking works')

    # Cleanup
    import shutil
    shutil.rmtree(log_dir)
    print('All tests passed!')

asyncio.run(test())
"
```

**Test (regression):**
```bash
# Verify __main__.py still runs
python3.11 -m scripts.orchestrate_v2 run --dry-run /Users/psauer/probe/.claude/plans/phase3-tui-v2

# Verify gate still executes in orchestration
# (requires manual run with small plan that has gate configured)
```

**Test (full gate):**
```bash
# Integration test
# 1. Start orchestrator on plan with gate command
# 2. Verify gate executes and logs to correct file
# 3. Verify monitor can access config.gate_command
# 4. Check no import errors or circular dependency issues
```

**Exit criteria:**
1. [ ] gate_runner.py module created with run_gate() function
2. [ ] GateResult dataclass includes all needed fields
3. [ ] __main__.py imports and calls run_gate() correctly
4. [ ] _run_gate() function removed from __main__.py
5. [ ] Config passed to monitor via __init__ and _create_app
6. [ ] request.app["config"] accessible in monitor handlers
7. [ ] Log files named with attempt number: gate-W{wave}-attempt{N}.log
8. [ ] Duration tracking works
9. [ ] Targeted test passes all assertions
10. [ ] Regression test confirms orchestrator still runs
11. [ ] No circular import errors

**Risk factor:** 4/10

**Estimated complexity:** Low

**Commit message:**
```
refactor(orchestrate): extract gate execution to reusable module

Move _run_gate() from __main__.py to new gate_runner.py module,
making it callable from monitor API handlers.

- Create gate_runner.py with run_gate() function
- Add GateResult dataclass for structured return value
- Pass OrchestrateConfig to monitor for gate_command access
- Update log file naming to include attempt number
- Add duration tracking to gate results
```

---

### Segment 3: Future Wave Cascade Reset

**Goal:** Implement logic to reset segments in future waves when a gate is retried and passes.

**Depends on:** Segment 1 (needs gate_attempts table and methods)

**Issues addressed:** Issue 3

**Cycle budget:** 15 (Medium complexity - state manipulation logic)

**Scope:** New function in `__main__.py` or method in `state.py`

**Key files and context:**
- `__main__.py` lines 407-455: `_mark_dependents_skipped()` function (pattern to follow)
- `state.py` lines 365-377: `reset_for_retry()` method
- `state.py` lines 338-350: `wave_segments()` method
- Wave execution logic: Segments with `wave` field from planner.py
- Status values to reset: skipped-gate-failed, blocked, failed, partial
- Status values to preserve: pass, running, pending

**Implementation approach:**

1. **Add helper method to state.py** for querying segments by wave and status:

```python
async def get_future_wave_segments(
    self,
    after_wave: int,
    statuses: list[str] | None = None,
) -> list[SegmentRow]:
    """Get all segments in waves after specified wave, optionally filtered by status.

    Args:
        after_wave: Return segments with wave > this value
        statuses: If provided, filter to these status values

    Returns:
        List of SegmentRow dicts
    """
    if statuses:
        placeholders = ",".join("?" * len(statuses))
        query = f"""
            SELECT num, slug, title, wave, status, attempts, started_at, finished_at
            FROM segments
            WHERE wave > ? AND status IN ({placeholders})
            ORDER BY wave, num
        """
        params = [after_wave] + statuses
    else:
        query = """
            SELECT num, slug, title, wave, status, attempts, started_at, finished_at
            FROM segments
            WHERE wave > ?
            ORDER BY wave, num
        """
        params = [after_wave]

    cur = await self._conn.execute(query, params)
    rows = await cur.fetchall()
    return [
        SegmentRow(
            num=r[0], slug=r[1], title=r[2], wave=r[3],
            status=r[4], attempts=r[5], started_at=r[6], finished_at=r[7]
        )
        for r in rows
    ]
```

2. **Add reset method to state.py**:

```python
async def reset_future_wave_segments(
    self,
    after_wave: int,
    reason: str = "gate retry",
) -> list[int]:
    """Reset segments in future waves to pending status.

    Only resets segments that are in terminal failure states.
    Preserves segments that are pass, running, or pending.

    Args:
        after_wave: Reset segments with wave > this value
        reason: Reason for reset (for logging)

    Returns:
        List of reset segment numbers
    """
    # Statuses that should be reset when gate passes
    reset_statuses = [
        "skipped-gate-failed",
        "skipped-dependency-failed",
        "blocked",
        "failed",
        "partial",
        "timeout",
        "unknown",
    ]

    segments = await self.get_future_wave_segments(
        after_wave=after_wave,
        statuses=reset_statuses,
    )

    reset_nums = []
    for seg in segments:
        await self.reset_for_retry(seg.num)
        await self.log_event(
            "gate_retry_cascade",
            f"S{seg.num:02d} reset to pending ({reason}, was {seg.status})",
            severity="info",
        )
        reset_nums.append(seg.num)

    return reset_nums
```

3. **Alternative: Add function to __main__.py** if prefer keeping orchestration logic there:

```python
async def _reset_future_wave_segments(
    gate_wave: int,
    state: StateDB,
    reason: str,
) -> list[int]:
    """Reset segments in waves after gate_wave to pending.

    Similar to _mark_dependents_skipped but for gate retry scenario.
    Only resets segments in terminal failure states, preserves pass/running/pending.
    """
    # Get max wave to know range
    max_wave = await state.max_wave()

    reset_nums = []

    for wave_num in range(gate_wave + 1, max_wave + 1):
        wave_segs = await state.wave_segments(wave_num)

        for seg in wave_segs:
            # Skip if already in good state
            if seg.status in ("pass", "running", "pending"):
                continue

            # Reset if in terminal failure state
            if seg.status in (
                "skipped-gate-failed",
                "skipped-dependency-failed",
                "blocked",
                "failed",
                "partial",
                "timeout",
                "unknown",
            ):
                log.info(
                    "S%02d reset to pending (gate W%d retry, was %s)",
                    seg.num,
                    gate_wave,
                    seg.status,
                )
                await state.reset_for_retry(seg.num)
                await state.log_event(
                    "gate_retry_cascade",
                    f"S{seg.num:02d} reset due to gate W{gate_wave} retry (was {seg.status})",
                    severity="info",
                )
                reset_nums.append(seg.num)

    return reset_nums
```

4. **Decision: Use state.py method** for better encapsulation and reusability.

**Alternatives ruled out:**
- Reset ALL future segments unconditionally: Loses pass work
- Use dependency graph traversal: Wave-based simpler
- Reset only next wave: Misses transitive impact

**Pre-mortem risks:**
- Reset segment that's currently running: Mitigated by preserving "running" status
- Race with orchestrator: Mitigated by re-validation at segment start
- Reset too many: Clear logging, operator can skip if needed

**Segment-specific commands:**

**Build:** `cargo build --workspace`

**Test (targeted):**
```python
python3 -c "
import asyncio
from pathlib import Path
from scripts.orchestrate_v2.state import StateDB
from scripts.orchestrate_v2.planner import Segment

async def test():
    db_path = Path('/tmp/test_cascade_reset.db')
    db_path.unlink(missing_ok=True)

    db = await StateDB.create(db_path)

    # Create test segments in multiple waves
    segments = [
        Segment(num=1, slug='s1', title='S1', wave=1, depends_on=[]),
        Segment(num=2, slug='s2', title='S2', wave=2, depends_on=[1]),
        Segment(num=3, slug='s3', title='S3', wave=2, depends_on=[1]),
        Segment(num=4, slug='s4', title='S4', wave=3, depends_on=[2]),
    ]

    for seg in segments:
        await db.init_segment(seg)

    # Set various statuses
    await db.set_status(1, 'pass')  # Wave 1 complete
    await db.set_status(2, 'failed')  # Wave 2 failed
    await db.set_status(3, 'pass')  # Wave 2 passed
    await db.set_status(4, 'skipped-gate-failed')  # Wave 3 skipped due to gate

    # Test reset after wave 1
    reset = await db.reset_future_wave_segments(after_wave=1, reason='gate W1 retry')

    # Should reset S2 (failed) and S4 (skipped-gate-failed)
    # Should NOT reset S1 (wave 1) or S3 (pass)
    assert 2 in reset, 'Should reset S2 (failed)'
    assert 4 in reset, 'Should reset S4 (skipped-gate-failed)'
    assert 1 not in reset, 'Should not reset S1 (wrong wave)'
    assert 3 not in reset, 'Should not reset S3 (pass status)'
    print(f'✓ Reset correct segments: {reset}')

    # Verify statuses after reset
    s2 = await db.get_segment(2)
    assert s2.status == 'pending', 'S2 should be pending after reset'
    s3 = await db.get_segment(3)
    assert s3.status == 'pass', 'S3 should still be pass'
    s4 = await db.get_segment(4)
    assert s4.status == 'pending', 'S4 should be pending after reset'
    print('✓ Statuses correct after reset')

    # Test with running segment
    await db.set_status(2, 'running')
    reset2 = await db.reset_future_wave_segments(after_wave=1, reason='test')
    assert 2 not in reset2, 'Should not reset running segments'
    print('✓ Running segments preserved')

    # Test events logged
    events = await db._conn.execute(
        \"SELECT kind, detail FROM events WHERE kind='gate_retry_cascade'\"
    )
    event_rows = await events.fetchall()
    assert len(event_rows) >= 2, 'Should log event for each reset'
    print(f'✓ Events logged: {len(event_rows)} events')

    await db.close()
    db_path.unlink()
    print('All tests passed!')

asyncio.run(test())
"
```

**Test (regression):**
```bash
# Verify existing reset_for_retry still works
python3 -c "
import asyncio
from pathlib import Path
from scripts.orchestrate_v2.state import StateDB
from scripts.orchestrate_v2.planner import Segment

async def test():
    db = await StateDB.create(Path('/tmp/test_reset_regression.db'))
    seg = Segment(num=1, slug='test', title='Test', wave=1, depends_on=[])
    await db.init_segment(seg)
    await db.set_status(1, 'failed')

    await db.reset_for_retry(1)

    s = await db.get_segment(1)
    assert s.status == 'pending'
    print('✓ reset_for_retry still works')

    await db.close()
    Path('/tmp/test_reset_regression.db').unlink()

asyncio.run(test())
"
```

**Test (full gate):**
```bash
# Integration test
# 1. Start orchestrator on plan with 3+ waves
# 2. Let gate fail after wave 1
# 3. Manually call reset_future_wave_segments(1)
# 4. Verify segments in waves 2+ are reset to pending
# 5. Verify pass segments are NOT reset
```

**Exit criteria:**
1. [ ] get_future_wave_segments() method added to state.py
2. [ ] reset_future_wave_segments() method added to state.py
3. [ ] Queries segments with wave > specified wave
4. [ ] Filters by status: resets failed/blocked/partial, preserves pass/running/pending
5. [ ] Calls reset_for_retry() for each eligible segment
6. [ ] Logs event for each reset with reason and old status
7. [ ] Returns list of reset segment numbers
8. [ ] Targeted test passes all assertions
9. [ ] Regression test confirms reset_for_retry unchanged
10. [ ] Integration test confirms correct cascade behavior

**Risk factor:** 5/10

**Estimated complexity:** Medium

**Commit message:**
```
feat(orchestrate): add future wave cascade reset for gate retry

Implement logic to reset segments in future waves when a gate passes
after retry. Preserves completed work, resets failed/blocked segments.

- Add get_future_wave_segments() to query by wave range and status
- Add reset_future_wave_segments() to reset eligible segments
- Preserve pass/running/pending segments
- Reset failed/blocked/partial/skipped segments
- Log event for each reset with old status
- Return list of reset segment numbers for API response
```

---

### Segment 4: Retry Gate API Endpoint

**Goal:** Add /api/control action for operators to retry failed gates via HTTP POST.

**Depends on:** Segment 2 (needs gate_runner module), Segment 3 (needs cascade reset logic)

**Issues addressed:** Issue 4

**Cycle budget:** 15 (Medium complexity - combines multiple operations)

**Scope:** Add action handler to `scripts/orchestrate_v2/monitor.py`

**Key files and context:**
- `monitor.py` lines 57-169: `_handle_control()` function with existing actions
- `monitor.py` lines 103-148: `interject` action as pattern (multi-step operation)
- `gate_runner.py`: run_gate() function from Segment 2
- `state.py`: reset_future_wave_segments() from Segment 3
- Response format: `{"ok": bool, "action": str, ...metadata}`
- Error codes: 400 (bad request), 404 (not found), 500 (internal error)

**Implementation approach:**

1. **Add retry_gate action handler** in `_handle_control()` after line 167:

```python
elif action == "retry_gate":
    wave = data.get("wave")
    if not wave:
        return web.json_response(
            {"ok": False, "error": "wave parameter required"},
            status=400,
        )

    wave = int(wave)

    # Validate wave number
    max_wave = await state.max_wave()
    if wave < 1 or wave > max_wave:
        return web.json_response(
            {"ok": False, "error": f"invalid wave number (must be 1-{max_wave})"},
            status=400,
        )

    # Check gate is configured
    config: OrchestrateConfig = request.app["config"]
    if not config.gate_command:
        return web.json_response(
            {"ok": False, "error": "no gate configured in orchestrate.toml"},
            status=400,
        )

    log_dir: Path = request.app["log_dir"]

    try:
        # Get current attempt count
        attempts = await state.get_gate_attempts(wave)
        next_attempt = len(attempts) + 1

        # Log operator action
        await state.log_event(
            "operator_retry_gate",
            f"Wave {wave} gate retry triggered by operator (attempt {next_attempt})",
            severity="warn",
        )

        # Execute gate
        from .gate_runner import run_gate

        start_time = asyncio.get_event_loop().time()
        gate_result = await run_gate(
            config.gate_command,
            log_dir,
            wave,
            attempt=next_attempt,
        )

        # Record attempt in database
        await state.record_gate_attempt(
            wave=wave,
            attempt=next_attempt,
            started_at=start_time,
            finished_at=asyncio.get_event_loop().time(),
            passed=gate_result.passed,
            exit_code=gate_result.exit_code,
            log_file=str(gate_result.log_file.name),
        )

        # Log result event
        await state.log_event(
            "gate_result",
            f"Wave {wave} gate (attempt {next_attempt}): {'PASS' if gate_result.passed else 'FAIL'}",
            severity="info" if gate_result.passed else "error",
        )

        # If gate passed, reset future wave segments
        reset_count = 0
        reset_segments = []
        if gate_result.passed:
            reset_segments = await state.reset_future_wave_segments(
                after_wave=wave,
                reason=f"gate W{wave} retry passed",
            )
            reset_count = len(reset_segments)

            if reset_count > 0:
                await state.log_event(
                    "gate_retry_cascade",
                    f"Gate W{wave} passed on retry, reset {reset_count} segments in future waves: {reset_segments}",
                    severity="info",
                )

        return web.json_response({
            "ok": True,
            "action": "retry_gate",
            "wave": wave,
            "attempt": next_attempt,
            "passed": gate_result.passed,
            "exit_code": gate_result.exit_code,
            "log_file": str(gate_result.log_file.name),
            "duration": gate_result.duration,
            "reset_count": reset_count,
            "reset_segments": reset_segments,
        })

    except Exception as e:
        await state.log_event(
            "operator_retry_gate_failed",
            f"Wave {wave} gate retry failed: {str(e)}",
            severity="error",
        )
        return web.json_response(
            {"ok": False, "error": str(e)},
            status=500,
        )
```

2. **Import asyncio** at top of monitor.py if not already imported.

3. **Response includes rich metadata** for dashboard display:
   - attempt: Which retry this was
   - passed: Gate result
   - exit_code: Process exit code
   - log_file: Where to find logs
   - duration: How long gate took
   - reset_count: How many segments were reset
   - reset_segments: List of segment numbers

**Alternatives ruled out:**
- Async return (return immediately): User wants to see result
- Separate endpoint: All control actions in /api/control
- Auto-retry on failure: User wants manual control

**Pre-mortem risks:**
- Gate hangs indefinitely: Add timeout (or accept current behavior)
- Multiple simultaneous retries: Asyncio prevents true concurrency
- Gate passes but reset fails: Return partial success with error details

**Segment-specific commands:**

**Build:** `cargo build --workspace`

**Test (targeted):**
```bash
# Manual API testing with curl
# Start orchestrator on test plan first

# Test missing wave parameter
curl -X POST http://localhost:8081/api/control \
  -H 'Content-Type: application/json' \
  -d '{"action":"retry_gate"}'
# Expected: {"ok": false, "error": "wave parameter required"}

# Test invalid wave number
curl -X POST http://localhost:8081/api/control \
  -H 'Content-Type: application/json' \
  -d '{"action":"retry_gate","wave":99}'
# Expected: {"ok": false, "error": "invalid wave number..."}

# Test no gate configured (requires plan without gate)
curl -X POST http://localhost:8081/api/control \
  -H 'Content-Type: application/json' \
  -d '{"action":"retry_gate","wave":1}'
# Expected: {"ok": false, "error": "no gate configured"}

# Test valid retry (requires running orchestrator with gate)
curl -X POST http://localhost:8081/api/control \
  -H 'Content-Type: application/json' \
  -d '{"action":"retry_gate","wave":1}'
# Expected: {"ok": true, "wave": 1, "passed": bool, "reset_count": N, ...}
```

**Test (regression):**
```bash
# Verify existing control actions still work
curl -X POST http://localhost:8081/api/control \
  -H 'Content-Type: application/json' \
  -d '{"action":"skip","seg_num":1}'

curl -X POST http://localhost:8081/api/control \
  -H 'Content-Type: application/json' \
  -d '{"action":"retry","seg_num":1}'

# Verify /api/state still works
curl http://localhost:8081/api/state | python3 -m json.tool
```

**Test (full gate):**
```bash
# Integration test: Full retry flow
# 1. Start orchestrator on plan with failing gate
# 2. Let gate fail after wave 1
# 3. Fix the issue that caused gate to fail
# 4. POST retry_gate action via curl
# 5. Verify response shows passed=true, reset_count > 0
# 6. Check gate_attempts table has 2 rows for wave 1
# 7. Verify segments in wave 2+ reset to pending
# 8. Check events table has operator_retry_gate and gate_retry_cascade events
# 9. Verify orchestrator resumes and completes wave 2
```

**Exit criteria:**
1. [ ] retry_gate action added to /api/control
2. [ ] Validates wave parameter (required, valid range)
3. [ ] Returns 400 if no gate configured
4. [ ] Gets attempt count from gate_attempts table
5. [ ] Executes gate via run_gate() with attempt number
6. [ ] Records attempt in database with result
7. [ ] If passed: calls reset_future_wave_segments()
8. [ ] Logs operator_retry_gate event
9. [ ] Logs gate_result event with attempt number
10. [ ] Returns rich JSON response with metadata
11. [ ] Handles exceptions with 500 error
12. [ ] Targeted curl tests pass
13. [ ] Regression tests show no breakage
14. [ ] Integration test shows full retry flow works

**Risk factor:** 6/10

**Estimated complexity:** Medium

**Commit message:**
```
feat(orchestrate): add retry_gate control action for gate recovery

Add API endpoint for operators to retry failed gates. On success,
resets future wave segments to pending for re-validation.

- Add retry_gate action to /api/control endpoint
- Validate wave parameter and gate configuration
- Execute gate via gate_runner.run_gate()
- Record attempt in gate_attempts table
- Reset future wave segments if gate passes
- Return rich metadata: attempt, result, reset count, duration
- Log operator action and cascade events for audit trail
```

---

### Segment 5: Retry Gate Dashboard UI

**Goal:** Add interactive retry button to failed gate bars in dashboard timeline.

**Depends on:** Segment 4 (needs retry_gate API endpoint)

**Issues addressed:** Issue 5

**Cycle budget:** 15 (Low-medium complexity - UI implementation)

**Scope:** `scripts/orchestrate_v2/dashboard.html`

**Key files and context:**
- `dashboard.html` lines 1239-1337: `renderTimeline()` function (gate rendering)
- `dashboard.html` lines 562-570: `gateStatus()` function (gate status logic)
- `dashboard.html` lines 1318-1337: `_selectGate()` function (gate log viewing)
- `dashboard.html` lines 915-964: `interjectSeg()` function (async POST pattern)
- `dashboard.html` lines 205-212: `.act-btn` CSS (button styling)
- Gate bars render between waves with status classes: pass, fail, running, pending

**Implementation approach:**

1. **Add CSS for gate retry button** (after line 212):

```css
.gate-retry-btn {
  background: var(--cc-orange);
  color: white;
  border: none;
  padding: 4px 12px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 11px;
  margin-left: 8px;
  font-family: inherit;
}

.gate-retry-btn:hover {
  background: #d97706;
  opacity: 0.9;
}

.gate-retry-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.gate-attempt-badge {
  font-size: 10px;
  color: var(--cc-text-dim);
  margin-left: 4px;
}
```

2. **Modify renderTimeline() to add retry button** (around line 1305):

Find the gate bar rendering code and update to:
```javascript
// Gate bar between waves
if (w < maxWave) {
  const gs = gateStatus(events, w, currentWave, wDone);
  const gateClass = gs === 'pass' ? 'pass' : gs === 'fail' ? 'fail' : gs === 'running' ? 'running' : 'pending';

  // Get attempt count for this wave
  const gateAttempts = (data.gate_attempts || []).filter(a => a.wave === w);
  const attemptCount = gateAttempts.length;
  const attemptBadge = attemptCount > 1 ? `<span class="gate-attempt-badge">(attempt ${attemptCount})</span>` : '';

  // Show retry button only for failed gates
  const retryBtn = gs === 'fail'
    ? `<button class="gate-retry-btn" onclick="retryGate(${w})" title="Retry gate validation">Retry</button>`
    : '';

  html += `<div class="gate-bar ${gateClass}" onclick="_selectGate(${w}, '${gs}')">`;
  html += `<span>Gate W${w}${attemptBadge}</span>`;
  html += retryBtn;
  html += `</div>`;
}
```

3. **Add retryGate() JavaScript function** (after line 964):

```javascript
window.retryGate = async function(wave) {
  // Confirm action
  if (!confirm(
    `Retry gate for Wave ${wave}?\n\n` +
    `This will re-run the validation command. If the gate passes, ` +
    `segments in future waves will be reset to pending for re-validation.\n\n` +
    `Continue?`
  )) {
    return;
  }

  // Disable button during execution
  const btn = event.target;
  const originalText = btn.textContent;
  btn.disabled = true;
  btn.textContent = 'Retrying...';

  try {
    const r = await fetch('/api/control', {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({
        action: 'retry_gate',
        wave: wave,
      }),
    });

    const data = await r.json();

    if (!data.ok) {
      alert(`Gate retry failed: ${data.error || 'unknown error'}`);
      return;
    }

    // Show result
    const result = data.passed ? 'PASSED ✓' : 'FAILED ✗';
    const resetMsg = data.reset_count > 0
      ? `\n${data.reset_count} segments reset in future waves.`
      : '';
    const durationMsg = `\nExecution time: ${data.duration.toFixed(1)}s`;

    alert(
      `Gate W${wave} retry complete!\n\n` +
      `Result: ${result}\n` +
      `Attempt: ${data.attempt}${resetMsg}${durationMsg}\n\n` +
      `Check logs for details: ${data.log_file}`
    );

    // Refresh state to show updated status
    refreshState();

    // Auto-select the gate log to show results
    _selectGate(wave, data.passed ? 'pass' : 'fail');

  } catch (e) {
    alert(`Request failed: ${e.message}`);
  } finally {
    // Re-enable button
    btn.disabled = false;
    btn.textContent = originalText;
  }
};
```

4. **Update gate log viewer for attempt selection** (around line 1318):

```javascript
window._selectGate = function(wave, status) {
  const logLabel = document.getElementById('log-seg-label');
  const logContent = document.getElementById('log-content');

  // Get attempts for this wave
  const attempts = (_lastState?.gate_attempts || []).filter(a => a.wave === wave);

  if (attempts.length > 1) {
    // Multiple attempts - show dropdown to select
    const latestAttempt = attempts[0].attempt; // Already sorted DESC
    logLabel.innerHTML = `
      Gate — Wave ${wave}
      <select id="gate-attempt-selector" onchange="selectGateAttempt(${wave})">
        ${attempts.map(a =>
          `<option value="${a.attempt}" ${a.attempt === latestAttempt ? 'selected' : ''}>
            Attempt ${a.attempt} (${a.passed ? 'PASS' : 'FAIL'})
          </option>`
        ).join('')}
      </select>
    `;
    const selectedAttempt = document.getElementById('gate-attempt-selector').value;
    _openLogStream(`/api/logs/gate-W${wave}-attempt${selectedAttempt}`, logContent, status === 'running');
  } else {
    // Single attempt - show default log
    logLabel.textContent = `Gate — Wave ${wave}`;
    _openLogStream(`/api/logs/gate-W${wave}`, logContent, status === 'running');
  }
};

window.selectGateAttempt = function(wave) {
  const attempt = document.getElementById('gate-attempt-selector').value;
  const logContent = document.getElementById('log-content');
  _openLogStream(`/api/logs/gate-W${wave}-attempt${attempt}`, logContent, false);
};
```

5. **Store _lastState globally** to access gate_attempts (add near top of script):

```javascript
let _lastState = null;  // Store last state for attempt lookup

// In refreshState() function, after fetching state:
function refreshState() {
  fetch('/api/state')
    .then(r => r.json())
    .then(data => {
      _lastState = data;  // Store for gate attempt lookup
      // ... rest of rendering
    });
}
```

**Alternatives ruled out:**
- Always-visible retry button: Confusing, only failed gates need retry
- Auto-retry: User wants manual control
- Separate retry page: Inline button more discoverable

**Pre-mortem risks:**
- Button clicks while gate running: Mitigated by disabled state during execution
- API timeout: Show timeout message, allow user to refresh
- Multiple attempts don't display: Mitigated by dropdown selector
- Log viewer doesn't update: Auto-select gate log after retry

**Segment-specific commands:**

**Build:** `cargo build --workspace` (validation only)

**Test (targeted):**
```bash
# Manual UI testing:
# 1. Start orchestrator with plan that has failing gate
# 2. Open http://localhost:8081 in browser
# 3. Wait for wave to complete and gate to fail
# 4. Verify "Retry" button appears on failed gate bar (right side)
# 5. Click retry button
# 6. Verify confirmation dialog appears with clear message
# 7. Click OK
# 8. Verify button shows "Retrying..." and is disabled
# 9. Wait for gate to complete (10-60 seconds)
# 10. Verify alert shows result (PASSED/FAILED, reset count, duration)
# 11. Verify timeline refreshes and shows updated gate status
# 12. Verify gate log viewer opens automatically showing latest attempt
# 13. If multiple attempts: verify dropdown appears to select attempt
# 14. Verify attempt badge shows "(attempt 2)" on gate bar

# Test validation:
# - Cancel confirmation: should do nothing
# - Network error: should show error alert
# - Gate passes: should show reset count
# - Gate fails again: should show fail message
```

**Test (regression):**
```bash
# Verify existing UI features still work:
# 1. Timeline renders correctly with waves and segments
# 2. Segment action buttons work (skip, kill, retry, interject)
# 3. Log viewer shows segment logs
# 4. State refreshes every 5 seconds
# 5. Gate bars clickable to view logs
# 6. No JavaScript errors in browser console
# 7. CSS styling not broken
```

**Test (full gate):**
```bash
# Full integration test:
# 1. Start orchestrator on multi-wave plan with gate
# 2. Inject failure into gate command (e.g., modify script to exit 1)
# 3. Let orchestrator run and gate fail after wave 1
# 4. Click retry button in dashboard
# 5. While retrying, verify button disabled and shows "Retrying..."
# 6. Fix the gate issue (e.g., revert script)
# 7. Verify gate passes on retry
# 8. Verify alert shows reset count > 0
# 9. Check timeline shows segments in wave 2+ reset to pending
# 10. Verify orchestrator resumes and executes wave 2
# 11. Click gate bar, verify attempt dropdown shows both attempts
# 12. Select "Attempt 1" from dropdown, verify log shows failed run
# 13. Select "Attempt 2" from dropdown, verify log shows passed run
```

**Exit criteria:**
1. [ ] CSS added for .gate-retry-btn and .gate-attempt-badge
2. [ ] Retry button appears on failed gate bars only
3. [ ] Button positioned on right side of gate bar
4. [ ] retryGate() function implemented with confirmation
5. [ ] Button disables during execution with "Retrying..." text
6. [ ] POST to /api/control with action=retry_gate
7. [ ] Alert shows result with pass/fail, reset count, duration
8. [ ] Auto-refresh state after retry completes
9. [ ] Auto-select gate log viewer to show results
10. [ ] Attempt badge shows "(attempt N)" when N > 1
11. [ ] Gate log viewer has dropdown for multiple attempts
12. [ ] Dropdown shows "Attempt N (PASS/FAIL)" for each
13. [ ] Selecting attempt loads correct log file
14. [ ] Targeted manual test passes all steps
15. [ ] Regression test shows no UI breakage
16. [ ] Full integration test confirms end-to-end flow

**Risk factor:** 4/10

**Estimated complexity:** Low

**Commit message:**
```
feat(orchestrate): add retry button to failed gate bars in dashboard

Add interactive UI for operators to retry failed gates. Shows attempt
count and allows viewing logs from any attempt.

- Add retry button to failed gate bars (fail status only)
- Implement retryGate() function with confirmation dialog
- Disable button during execution with loading state
- Show result alert with pass/fail, reset count, duration
- Display attempt badge on gate bars: "(attempt N)"
- Add dropdown in log viewer to select attempt when multiple exist
- Auto-select gate log after retry to show results
- Refresh state automatically after retry completes
```

---

## Exit Criteria (Plan-Level)

After all segments complete:

1. [ ] Database has gate_attempts table with proper schema
2. [ ] Gate execution refactored to reusable gate_runner module
3. [ ] Config object accessible in monitor for gate_command
4. [ ] Future wave cascade reset logic implemented
5. [ ] POST /api/control with action=retry_gate executes gate and resets downstream
6. [ ] Dashboard shows retry button on failed gate bars
7. [ ] Attempt count displayed: "(attempt N)"
8. [ ] Gate log viewer allows selecting attempt via dropdown
9. [ ] Full flow works: Gate fails → Click retry → Gate passes → Future waves reset → Orchestrator resumes
10. [ ] No regressions: Existing orchestrator and dashboard functionality intact
11. [ ] All segments committed with proper commit messages

**Verification checklist:**
- [ ] Run orchestrator on multi-wave plan with intentional gate failure
- [ ] Retry gate via dashboard button
- [ ] Verify segments in future waves reset to pending
- [ ] Verify orchestrator automatically resumes to next wave
- [ ] Verify attempt count increments correctly
- [ ] Verify gate logs saved with attempt numbers
- [ ] Check database for gate_attempts records
- [ ] Check events table for operator_retry_gate and gate_retry_cascade events

---

## Testing Strategy

**Per-segment:** Each segment has specific test commands in its brief

**Integration testing:**
1. Start orchestrator on test plan with 3+ waves and gate configured
2. Inject failure into gate command
3. Wait for gate to fail after wave 1
4. Click "Retry" button in dashboard
5. Fix gate issue while retry is executing
6. Verify gate passes on retry
7. Verify segments in waves 2-3 reset to pending (if they were failed/blocked)
8. Verify pass segments NOT reset
9. Check gate_attempts table shows 2 attempts
10. Check events table has operator_retry_gate and gate_retry_cascade
11. Verify orchestrator resumes and executes wave 2

**Regression testing:**
- All existing orchestrator features work (segment execution, status tracking, logs)
- Dashboard UI functional (timeline, log viewer, segment buttons)
- Gate execution during normal orchestration still works
- Database upgrades cleanly on existing state.db files

---

## Risk Mitigation

1. **Database migration:** Test on copy of existing state.db before deploying to production orchestrator runs
2. **Circular imports:** gate_runner.py imports only stdlib, no orchestrate_v2 modules
3. **Concurrent gate retries:** Asyncio single-thread + operator confirmation prevents races
4. **Reset too many segments:** Clear logging, operator can manually skip segments if needed
5. **API timeout:** Use existing subprocess patterns, accept current behavior (no explicit timeout)
6. **Audit trail:** Log all gate retries and cascade resets with operator attribution

---

## Known Limitations

1. **No automatic retry limit:** Operators can retry indefinitely (no max_attempts check)
2. **No retry scheduling:** Retries are immediate, no delayed/scheduled retry
3. **Blocking API call:** retry_gate endpoint blocks until gate completes (acceptable for <2min gates)
4. **No partial reset control:** Resets all eligible future segments, no per-segment control
5. **Single operator context:** No authentication, any operator can retry any gate

---

## Future Enhancements (Out of Scope)

- Automatic retry with backoff policy (configurable max_attempts)
- Scheduled gate retry (e.g., "retry at 2pm")
- Selective segment reset (operator chooses which segments to reset)
- Rich notifications (Slack/email on gate pass/fail)
- Gate retry history viewer in dashboard
- Per-wave retry permissions (restrict who can retry production gates)

---

## References

**Research artifacts:**
- Azure DevOps approvals and checks: Manual intervention pattern
- Existing cascade logic: `_mark_dependents_skipped` (__main__.py:407-455)
- Retry infrastructure: `reset_for_retry` (state.py:365-377)
- Operator action pattern: `interject` (monitor.py:103-148)

**Codebase files modified:**
- `scripts/orchestrate_v2/state.py` - Database schema and methods
- `scripts/orchestrate_v2/gate_runner.py` - New module for gate execution
- `scripts/orchestrate_v2/__main__.py` - Gate execution refactor, cascade reset
- `scripts/orchestrate_v2/monitor.py` - retry_gate API endpoint
- `scripts/orchestrate_v2/dashboard.html` - Retry button UI

---

## Execution Instructions

To execute this plan, use the `/orchestrate` skill:

1. The skill will launch an `iterative-builder` subagent for each segment in order
2. Each builder receives the full segment brief with exit criteria
3. Segments 2 and 3 can run in parallel after Segment 1 completes
4. After all segments complete, run `/deep-verify` to validate exit criteria
5. If verification finds gaps, re-enter `/deep-plan` on unresolved items

**Do not implement segments directly** - always delegate to iterative-builder subagents per the orchestration protocol.

---

## Metadata

**Total estimated scope:**
- 5 segments
- ~415 lines of code
- 3-4 hours estimated time
- Risk budget: 32.5% (13/40 points)
- Complexity distribution: 2 Low, 3 Medium

**Segment risk breakdown:**
- Segment 1: 2/10 (database schema)
- Segment 2: 4/10 (refactoring)
- Segment 3: 5/10 (state manipulation)
- Segment 4: 6/10 (complex API action)
- Segment 5: 4/10 (UI changes)

**Dependencies:**
- Linear: 1 → (2, 3) → 4 → 5
- Parallelization: Segments 2 and 3 after Segment 1
