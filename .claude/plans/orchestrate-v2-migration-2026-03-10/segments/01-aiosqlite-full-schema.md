---
segment: 1
title: "aiosqlite migration + full schema"
depends_on: []
risk: 7/10
complexity: High
cycle_budget: 20
status: pending
commit_message: "refactor(orchestrate_v2): migrate StateDB to aiosqlite, add full schema"
---

# Segment 1: aiosqlite migration + full schema

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Migrate `StateDB` from `sqlite3` to `aiosqlite`, make all state methods `async`, and add the complete new schema needed by all downstream segments.

**Depends on:** None (pre-step copy of `scripts/orchestrate/` → `scripts/orchestrate_v2/` already done)

## Context: Issues Addressed

**Issue 2 — sqlite3 blocking the event loop:**
All `StateDB` methods are synchronous `sqlite3` calls on the asyncio event loop thread. When 4 segments finish simultaneously, `state.set_status()` / `state.log_event()` calls block the loop, stalling the SSE stream, heartbeat tasks, and notification worker. Fix: migrate to `aiosqlite` (v0.22.1) — runs sqlite3 on a dedicated background thread with an internal queue; all blocking I/O off the event loop. Every `StateDB` method becomes `async def`. Risk: missing a single `await` silently returns a coroutine object instead of the value.

**Schema foundation for all other segments:**
This segment also adds all new schema in one place to avoid migration conflicts between S2/S3:
- `notifications` table (S2 outbox)
- `segment_attempts` table (S3 history)
- `severity TEXT DEFAULT 'info'` on `events` (S4 dashboard color coding)
- `last_seen_at REAL`, `last_activity TEXT`, `per_segment_timeout INTEGER` on `segments` (S3 heartbeats)
- `last_attempt_at REAL` on `notifications` (S2 retry backoff timing)

## Scope

- `scripts/orchestrate_v2/state.py` — full rewrite to aiosqlite
- `scripts/orchestrate_v2/__main__.py` — all `state.*` calls add `await`
- `scripts/orchestrate_v2/runner.py` — all `state.*` calls add `await`
- `scripts/orchestrate_v2/monitor.py` — `state.all_as_dict()` adds `await`
- `scripts/orchestrate_v2/requirements.txt` — create with pinned versions

## Key Files and Context

**`scripts/orchestrate_v2/state.py` current `__init__` (lines 64–74):**
```python
self._conn = sqlite3.connect(str(db_path), isolation_level="DEFERRED", check_same_thread=False)
self._conn.execute("PRAGMA journal_mode=WAL")
self._conn.execute("PRAGMA busy_timeout=5000")
self._conn.executescript(_SCHEMA)
self._conn.commit()
```

**New async factory pattern:**
```python
import aiosqlite

class StateDB:
    @classmethod
    async def create(cls, db_path: Path) -> "StateDB":
        conn = await aiosqlite.connect(str(db_path))
        conn.row_factory = aiosqlite.Row
        await conn.execute("PRAGMA journal_mode=WAL")
        await conn.execute("PRAGMA busy_timeout=5000")
        await conn.execute("PRAGMA synchronous=NORMAL")
        await conn.executescript(_SCHEMA)
        for sql in _MIGRATIONS:
            try:
                await conn.execute(sql)
            except Exception:
                pass  # column/table already exists
        await conn.commit()
        obj = object.__new__(cls)
        obj._conn = conn
        obj._path = db_path
        return obj

    async def close(self) -> None:
        await self._conn.close()
```

**New schema additions to append to `_SCHEMA`:**
```sql
CREATE TABLE IF NOT EXISTS notifications (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at     REAL NOT NULL,
    event_key      TEXT NOT NULL UNIQUE,
    kind           TEXT NOT NULL,
    message        TEXT NOT NULL,
    priority       TEXT NOT NULL DEFAULT 'default',
    sent_at        REAL,
    attempts       INTEGER NOT NULL DEFAULT 0,
    last_attempt_at REAL,
    last_error     TEXT
);

CREATE TABLE IF NOT EXISTS segment_attempts (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    seg_num     INTEGER NOT NULL,
    attempt     INTEGER NOT NULL,
    started_at  REAL,
    finished_at REAL,
    status      TEXT,
    summary     TEXT,
    tokens_in   INTEGER DEFAULT 0,
    tokens_out  INTEGER DEFAULT 0
);
```

**`_MIGRATIONS` list (idempotent ALTER TABLE):**
```python
_MIGRATIONS = [
    "ALTER TABLE segments ADD COLUMN last_seen_at REAL",
    "ALTER TABLE segments ADD COLUMN last_activity TEXT",
    "ALTER TABLE segments ADD COLUMN per_segment_timeout INTEGER",
    "ALTER TABLE events ADD COLUMN severity TEXT DEFAULT 'info'",
]
```

**All `StateDB` method changes:**
- `def` → `async def`
- `with self._conn:` → `async with self._conn:`
- `self._conn.execute(...)` → `await self._conn.execute(...)`
- `cur.fetchall()` / `cur.fetchone()` → `await cur.fetchall()` / `await cur.fetchone()`
- `self._conn.executescript(...)` → `await self._conn.executescript(...)`

**New methods to add in this segment:**
```python
async def record_attempt(self, seg_num, attempt, started_at, finished_at,
                          status, summary, tokens_in=0, tokens_out=0) -> None: ...
async def get_attempts(self, seg_num: int) -> list[dict]: ...
async def update_heartbeat(self, num: int, last_seen_at: float, last_activity: str) -> None: ...
async def enqueue_notification(self, kind, message, event_key, priority="default") -> None: ...
async def get_pending_notifications(self, max_attempts: int) -> list[dict]: ...
async def mark_notification_sent(self, notif_id: int) -> None: ...
async def mark_notification_failed(self, notif_id: int, error: str) -> None: ...
async def get_recent_notifications(self, limit: int = 20) -> list[dict]: ...
async def reset_for_retry(self, num: int) -> None: ...
```

**Update `log_event()` signature** to accept `severity: str = "info"` param.

**Update `all_as_dict()`** to include notifications and attempts history per segment:
```python
async def all_as_dict(self) -> dict:
    segments = [dict(s) for s in await self.get_all_segments()]
    for seg in segments:
        seg["attempts_history"] = await self.get_attempts(seg["num"])
    return {
        ...,  # existing fields
        "segments": segments,
        "events": await self.get_events(limit=50),
        "notifications": await self.get_recent_notifications(limit=20),
    }
```

**Callers to update (add `await`):**
- `__main__.py`: `state = StateDB(db_path)` → `state = await StateDB.create(db_path)`, plus all `state.set_status(...)`, `state.log_event(...)`, `state.get_segment(...)`, `state.progress()`, `state.init_segments(...)`, `state.migrate_from_json(...)`, `state.reset_stale_running()`, `state.set_meta(...)`, `state.get_meta(...)`, `state.all_as_dict()`, `state.close()`
- `runner.py`: `state.set_status(...)`, `state.log_event(...)`, `state.increment_attempts(...)`
- `monitor.py`: `state.all_as_dict()`, `state.get_events(...)` in SSE handlers

**`requirements.txt` to create:**
```
aiosqlite>=0.22.1
httpx>=0.28.1
aiohttp>=3.13.3
```

## Implementation Approach

1. Create `requirements.txt` first.
2. Rewrite `state.py` method by method — same public API names and return types, just `async`/`await` added.
3. Update `close()` to `async def close()`.
4. Update `__main__.py` callers (search for `state.` — every occurrence in `_orchestrate_inner`, `_run_wave`, `_run_one`, `_heartbeat_loop`, `cmd_status`, `_run_gate`).
5. Update `runner.py` callers (3 calls in `run_segment()`).
6. Update `monitor.py` handler (`_handle_state`).
7. `cmd_status` and `cmd_dry_run` are sync `def main()` branches — they already call `asyncio.run(...)` internally or can be wrapped.

## Alternatives Ruled Out

- `run_in_executor(ThreadPoolExecutor(max_workers=1))`: manual wrapping per method, same boilerplate as rewriting. Rejected.
- Keep sync + asyncio.Lock: lock prevents concurrent writes but still blocks the event loop thread. Rejected.

## Pre-Mortem Risks

- **Missing `await`**: compiles silently. After migration: `python -m py_compile scripts/orchestrate_v2/*.py` + grep `state\.\(set_status\|log_event\|get_segment\|all_as_dict\|progress\|close\)` not preceded by `await`.
- **`aiosqlite.connect()`**: use `conn = await aiosqlite.connect(path)` (not `async with aiosqlite.connect(path) as conn`) for a long-lived connection.
- **`cmd_status`** is a sync function — ensure its state reads happen inside `asyncio.run()`.
- **`migrate_from_json`** imports `datetime` — must become async since it writes to DB.

## Build and Test Commands

- **Install**: `pip install "aiosqlite>=0.22.1" "httpx>=0.28.1" "aiohttp>=3.13.3"`
- **Build**: `python -m py_compile scripts/orchestrate_v2/*.py`
- **Test (targeted)**:
  ```bash
  python3 -c "
  import asyncio
  from pathlib import Path
  from scripts.orchestrate_v2.state import StateDB
  async def t():
      db = await StateDB.create(Path('/tmp/test_s1.db'))
      async with db._conn.execute(\"SELECT name FROM sqlite_master WHERE type='table'\") as c:
          tables = {r[0] for r in await c.fetchall()}
      assert 'notifications' in tables, f'missing: {tables}'
      assert 'segment_attempts' in tables, f'missing: {tables}'
      async with db._conn.execute('PRAGMA table_info(segments)') as c:
          cols = {r[1] for r in await c.fetchall()}
      for col in ('last_seen_at', 'last_activity', 'per_segment_timeout'):
          assert col in cols, f'missing column: {col}'
      print('PASS')
      await db.close()
  asyncio.run(t())
  "
  ```
- **Test (regression)**: `python -m scripts.orchestrate_v2 dry-run .claude/plans/phase2-coverage-hardening`
- **Test (full gate)**: `python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening`

## Exit Criteria

1. **Targeted tests:**
   - `StateDB.create()` produces a DB with `notifications`, `segment_attempts` tables and new columns: verified above.
   - All existing state ops (set_status, log_event, get_segment, progress, all_as_dict, migrate_from_json) work async.
   - `migrate_from_json` migrates existing `execution-state.json`: counts match.
2. **Regression tests:** `dry-run .claude/plans/phase2-coverage-hardening` exits 0, shows 6-wave plan.
3. **Full build gate:** `python -m py_compile scripts/orchestrate_v2/*.py` — zero errors.
4. **Full test gate:** `status .claude/plans/phase2-coverage-hardening` shows correct segment statuses.
5. **Self-review gate:** Zero `import sqlite3` in `orchestrate_v2/`. Zero un-awaited `state.*` method calls. `requirements.txt` present with all three pinned deps.
6. **Scope verification gate:** Only `scripts/orchestrate_v2/` files modified. `scripts/orchestrate/` untouched.
