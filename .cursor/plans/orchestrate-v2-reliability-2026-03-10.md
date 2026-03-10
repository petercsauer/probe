# Orchestrate v2 — Reliability Hardening

**Goal:** Guarantee iMessage notification delivery (never silently dropped) and make state storage async-safe with mid-run progress heartbeats, via the transactional outbox pattern + aiosqlite migration.
**Generated:** 2026-03-10
**Entry point:** A (Fresh Goal)
**Status:** Ready for execution

---

## Execution Log

| Segment | Est. Complexity | Risk | Cycles Used | Status | Notes |
|---------|----------------|------|-------------|--------|-------|
| 1: Copy + aiosqlite migration | High | 7/10 | -- | -- | -- |
| 2: Notification outbox worker | High | 6/10 | -- | -- | -- |
| 3: Progress heartbeats + stall detection | Medium | 4/10 | -- | -- | -- |

**Deep-verify result:** --
**Follow-up plans:** --

---

## Pre-Step: Backup + Create orchestrate_v2

Before any segment runs, the orchestration agent must:

```bash
# Already done (backup):
cp -r scripts/orchestrate scripts/orchestrate_backup

# Create v2 as a working copy:
cp -r scripts/orchestrate scripts/orchestrate_v2
# Update the package name inside __init__.py if it has one (likely empty, no change needed)
```

The `scripts/orchestrate_v2/` package is what all three segments modify. The original `scripts/orchestrate/` (and `scripts/orchestrate_backup/`) are left untouched.

---

## Issue Analysis Briefs

### Issue 1: Notifications silently dropped on osascript failure

**Core Problem:**
`notify.py`'s `_send_imessage()` makes a single osascript call with a 15-second timeout. If it times out or returns non-zero, `Notifier.send()` silently swallows the failure. There is no retry, no queue, no persistence. Any transient failure (Messages.app slow, macOS idle delay, rate limiting) permanently loses the notification.

**Root Cause:**
Fire-and-forget async call with no durability layer. macOS `Messages.app` AppleScript has a hardcoded 10-second handler timeout and is documented to delay sends by up to 5 minutes during idle periods. A single attempt with no retry is structurally guaranteed to miss notifications.

**Proposed Fix:**
Transactional outbox pattern:
1. Add a `notifications` table to `state.db`: `(id, created_at, kind, message, sent_at, attempts, last_error)`.
2. `Notifier.enqueue(kind, message)` writes a row to this table atomically (never blocks long, just a DB insert).
3. A dedicated `_notification_worker` asyncio task polls the table every 10 seconds, picks up rows where `sent_at IS NULL AND attempts < max_attempts`, calls osascript, marks `sent_at` on success or increments `attempts` + writes `last_error` on failure.
4. Exponential backoff between retries: 10s → 60s → 300s (configurable via `orchestrate.toml`).
5. Notifications that exhaust all attempts are logged as `notification_failed` events but not retried further.

This gives "at least once" delivery: if osascript eventually succeeds (even on the 3rd retry 5 minutes later), the notification arrives. It also survives orchestrator restarts — unsent rows in `state.db` are picked up when the worker starts.

**Existing Solutions Evaluated:**
- `outbox-streaming` (PyPI: `outbox-streaming`, github.com/hyzyla/outbox-streaming): implements transactional outbox for SQLAlchemy + PostgreSQL. Rejected — requires PostgreSQL and SQLAlchemy, neither of which this project uses. Pattern is adopted, library is not.
- `celery` / `rq` / `dramatiq`: full task queue infrastructure. Rejected — wildly over-engineered for a single-machine, single-process script tool with one notification channel.
- Hand-rolled outbox with aiosqlite: **adopted**. Minimal, fits the existing SQLite infrastructure, no new heavyweight dependencies.

**Alternatives Considered:**
- *Increase osascript timeout to 60s*: Does not solve the problem. macOS can delay sends by 5 minutes and a longer timeout just blocks the event loop thread longer. Rejected.
- *Thread-pool for osascript calls*: Decouples the send from the event loop but still no retry or persistence if the process crashes. Rejected.

**Pre-Mortem — What Could Go Wrong:**
- The worker polls every 10s — if Messages.app is down for exactly 9.9s between polls, a notification could be delayed by up to 10s (acceptable).
- The `notifications` table adds a write on every notification event. If the DB is locked under heavy load, `enqueue()` could block. Mitigate with aiosqlite (segment 1) and keep the insert lightweight.
- At-least-once means duplicate notifications if the worker sends successfully but crashes before writing `sent_at`. Mitigate with a unique `event_key` column (hash of kind+detail+ts) and `INSERT OR IGNORE`.
- Worker and segment statuses could write to DB simultaneously — mitigated by aiosqlite's single-writer thread.

**Risk Factor:** 6/10

**Evidence for Optimality:**
- *Codebase*: Current `Notifier.send()` return value is never checked by callers — confirming fire-and-forget is the existing contract. Adding outbox is a pure additive change.
- *External*: Transactional outbox is the 2026 consensus for "at least once" event delivery without a message broker (james-carr.org/posts/2026-01-15, medium.com/@dsbraz Feb 2026).

**Blast Radius:**
- Direct: `state.py` (schema addition), `notify.py` (rewrite), `__main__.py` (worker task)
- Ripple: `config.py` (new retry config fields), `monitor.py` (expose unsent count in dashboard state)

---

### Issue 2: sqlite3 sync calls blocking the asyncio event loop

**Core Problem:**
`StateDB` uses `sqlite3.connect(..., check_same_thread=False)` and all methods are synchronous. When multiple segments finish simultaneously and call `state.set_status()` / `state.log_event()`, these blocking I/O calls execute on the asyncio event loop thread, stalling other coroutines. In the worst case (4 segments finishing at once with a slow disk or lock contention), heartbeat tasks, the monitor SSE stream, and the notification worker all stall.

**Root Cause:**
`check_same_thread=False` disables SQLite's thread-safety check but doesn't make operations non-blocking. All `with self._conn:` context managers are synchronous. There is no `await` in any `StateDB` method, so they cannot yield control back to the event loop while waiting for disk I/O.

**Proposed Fix:**
Migrate `StateDB` to `aiosqlite`. Every method that writes or reads becomes `async def`, every query becomes `await conn.execute(...)`. `aiosqlite` runs the sqlite3 connection on a dedicated background thread with an internal `asyncio.Queue`, so all blocking I/O is off the event loop thread.

API change: all callers in `__main__.py` and `runner.py` that call `state.*` must add `await`.

**Existing Solutions Evaluated:**
- `aiosqlite` (PyPI: `aiosqlite`, github.com/omnilib/aiosqlite): MIT license, actively maintained (2024 performance improvements), 2.5k GitHub stars, standard recommendation for asyncio + SQLite. **Adopted**.
- `databases` (encode/databases): async database abstraction layer. Supports SQLite but adds complexity and an extra abstraction tier. Rejected — overkill for a single-file schema.
- `run_in_executor` with `ThreadPoolExecutor(max_workers=1)`: would work but requires manual wrapping of every method. Rejected in favor of `aiosqlite`'s cleaner API.

**Alternatives Considered:**
- *Keep sync SQLite, add asyncio.Lock*: A lock prevents concurrent writes but still blocks the event loop. Does not solve the core problem. Rejected.
- *Move to PostgreSQL*: No PostgreSQL in this environment, and this is a single-machine tool. Rejected.

**Pre-Mortem — What Could Go Wrong:**
- `aiosqlite` context manager pattern (`async with aiosqlite.connect()`) is different from `sqlite3` — migration must be thorough or some callers will use the old sync interface without errors.
- The `migrate_from_json` method uses `datetime.fromisoformat` — must remain sync-compatible since it's called once at startup before the event loop fully runs. Wrap in `asyncio.to_thread` or call it from a sync context.
- `monitor.py` uses `state.all_as_dict()` inside aiohttp request handlers — these must also become `await` calls.

**Risk Factor:** 7/10

**Evidence for Optimality:**
- *External*: aiosqlite docs (omnilib.dev) explicitly address the "share connection across coroutines" use case and recommend a single long-lived connection — matches existing usage pattern.
- *External*: SO #63813922 confirms `aiosqlite` is preferred over `run_in_executor` for asyncio SQLite work due to deterministic thread behavior.

**Blast Radius:**
- Direct: `state.py` (full rewrite), `__main__.py` (all `state.*` calls become `await`), `runner.py` (same), `monitor.py` (same)
- Ripple: All tests (if any) that instantiate `StateDB`

---

### Issue 3: No mid-run state updates — crash leaves segments in the dark

**Core Problem:**
A segment's row in `state.db` is written twice: once when it starts (`status=running, started_at=now`) and once when it ends. If the orchestrator is killed mid-run, there is no way to know how long a segment has been running, whether it was making progress, or whether it was stuck. The `reset_stale_running` startup fix just resets everything to `pending` — losing all progress info.

**Root Cause:**
No watchdog task updates segment state while it's running. The `run_segment()` coroutine in `runner.py` is a single `asyncio.wait_for(_drain_stdout(), ...)` call — all of the segment's state knowledge is locked inside that coroutine until it completes.

**Proposed Fix:**
A `SegmentHeartbeat` task per running segment:
1. While `_drain_stdout()` is running, a parallel task wakes every 60 seconds.
2. It reads the tail of the segment's `.stream.jsonl` file (last 2KB).
3. Extracts the most recent human-readable text snippet (using `_extract_text_from_stream_line` from `monitor.py`).
4. Writes `last_seen_at=now` and `last_activity=<snippet>` to the segment row.
5. Logs a `segment_heartbeat` event to the events table.
6. If `last_seen_at` hasn't advanced from `started_at` for more than `stall_threshold` seconds (default: 1800s = 30min), enqueues a `segment_stall` notification.

Additionally, add `last_seen_at` and `last_activity` columns to the `segments` schema, and expose them in the `--status` CLI output.

**Existing Solutions Evaluated:**
N/A — internal monitoring feature. No external tool addresses per-segment heartbeat writes in a subprocess orchestrator. Pattern inspired by distributed job queue health checks (Sidekiq's heartbeat, Celery worker heartbeat), but implemented as a simple asyncio task.

**Alternatives Considered:**
- *Parse stream output in real-time during drain*: More complex than reading the tail of the file, requires splitting the `_drain_stdout` into a producer/consumer. Rejected in favor of the simpler tail-read approach.
- *Use file modification time instead of DB write*: Fragile (depends on filesystem mtime accuracy) and not queryable via the state API. Rejected.

**Pre-Mortem — What Could Go Wrong:**
- Reading `.stream.jsonl` from a parallel task while `_drain_stdout` writes to it is safe (read-only from heartbeat, append-only from drain). File may have a partial last line — read only complete lines (split on `\n`, discard last fragment).
- If the heartbeat task itself throws (file not yet created, JSON parse error), it must not crash `run_segment()` — wrap in `try/except`.
- `last_activity` column stores a short text snippet — cap at 500 chars to keep DB rows small.

**Risk Factor:** 4/10

**Evidence for Optimality:**
- *Codebase*: `monitor.py` already has `_extract_text_from_stream_line()` which does exactly the parsing needed — the heartbeat task can reuse it.
- *External*: Distributed job systems (Sidekiq, Celery, Resque) universally implement heartbeat writes for liveness detection. 30-minute stall threshold matches the segment timeout (3600s) at 50% — a reasonable "something is wrong" signal.

**Blast Radius:**
- Direct: `runner.py` (heartbeat task), `state.py` (schema columns + new methods), `__main__.py` (stall notification config)
- Ripple: `config.py` (new `stall_threshold` config field), `monitor.py` (expose `last_seen_at` in dashboard)

---

## Dependency Diagram

```
S1 (aiosqlite + schema)
  └── S2 (outbox worker) ── depends on async StateDB + notifications table
        └── S3 (heartbeat) ── depends on async StateDB + heartbeat columns
```

S1 must land first. S2 and S3 both depend on S1 but are independent of each other — they could run in parallel if desired, but S2 adds the `notifications` table and S3 adds columns/methods; safest to run sequentially to avoid schema conflicts.

---

## Segment 1: aiosqlite migration + notifications schema

> **Execution method:** Run directly (no iterative-builder subagent needed — this is a mechanical API migration in `scripts/orchestrate_v2/`).

**Goal:** Migrate `StateDB` from `sqlite3` to `aiosqlite`, make all state methods `async`, and add the `notifications` outbox table + `last_seen_at`/`last_activity` columns to the schema.

**Depends on:** None (pre-step copy must be done first)

**Issues addressed:** Issue 2 (async SQLite), prerequisite for Issues 1 and 3

**Cycle budget:** 20

**Scope:**
- `scripts/orchestrate_v2/state.py` — full rewrite to aiosqlite
- `scripts/orchestrate_v2/__main__.py` — all `state.*` calls become `await`
- `scripts/orchestrate_v2/runner.py` — all `state.*` calls become `await`
- `scripts/orchestrate_v2/monitor.py` — `state.all_as_dict()` becomes `await`
- `scripts/orchestrate_v2/notify.py` — add `enqueue()` method that inserts into outbox table (implemented here so schema is in one place)

**Key files and context:**

Current `StateDB.__init__` (state.py:64-74):
```python
self._conn = sqlite3.connect(str(db_path), isolation_level="DEFERRED", check_same_thread=False)
self._conn.execute("PRAGMA journal_mode=WAL")
self._conn.execute("PRAGMA busy_timeout=5000")
self._conn.executescript(_SCHEMA)
self._conn.commit()
```
This becomes:
```python
# In __init__: store path only. Actual connection created in async classmethod.
@classmethod
async def create(cls, db_path: Path) -> "StateDB":
    conn = await aiosqlite.connect(str(db_path))
    await conn.execute("PRAGMA journal_mode=WAL")
    await conn.execute("PRAGMA busy_timeout=5000")
    await conn.execute("PRAGMA synchronous=NORMAL")
    await conn.executescript(_SCHEMA)
    await conn.commit()
    obj = cls.__new__(cls)
    obj._conn = conn
    obj._path = db_path
    return obj
```

New schema additions to `_SCHEMA`:
```sql
-- Notification outbox (for at-least-once delivery)
CREATE TABLE IF NOT EXISTS notifications (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at  REAL NOT NULL,
    event_key   TEXT NOT NULL UNIQUE,  -- dedup: hash of kind+message
    kind        TEXT NOT NULL,
    message     TEXT NOT NULL,
    sent_at     REAL,
    attempts    INTEGER NOT NULL DEFAULT 0,
    last_error  TEXT
);

-- Additional segment columns for heartbeat tracking
ALTER TABLE segments ADD COLUMN last_seen_at REAL;
ALTER TABLE segments ADD COLUMN last_activity TEXT;
```

Note: `ALTER TABLE ADD COLUMN` in SQLite is idempotent-safe if wrapped in try/except (SQLite raises `OperationalError: duplicate column name` if column already exists). Use:
```python
for sql in [
    "ALTER TABLE segments ADD COLUMN last_seen_at REAL",
    "ALTER TABLE segments ADD COLUMN last_activity TEXT",
    # notifications table is CREATE IF NOT EXISTS, no ALTER needed
]:
    try:
        await conn.execute(sql)
    except aiosqlite.OperationalError:
        pass  # column already exists
```

All `StateDB` method signatures change from `def` to `async def`. All `with self._conn:` blocks change to `async with self._conn:`. All `self._conn.execute(...)` calls become `await self._conn.execute(...)`.

In `__main__.py`, `state = StateDB(db_path)` becomes `state = await StateDB.create(db_path)`. All `state.*` calls in `_orchestrate_inner`, `_run_wave`, `_run_one`, `_heartbeat_loop`, `cmd_status`, etc. get `await`.

In `runner.py`, the three `state.*` calls in `run_segment()` get `await`.

In `monitor.py`, `_handle_state` becomes `async def` (it already is) and calls `await state.all_as_dict()`.

**Implementation approach:**
1. Install `aiosqlite` (no pyproject.toml exists — add a `requirements.txt` to `scripts/orchestrate_v2/` if one doesn't exist, or just confirm it's pip-installable).
2. Rewrite `state.py` method by method, maintaining the exact same public API (same method names, same parameter types, same return types) — just adding `async` and `await`.
3. Update `close()` to `async def close()` and `await self._conn.close()`.
4. Update all call sites (grepping for `state.` in `__main__.py`, `runner.py`, `monitor.py`).
5. The `migrate_from_json` method reads a file + does DB writes — it must become async too.

**Alternatives ruled out:**
- `run_in_executor` per method: rejected (more boilerplate, less clean API, aiosqlite is purpose-built)
- Keep sync StateDB, add explicit asyncio.Lock around all calls: rejected (still blocks event loop)

**Pre-mortem risks:**
- Missing an `await` somewhere compiles fine (returns a coroutine object instead of the value) but causes subtle bugs. After migration, run `python -m py_compile` on all files and do a grep for `state\.\(set_status\|log_event\|get_segment\|increment_attempts\|all_as_dict\|progress\|init_segments\|migrate_from_json\|reset_stale_running\|set_meta\|get_meta\|close\)` that is NOT preceded by `await`.
- `aiosqlite.connect()` returns an async context manager OR a connection object (depending on usage). Use `aiosqlite.connect(path)` as a regular awaitable connection (not `async with`) to match the long-lived connection pattern.

**Segment-specific commands:**
- Install: `pip install aiosqlite`
- Build (syntax check): `python -m py_compile scripts/orchestrate_v2/*.py`
- Test (targeted): `python -c "import asyncio; from scripts.orchestrate_v2.state import StateDB; asyncio.run(StateDB.create('/tmp/test_v2.db'))"`
- Test (regression): `python -m scripts.orchestrate_v2 dry-run .claude/plans/phase2-coverage-hardening`
- Full gate: `python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening`

**Exit criteria:**

1. **Targeted tests:**
   - `StateDB.create()` creates a `state.db` with both old tables AND new `notifications` table and new columns: verify with `.schema` query
   - All existing state operations (set_status, log_event, get_segment, progress, all_as_dict) work async: verify with a simple integration script
   - `migrate_from_json` correctly migrates the existing `execution-state.json`: verify counts match
2. **Regression tests:** `python -m scripts.orchestrate_v2 dry-run .claude/plans/phase2-coverage-hardening` exits 0 and shows the 6-wave plan
3. **Full build gate:** `python -m py_compile scripts/orchestrate_v2/*.py`
4. **Full test gate:** `python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening` shows correct segment statuses
5. **Self-review gate:** No `sqlite3` imports remaining in `orchestrate_v2/`. No sync `StateDB` method calls without `await`.
6. **Scope verification gate:** Only files in `scripts/orchestrate_v2/` are modified. `scripts/orchestrate/` is untouched.

**Risk factor:** 7/10

**Estimated complexity:** High

**Commit message:**
`refactor(orchestrate_v2): migrate StateDB to aiosqlite, add notifications outbox schema`

---

## Segment 2: Notification outbox worker

> **Execution method:** Run directly (no iterative-builder subagent needed).

**Goal:** Replace fire-and-forget `Notifier.send()` with a persistent outbox queue backed by the `notifications` table, and a background `_notification_worker` that retries failed sends with exponential backoff.

**Depends on:** Segment 1 (async StateDB + notifications table)

**Issues addressed:** Issue 1 (sporadic notifications)

**Cycle budget:** 20

**Scope:**
- `scripts/orchestrate_v2/notify.py` — add `enqueue()`, rewrite `send()` to use outbox, keep `_send_imessage` as the delivery mechanism
- `scripts/orchestrate_v2/__main__.py` — add `_notification_worker` task, update all `await notifier.*` calls
- `scripts/orchestrate_v2/config.py` — add `notify_max_attempts: int = 3` and `notify_retry_delays: list[int] = [10, 60, 300]`
- `scripts/orchestrate_v2/state.py` — add `enqueue_notification()`, `get_pending_notifications()`, `mark_notification_sent()`, `mark_notification_failed()` methods

**Key files and context:**

Current `Notifier.send()` (notify.py:54-57):
```python
async def send(self, message: str) -> None:
    if not self._enabled:
        return
    await _send_imessage(self._contact, message)
```
Problem: one-shot, failure silently discarded.

New design:

```python
# notify.py

import hashlib

class Notifier:
    def __init__(self, config, state: StateDB):
        self._enabled = config.notify_enabled and bool(config.notify_contact)
        self._contact = config.notify_contact
        self._state = state
        self._max_attempts = config.notify_max_attempts       # default 3
        self._retry_delays = config.notify_retry_delays       # default [10, 60, 300]

    async def enqueue(self, kind: str, message: str) -> None:
        """Write to outbox. Returns immediately even if disabled."""
        if not self._enabled:
            return
        event_key = hashlib.sha256(f"{kind}:{message}".encode()).hexdigest()[:32]
        await self._state.enqueue_notification(kind, message, event_key)

    async def _try_send_direct(self, message: str) -> bool:
        """Direct osascript call — used only by the worker."""
        return await _send_imessage(self._contact, message)
```

`StateDB` additions (state.py):
```python
async def enqueue_notification(self, kind: str, message: str, event_key: str) -> None:
    async with self._conn:
        await self._conn.execute(
            """INSERT OR IGNORE INTO notifications
               (created_at, event_key, kind, message, attempts)
               VALUES (?, ?, ?, ?, 0)""",
            (time.time(), event_key, kind, message),
        )

async def get_pending_notifications(self, max_attempts: int) -> list[dict]:
    """Return unsent notifications eligible for delivery."""
    async with self._conn.execute(
        """SELECT id, kind, message, attempts FROM notifications
           WHERE sent_at IS NULL AND attempts < ?
           ORDER BY created_at ASC LIMIT 10""",
        (max_attempts,),
    ) as cur:
        rows = await cur.fetchall()
    return [{"id": r[0], "kind": r[1], "message": r[2], "attempts": r[3]} for r in rows]

async def mark_notification_sent(self, notif_id: int) -> None:
    async with self._conn:
        await self._conn.execute(
            "UPDATE notifications SET sent_at=? WHERE id=?",
            (time.time(), notif_id),
        )

async def mark_notification_failed(self, notif_id: int, error: str) -> None:
    async with self._conn:
        await self._conn.execute(
            "UPDATE notifications SET attempts=attempts+1, last_error=? WHERE id=?",
            (error[:500], notif_id),
        )
```

`_notification_worker` in `__main__.py`:
```python
async def _notification_worker(
    notifier: Notifier,
    state: StateDB,
    stop_event: asyncio.Event,
    poll_interval: int = 10,
) -> None:
    """Poll outbox table and deliver pending notifications with retry."""
    while not stop_event.is_set():
        try:
            pending = await state.get_pending_notifications(notifier._max_attempts)
            for notif in pending:
                # Exponential backoff: check if enough time has passed since last attempt
                # (attempts=0 → send immediately; attempts=1 → wait retry_delays[0]; etc.)
                ok = await notifier._try_send_direct(notif["message"])
                if ok:
                    await state.mark_notification_sent(notif["id"])
                    log.info("Notification delivered: kind=%s id=%d", notif["kind"], notif["id"])
                else:
                    await state.mark_notification_failed(notif["id"], "osascript failed")
                    log.warning("Notification delivery failed (attempt %d): kind=%s",
                                notif["attempts"] + 1, notif["kind"])
        except Exception:
            log.exception("Notification worker error")

        try:
            await asyncio.wait_for(stop_event.wait(), timeout=poll_interval)
        except asyncio.TimeoutError:
            pass
```

All existing `await notifier.segment_complete(...)`, `await notifier.wave_start(...)`, etc. calls in `__main__.py` must be updated to call `enqueue()` instead of the direct send. The typed helper methods (`started`, `wave_start`, `segment_complete`, etc.) all become:
```python
async def segment_complete(self, num, title, status, summary):
    icon = {"pass": "✅", ...}.get(status.lower(), "❓")
    msg = f"{icon} S{num:02d} {status.upper()}: {title}\n{summary}"
    await self.enqueue("segment_complete", msg)
```

The `_notification_worker` task is created alongside `heartbeat_task` in `_orchestrate_inner` and stopped via the same `heartbeat_stop` event (rename to `_stop_event` to be more general).

**Config additions (config.py):**
```toml
[notifications]
enabled = true
contact = "+12036446182"
max_attempts = 3
retry_delays = [10, 60, 300]  # seconds between retry attempts
```
Add fields to `OrchestrateConfig`:
```python
notify_max_attempts: int = 3
notify_retry_delays: list[int] = field(default_factory=lambda: [10, 60, 300])
```

**Implementation approach:**
1. Add state methods first (get_pending_notifications, mark_notification_sent, mark_notification_failed).
2. Rewrite `Notifier` — all helper methods call `enqueue()` instead of `send()`. Keep `_try_send_direct` as the actual osascript caller.
3. Update `Notifier.__init__` to accept `state: StateDB`.
4. Add `_notification_worker` coroutine.
5. Wire it up in `_orchestrate_inner`: create task, add to cleanup.
6. Update `config.py` with new fields and TOML parsing.

**Alternatives ruled out:**
- *asyncio.Queue in-memory*: notifications lost on crash. Rejected — outbox in DB is the whole point.
- *Increase osascript timeout to 60s and retry in-place*: still blocks, still loses notifications on crash. Rejected.
- *Third-party notification service (Pushover, ntfy.sh)*: introduces external dependency, doesn't fix the osascript flakiness if iMessage is required. Rejected.

**Pre-mortem risks:**
- If `Notifier` is constructed before the event loop is running (e.g., in `cmd_status`), the `state` reference to an async `StateDB` must not be used outside an async context. `cmd_status` doesn't use `Notifier` so this is safe.
- Duplicate event_key hashes: SHA-256 first 32 hex chars has astronomically low collision probability for this workload. Acceptable.
- Worker sends at most 10 notifications per poll cycle to avoid flooding Messages.app. Large backlogs drain over multiple cycles — acceptable.

**Segment-specific commands:**
- Install: `pip install aiosqlite` (already done in S1)
- Build: `python -m py_compile scripts/orchestrate_v2/*.py`
- Test (targeted):
  ```python
  # Integration test: enqueue a notification, verify DB row, simulate worker delivery
  python3 -c "
  import asyncio
  from pathlib import Path
  from scripts.orchestrate_v2.state import StateDB
  async def test():
      db = await StateDB.create(Path('/tmp/test_notif.db'))
      await db.enqueue_notification('test', 'hello world', 'key123')
      pending = await db.get_pending_notifications(3)
      assert len(pending) == 1, f'Expected 1, got {len(pending)}'
      await db.mark_notification_sent(pending[0]['id'])
      pending2 = await db.get_pending_notifications(3)
      assert len(pending2) == 0
      print('PASS')
  asyncio.run(test())
  "
  ```
- Test (regression): `python -m scripts.orchestrate_v2 dry-run .claude/plans/phase2-coverage-hardening`
- Full gate: `python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening`

**Exit criteria:**

1. **Targeted tests:**
   - `enqueue_notification` inserts a row; calling twice with the same `event_key` inserts only once (idempotent): verify with SQL SELECT
   - `get_pending_notifications` returns only rows with `sent_at IS NULL AND attempts < max_attempts`
   - `mark_notification_sent` sets `sent_at`; subsequent `get_pending_notifications` excludes it
   - `_notification_worker` picks up the row, calls `_try_send_direct`, marks sent: verify with a mock that returns True
2. **Regression:** `dry-run` and `status` commands exit 0
3. **Full build gate:** `python -m py_compile scripts/orchestrate_v2/*.py`
4. **Full test gate:** Run `python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening`
5. **Self-review gate:** No direct `await _send_imessage(...)` calls outside the worker. All `Notifier` helper methods route through `enqueue()`.
6. **Scope verification gate:** Only `scripts/orchestrate_v2/` files modified.

**Risk factor:** 6/10

**Estimated complexity:** High

**Commit message:**
`feat(orchestrate_v2): add notification outbox with exponential-backoff retry worker`

---

## Segment 3: Segment progress heartbeats + stall detection

> **Execution method:** Run directly (no iterative-builder subagent needed).

**Goal:** Write `last_seen_at` and a log excerpt to each running segment's DB row every 60 seconds during execution; detect stalled segments (no file growth for >30min) and enqueue a notification.

**Depends on:** Segment 1 (async StateDB with new columns), Segment 2 (notification enqueue)

**Issues addressed:** Issue 3 (no mid-run state updates)

**Cycle budget:** 15

**Scope:**
- `scripts/orchestrate_v2/runner.py` — add `_segment_heartbeat_task` parallel to `_drain_stdout`
- `scripts/orchestrate_v2/state.py` — add `update_heartbeat(num, last_seen_at, last_activity)` method
- `scripts/orchestrate_v2/__main__.py` — add `stall_threshold` config use; pass `notifier` to `run_segment`
- `scripts/orchestrate_v2/config.py` — add `stall_threshold: int = 1800` (seconds)
- `scripts/orchestrate_v2/monitor.py` — expose `last_seen_at` in `all_as_dict()` (it will appear automatically once the column is populated)

**Key files and context:**

`runner.py` `run_segment()` currently does:
```python
try:
    await asyncio.wait_for(_drain_stdout(), timeout=config.segment_timeout)
except asyncio.TimeoutError:
    ...
```

New design — run heartbeat task alongside drain:
```python
async def _segment_heartbeat_task(
    seg_num: int,
    raw_log: Path,
    state: StateDB,
    notifier,          # Notifier | None
    started_at: float,
    heartbeat_interval: int = 60,
    stall_threshold: int = 1800,
) -> None:
    """Write heartbeat to DB every 60s. Send stall notification if file stops growing."""
    last_size = 0
    stall_notified = False

    while True:
        await asyncio.sleep(heartbeat_interval)

        # Read tail of stream file for activity snippet
        activity = ""
        current_size = 0
        if raw_log.exists():
            raw = raw_log.read_bytes()
            current_size = len(raw)
            # Extract last 2KB, split on newlines, take complete lines only
            tail_bytes = raw[-2048:] if len(raw) > 2048 else raw
            tail_text = tail_bytes.decode("utf-8", errors="replace")
            lines = tail_text.splitlines()
            # Skip partial last line unless file didn't change (completed line)
            # Try each line from the end to find parseable text
            for line in reversed(lines[:-1]):  # skip potential partial last line
                text = _extract_text_from_stream_line(line)
                if text and text.strip():
                    activity = text.strip()[:500]
                    break

        now = time.time()
        await state.update_heartbeat(seg_num, now, activity)

        # Stall detection: file hasn't grown in stall_threshold seconds
        elapsed_since_start = now - started_at
        if elapsed_since_start > stall_threshold and current_size == last_size:
            if not stall_notified and notifier:
                await notifier.enqueue(
                    "segment_stall",
                    f"⚠️ S{seg_num:02d} may be stalled\n"
                    f"No output growth for {stall_threshold//60}min\n"
                    f"Last activity: {activity[:200] or '(none)'}"
                )
                stall_notified = True
        else:
            stall_notified = False  # reset if file grew again

        last_size = current_size
```

Launch heartbeat alongside drain in `run_segment()`:
```python
heartbeat_task = asyncio.create_task(
    _segment_heartbeat_task(
        seg.num, raw_log, state, notifier,
        started_at=time.time(),
        heartbeat_interval=60,
        stall_threshold=config.stall_threshold,
    )
)
try:
    await asyncio.wait_for(_drain_stdout(), timeout=config.segment_timeout)
except asyncio.TimeoutError:
    ...
finally:
    heartbeat_task.cancel()
    try:
        await heartbeat_task
    except asyncio.CancelledError:
        pass
```

`StateDB` addition:
```python
async def update_heartbeat(self, num: int, last_seen_at: float, last_activity: str) -> None:
    async with self._conn:
        await self._conn.execute(
            "UPDATE segments SET last_seen_at=?, last_activity=? WHERE num=?",
            (last_seen_at, last_activity[:500], num),
        )
    await self.log_event("segment_heartbeat", f"S{num:02d} alive: {last_activity[:100]}")
```

`--status` CLI update to show elapsed and last activity:
```python
# In cmd_status, for each segment with status == "running":
if seg.get("last_seen_at"):
    age = int(time.time() - seg["last_seen_at"])
    activity = seg.get("last_activity", "")[:60]
    print(f"    └─ last seen {age}s ago: {activity}")
```

`config.py` addition:
```python
stall_threshold: int = 1800
```
Parse from `[execution]` section: `stall_threshold=execution.get("stall_threshold", 1800)`.

TOML (orchestrate.toml):
```toml
[execution]
stall_threshold = 1800   # seconds of no output before stall notification
```

**Implementation approach:**
1. Add `update_heartbeat()` to `StateDB`.
2. Add `_segment_heartbeat_task()` function to `runner.py` (import `_extract_text_from_stream_line` from `monitor.py` — or copy it to a shared `utils.py` to avoid circular import).
3. Update `run_segment()` signature to accept `notifier` parameter; update callers in `__main__.py`.
4. Add `stall_threshold` to config.
5. Update `cmd_status` to show elapsed/last_activity for running segments.

**Circular import risk:** `runner.py` importing from `monitor.py` could cause circular imports if `monitor.py` imports from `runner.py`. Audit first. If circular: extract `_extract_text_from_stream_line` to `scripts/orchestrate_v2/streamparse.py`.

**Alternatives ruled out:**
- *Parse stream in real-time during drain instead of tailing file*: requires replacing `read(256KB)` with a more complex producer/consumer with in-process queues. Higher complexity, no benefit over tailing the file. Rejected.
- *Use `inotify`/`watchdog` for file change events*: adds a dependency, overkill for 60s polling. Rejected.

**Pre-mortem risks:**
- If `raw_log` doesn't exist yet (segment just started), `raw_log.exists()` returns False and `activity` stays empty — safe.
- Heartbeat writes 1 DB row every 60s per running segment. With 4 parallel segments, that's 4 writes/minute — negligible.
- `_extract_text_from_stream_line` may raise on malformed JSON — wrap in try/except.
- Heartbeat task must be cancelled even if `_drain_stdout` raises — use `finally` block.

**Segment-specific commands:**
- Build: `python -m py_compile scripts/orchestrate_v2/*.py`
- Test (targeted):
  ```bash
  # Run a dry segment and verify last_seen_at updates in the DB
  python3 -c "
  import asyncio, time
  from pathlib import Path
  from scripts.orchestrate_v2.state import StateDB
  async def test():
      db = await StateDB.create(Path('/tmp/test_heartbeat.db'))
      # Simulate a segment row
      from scripts.orchestrate_v2.planner import Segment
      seg = Segment(num=99, slug='test', title='Test', wave=1)
      await db.init_segments([seg])
      await db.update_heartbeat(99, time.time(), 'doing stuff')
      row = await db.get_segment(99)
      assert row.last_seen_at is not None
      assert row.last_activity == 'doing stuff'
      print('PASS')
  asyncio.run(test())
  "
  ```
- Test (regression): `python -m scripts.orchestrate_v2 dry-run .claude/plans/phase2-coverage-hardening`
- Full gate: `python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening`

**Exit criteria:**

1. **Targeted tests:**
   - `update_heartbeat()` writes `last_seen_at` and `last_activity` to the segment row: verified via `get_segment()`
   - After 60s sleep in a test run, the DB row's `last_seen_at` is within 2s of `time.time()`: not practical to verify in unit test — verify via integration (status CLI shows updated time)
   - Stall notification is enqueued when `current_size == last_size` for `> stall_threshold` elapsed time: verify with a mock that doesn't write to the file
2. **Regression:** All prior commands exit 0
3. **Full build gate:** `python -m py_compile scripts/orchestrate_v2/*.py`
4. **Full test gate:** `python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening`
5. **Self-review gate:** Heartbeat task is always cancelled in `finally`. No circular imports.
6. **Scope verification gate:** Only `scripts/orchestrate_v2/` modified.

**Risk factor:** 4/10

**Estimated complexity:** Medium

**Commit message:**
`feat(orchestrate_v2): add per-segment heartbeat writes and stall detection notifications`

---

## Execution Instructions

Switch to Agent Mode. Execute segments sequentially in this order:

**Pre-step (before S1):**
```bash
cp -r scripts/orchestrate scripts/orchestrate_v2
```
(Already done: `scripts/orchestrate_backup/` exists as the untouched backup.)

Then implement each segment directly — these are clean Python migrations suitable for direct implementation, not full iterative-builder subagent launches:

1. **Segment 1** — aiosqlite migration. Run `pip install aiosqlite` first. Verify with `dry-run`.
2. **Segment 2** — Notification outbox. Builds directly on S1's async StateDB. Verify with integration test script.
3. **Segment 3** — Heartbeat + stall. Builds on S1+S2. Verify with status CLI.

After all three segments:
```bash
# Smoke test the full v2 stack:
python -m scripts.orchestrate_v2 dry-run .claude/plans/phase2-coverage-hardening
python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening

# Optional: run one segment for real to validate end-to-end
python -m scripts.orchestrate_v2 run .claude/plans/phase2-coverage-hardening --monitor 8078
```

---

## Total Estimated Scope

- **Segments:** 3
- **Overall complexity:** High (driven by aiosqlite migration touching 5 files)
- **Total risk budget:** 7 + 6 + 4 = 17/30
- **No segment at risk 8+** — no risk budget flag needed
- **Files modified in v2:** `state.py`, `notify.py`, `runner.py`, `__main__.py`, `config.py`, `monitor.py` (minor)
- **New files:** potentially `streamparse.py` if circular import occurs
- **Caveats:** The aiosqlite migration (S1) is the highest-risk change. If the existing `state.db` has schema drift from the initial run of the bash-script version, `ALTER TABLE` guards (try/except on duplicate column) must be robust.
