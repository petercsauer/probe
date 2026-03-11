# Orchestrate v2 — Reliability Hardening

**Goal:** Replace osascript with ntfy.sh HTTP notifications (persistent outbox, at-least-once delivery, batching, verbosity control), migrate SQLite to aiosqlite (async-safe), add mid-run heartbeats + stall detection + network outage handling, ship a mobile-first dashboard with operator controls (skip/retry/kill), ETA, log search, and keyboard navigation.
**Generated:** 2026-03-10
**Entry point:** A (Fresh Goal)
**Status:** Ready for execution

---

## Execution Log

| Segment | Est. Complexity | Risk | Cycles Used | Status | Notes |
|---------|----------------|------|-------------|--------|-------|
| 1: aiosqlite + full schema | High | 7/10 | -- | -- | -- |
| 2: ntfy outbox + batching | Medium | 4/10 | -- | -- | -- |
| 3: Heartbeats + network + timeout override | Medium | 4/10 | -- | -- | -- |
| 4: Mobile dashboard + UX | Medium | 3/10 | -- | -- | -- |
| 5: Operator control API + buttons | Medium | 5/10 | -- | -- | -- |

**Deep-verify result:** --
**Follow-up plans:** --

---

## Pre-Step: Backup + Working Copy

Both already done:
```bash
scripts/orchestrate_backup/   # untouched original
scripts/orchestrate_v2/       # working copy — all segments modify this
```

All segments modify only `scripts/orchestrate_v2/`. The original `scripts/orchestrate/` is never touched.

**ntfy setup (one-time, before first run):**
1. Pick a UUID-style topic: `prb-$(python3 -c "import uuid; print(uuid.uuid4().hex[:16])")`  e.g. `prb-a3f8c12b9e4d7051`
2. Install the [ntfy iOS app](https://apps.apple.com/app/ntfy/id1625396347) or [Android app](https://play.google.com/store/apps/details?id=io.heckel.ntfy)
3. Subscribe to your topic in the app
4. Set `ntfy_topic = "prb-a3f8c12b9e4d7051"` in `.claude/plans/phase2-coverage-hardening/orchestrate.toml`

**Rate limits:** ntfy.sh anonymous = 17,280 messages/12 hours per IP. A full 24-segment run sends ~50 notifications. No practical limit.

---

## Issue Analysis Briefs

### Issue 1: Notifications silently dropped — replace osascript with ntfy.sh

**Core Problem:**
`notify.py`'s `_send_imessage()` makes a single osascript call with a 15-second timeout. macOS `Messages.app` has a hardcoded 10-second AppleScript handler timeout and is documented to delay sends by up to 5 minutes during idle. Any transient failure silently drops the notification forever — no retry, no queue, no persistence.

**Root Cause:**
Two compounding failures: (1) osascript → Messages.app is inherently unreliable on macOS; (2) fire-and-forget with zero retry. Either alone causes sporadic misses; both together guarantee it.

**Proposed Fix:**
Replace osascript with ntfy.sh HTTP transport + transactional outbox + batching:
1. `_send_ntfy(topic, message, title, priority, tags, click_url)` — plain `httpx.AsyncClient.post()` to `https://ntfy.sh/{topic}` with ntfy headers.
2. `notifications` table in `state.db` for persistent outbox (INSERT OR IGNORE dedup on `event_key`).
3. `_notification_worker` asyncio task: polls every 10s, retries with exponential backoff (10s→60s→300s).
4. Notification batching: wave completions send one message summarising all segment results rather than N individual messages.
5. Verbosity config: `all` | `failures_only` | `waves_only` | `final_only`.
6. ntfy priority levels: `urgent` for failures/blocked, `high` for wave failures, `default` for progress, `min` for heartbeats.
7. Click-through URL: ntfy `Click` header set to `http://localhost:{monitor_port}` so tapping a notification opens the dashboard.

**Existing Solutions Evaluated:**
- `apprise` (PyPI): routes to 100+ services including ntfy. Considered — adds flexibility but unnecessary dependency for single-target use. Rejected.
- `python-telegram-bot`: requires account + bot token. Rejected (no key desired).
- Hand-rolled ntfy + httpx outbox: **adopted**.

**Risk Factor:** 4/10

**Blast Radius:** `notify.py` (full rewrite), `state.py` (outbox methods), `__main__.py` (worker task), `config.py` (ntfy fields)

---

### Issue 2: sqlite3 sync calls blocking the asyncio event loop

**Core Problem:**
All `StateDB` methods are synchronous `sqlite3` calls on the event loop thread. When 4 segments finish simultaneously, `state.set_status()` / `state.log_event()` block the loop, stalling the SSE stream, heartbeat tasks, and the notification worker.

**Root Cause:**
`check_same_thread=False` disables the thread check but doesn't make I/O non-blocking. No `await` anywhere in `StateDB`.

**Proposed Fix:**
Migrate to `aiosqlite`: every method becomes `async def`, every query becomes `await conn.execute(...)`. aiosqlite runs sqlite3 on a dedicated background thread with an internal queue — all I/O is off the event loop thread.

**Existing Solutions Evaluated:**
- `aiosqlite` (omnilib, MIT, 2.5k ★, 2024 CPU/locking improvements): **adopted**.
- `run_in_executor(ThreadPoolExecutor(max_workers=1))`: works but requires manual wrapping of every call. Rejected.

**Risk Factor:** 7/10

**Blast Radius:** `state.py` (full rewrite), `__main__.py`, `runner.py`, `monitor.py` (all state calls add `await`)

---

### Issue 3: No mid-run state updates

**Core Problem:**
A segment's DB row is written twice — start and end. A crash mid-run leaves segments permanently `running` with no progress info. Also: no per-segment timeout override (all segments share the same 3600s global timeout), and the Python v2 lost the bash version's network outage detection.

**Root Cause:**
No watchdog task. No timeout field in segment frontmatter. No network health check before launching new segments.

**Proposed Fix:**
1. **Heartbeat task** per running segment: wakes every 60s, reads tail of `.stream.jsonl`, writes `last_seen_at` + `last_activity` to DB, logs `segment_heartbeat` event. If file size unchanged for >stall_threshold seconds, enqueues stall notification.
2. **Per-segment timeout override**: read `timeout` from segment frontmatter (falls back to `config.segment_timeout`).
3. **Network outage detection**: before launching each segment, poll `https://api.anthropic.com` with 5s timeout. If unreachable, wait with exponential backoff (10s→60s, max 10min), send ntfy notification once per outage.
4. **Attempt history**: store each attempt's result/summary/tokens in `segment_attempts` table. Show in `--status` CLI and dashboard.

**Risk Factor:** 4/10

**Blast Radius:** `runner.py` (heartbeat task, network check), `state.py` (new columns + attempts table), `planner.py` (read timeout frontmatter), `config.py` (stall_threshold, network_retry_max)

---

### Issue 4: Dashboard not mobile-usable or feature-complete

**Core Problem:**
The dashboard has a hardcoded `380px 1fr` two-column grid — unusable on mobile. No way to search logs, filter segments by status, see ETA, or navigate without a mouse. No elapsed time displayed. No keyboard shortcuts. State is lost on page refresh.

**Root Cause:**
Built for desktop only; no responsive breakpoints; no interactive features beyond click-to-select.

**Proposed Fix:**
1. **Mobile-first layout**: single column on `<640px` with tab bar (Timeline | Log | Events). Two-column grid on desktop unchanged.
2. **ETA**: `avg_completed_duration × pending_count ÷ max_parallel` shown in header.
3. **Elapsed time per segment**: show on each row for running segments.
4. **Log search**: text input filters log lines in real-time (highlight matches).
5. **Log color coding**: ERROR/BLOCKED = red, PASS/✅ = green, tool calls (`→ Bash:`) = dim monospace, WARN/⚠️ = amber.
6. **Keyboard shortcuts**: `j`/`k` navigate segments, `Enter` opens log, `s` skip, `r` retry, `/` focus search, `Escape` clear.
7. **localStorage persistence**: selected segment + scroll position survive refresh.
8. **Filter by status**: dropdown to show All / Running / Failed / Blocked / Pending.
9. **Event severity color coding**: error events in red, warn in amber, info in default.
10. **Auto-select first running segment** when nothing is selected.
11. **Notification log tab**: show `notifications` table (sent_at, attempts, message) so you can verify deliveries.

**Risk Factor:** 3/10

**Blast Radius:** `dashboard.html` (full rewrite), `monitor.py` (expose notifications in `/api/state`, add severity to events response)

---

### Issue 5: No operator control — can't intervene during a run

**Core Problem:**
If a segment is blocked/failed and you want to skip it, or a segment crashed and you want to retry it immediately without restarting the whole orchestrator, there's no mechanism. You can only kill the whole process or wait for it to exhaust `max_retries`.

**Root Cause:**
No control API. Dashboard is read-only. CLI has no `--skip` or `--retry` flags.

**Proposed Fix:**
1. `/api/control` POST endpoint in `monitor.py`: `{"action": "skip"|"retry"|"kill", "seg_num": N}`.
   - `skip`: set status to `skipped`, add event, cancel running process if any.
   - `retry`: reset status to `pending`, clear `attempts`, re-queue in current wave.
   - `kill`: SIGTERM the segment's process group (tracked via `_running_pids` dict in orchestrator).
2. Dashboard: add Skip / Retry / Kill buttons per segment row (shown contextually — Retry on failed/blocked, Skip on pending/blocked, Kill on running).
3. CLI: `--skip S11` and `--retry S11` flags that write to the state DB without launching the orchestrator (useful for pre-flight corrections).
4. Running PIDs registry: `__main__.py` maintains `_running_pids: dict[int, int]` mapping seg_num → process PID, updated by runner, read by control endpoint.

**Risk Factor:** 5/10

**Blast Radius:** `monitor.py` (control endpoint), `__main__.py` (PID registry, retry/skip logic), `runner.py` (expose PID), `state.py` (skipped status), `dashboard.html` (action buttons), `__main__.py` (CLI flags)

---

## Dependency Diagram

```
S1 (aiosqlite + full schema)
├── S2 (ntfy outbox) — needs async StateDB + notifications table
├── S3 (heartbeats + network + timeout) — needs async StateDB + heartbeat columns + attempts table
└── S4 (dashboard) — needs severity in events, notifications in API (S2 first)
      └── S5 (operator control) — needs dashboard + PID registry + async state mutations
```

S1 must land first. S2 and S3 can run in parallel (different schema additions, no conflict). S4 depends on S2 (needs notifications table in API). S5 depends on S4.

**Parallelizable:** S2 ∥ S3 after S1 lands.

---

## Segment 1: aiosqlite migration + full schema

**Goal:** Migrate `StateDB` from `sqlite3` to `aiosqlite`, make all state methods `async`, and add all new schema: `notifications` outbox table, `segment_attempts` history table, `severity` column on `events`, `last_seen_at`/`last_activity` columns on `segments`, `per_segment_timeout` on `segments`.

**Depends on:** None

**Issues addressed:** Issue 2 (async SQLite), schema foundation for all other segments

**Cycle budget:** 20

**Scope:**
- `scripts/orchestrate_v2/state.py` — full rewrite to aiosqlite, all new schema
- `scripts/orchestrate_v2/__main__.py` — all `state.*` calls add `await`
- `scripts/orchestrate_v2/runner.py` — all `state.*` calls add `await`
- `scripts/orchestrate_v2/monitor.py` — `state.all_as_dict()` adds `await`
- `scripts/orchestrate_v2/requirements.txt` — add `aiosqlite`, `httpx`

**Key files and context:**

Current `StateDB.__init__` opens a sync `sqlite3` connection. Replace with async factory:

```python
import aiosqlite

class StateDB:
    def __init__(self):
        raise RuntimeError("Use await StateDB.create(path)")

    @classmethod
    async def create(cls, db_path: Path) -> "StateDB":
        conn = await aiosqlite.connect(str(db_path))
        conn.row_factory = aiosqlite.Row
        await conn.execute("PRAGMA journal_mode=WAL")
        await conn.execute("PRAGMA busy_timeout=5000")
        await conn.execute("PRAGMA synchronous=NORMAL")
        await conn.executescript(_SCHEMA)
        # Idempotent column additions (ALTER TABLE fails silently if exists)
        for sql in _MIGRATIONS:
            try:
                await conn.execute(sql)
            except Exception:
                pass
        await conn.commit()
        obj = object.__new__(cls)
        obj._conn = conn
        obj._path = db_path
        return obj

    async def close(self) -> None:
        await self._conn.close()
```

Full new schema (`_SCHEMA` additions):

```sql
-- Notification outbox
CREATE TABLE IF NOT EXISTS notifications (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at  REAL NOT NULL,
    event_key   TEXT NOT NULL UNIQUE,
    kind        TEXT NOT NULL,
    message     TEXT NOT NULL,
    priority    TEXT NOT NULL DEFAULT 'default',
    sent_at     REAL,
    attempts    INTEGER NOT NULL DEFAULT 0,
    last_error  TEXT
);

-- Per-attempt history
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

Migrations (idempotent via try/except):
```sql
ALTER TABLE segments ADD COLUMN last_seen_at REAL;
ALTER TABLE segments ADD COLUMN last_activity TEXT;
ALTER TABLE segments ADD COLUMN per_segment_timeout INTEGER;
ALTER TABLE events ADD COLUMN severity TEXT DEFAULT 'info';
```

All `StateDB` methods change from `def` → `async def`, `with self._conn:` → `async with self._conn:`, `self._conn.execute(...)` → `await self._conn.execute(...)`, `cur.fetchall()` → `await cur.fetchall()`.

New methods to add in this segment:
```python
async def record_attempt(self, seg_num: int, attempt: int, started_at: float,
                          finished_at: float, status: str, summary: str,
                          tokens_in: int = 0, tokens_out: int = 0) -> None: ...

async def get_attempts(self, seg_num: int) -> list[dict]: ...

async def log_event(self, kind: str, detail: str = "", severity: str = "info") -> None: ...
    # Add severity param, default "info". Callers can pass severity="warn" or "error".

async def update_heartbeat(self, num: int, last_seen_at: float, last_activity: str) -> None: ...

async def enqueue_notification(self, kind: str, message: str, event_key: str,
                                priority: str = "default") -> None: ...
async def get_pending_notifications(self, max_attempts: int) -> list[dict]: ...
async def mark_notification_sent(self, notif_id: int) -> None: ...
async def mark_notification_failed(self, notif_id: int, error: str) -> None: ...
async def get_recent_notifications(self, limit: int = 20) -> list[dict]: ...
```

Update `all_as_dict()` to include notifications and attempt history:
```python
async def all_as_dict(self) -> dict:
    segments = [dict(s) for s in await self.get_all_segments()]
    # Attach attempt history to each segment
    for seg in segments:
        seg["attempts_history"] = await self.get_attempts(seg["num"])
    return {
        ...,
        "segments": segments,
        "events": await self.get_events(limit=50),
        "notifications": await self.get_recent_notifications(limit=20),
    }
```

**Implementation approach:**
1. Write `requirements.txt` with `aiosqlite>=0.20`, `httpx>=0.27`, `aiohttp>=3.9`.
2. Rewrite `state.py` method-by-method. Keep exact same public API, just add `async`/`await`.
3. Update `close()` to `async`.
4. Update all callers: grep `state\.` in `__main__.py`, `runner.py`, `monitor.py` — every call site gets `await`.
5. In `__main__.py`, `state = StateDB(db_path)` → `state = await StateDB.create(db_path)`.
6. `cmd_status` is sync — wrap its state reads with `asyncio.run(state_reads_coroutine())` or make it async internally.

**Alternatives ruled out:**
- `run_in_executor`: manual wrapping, same thread-safety concerns. Rejected.
- Keep sync + `asyncio.Lock`: still blocks event loop. Rejected.

**Pre-mortem risks:**
- Missing `await` compiles silently (returns coroutine object). After migration: `python -m py_compile scripts/orchestrate_v2/*.py` + grep for un-awaited state calls.
- `aiosqlite.connect()` should be used as `await aiosqlite.connect(path)` (not `async with`) for a long-lived connection.
- `cmd_status` and `cmd_dry_run` are sync entry points — they call `asyncio.run()` already. Need to ensure `StateDB.create()` is called inside `asyncio.run()`.

**Segment-specific commands:**
- Install: `pip install aiosqlite httpx aiohttp`
- Build: `python -m py_compile scripts/orchestrate_v2/*.py`
- Test (targeted):
  ```bash
  python3 -c "
  import asyncio
  from pathlib import Path
  from scripts.orchestrate_v2.state import StateDB
  async def t():
      db = await StateDB.create(Path('/tmp/test_s1.db'))
      # Verify new tables exist
      async with db._conn.execute(\"SELECT name FROM sqlite_master WHERE type='table'\") as c:
          tables = {r[0] for r in await c.fetchall()}
      assert 'notifications' in tables, tables
      assert 'segment_attempts' in tables, tables
      # Verify new columns
      async with db._conn.execute('PRAGMA table_info(segments)') as c:
          cols = {r[1] for r in await c.fetchall()}
      assert 'last_seen_at' in cols, cols
      assert 'last_activity' in cols, cols
      print('PASS')
      await db.close()
  asyncio.run(t())
  "
  ```
- Test (regression): `python -m scripts.orchestrate_v2 dry-run .claude/plans/phase2-coverage-hardening`
- Full gate: `python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening`

**Exit criteria:**
1. **Targeted tests:** All new tables and columns present in fresh DB; all existing state ops work async; `migrate_from_json` migrates correctly.
2. **Regression:** `dry-run` and `status` commands exit 0.
3. **Full build gate:** `python -m py_compile scripts/orchestrate_v2/*.py` — zero errors.
4. **Full test gate:** `status` shows correct segment statuses from existing `state.db`.
5. **Self-review gate:** Zero `sqlite3` imports in `orchestrate_v2/`. Zero un-awaited `state.*` calls.
6. **Scope gate:** Only `scripts/orchestrate_v2/` modified.

**Risk factor:** 7/10
**Estimated complexity:** High
**Commit message:** `refactor(orchestrate_v2): migrate StateDB to aiosqlite, add full schema`

---

## Segment 2: ntfy outbox worker + batching + verbosity

**Goal:** Replace fire-and-forget osascript with a persistent ntfy.sh HTTP outbox: enqueue notifications atomically to SQLite, retry with exponential backoff, batch wave completions into single messages, support verbosity levels, and use ntfy priority/tag/click headers.

**Depends on:** Segment 1 (async StateDB + notifications table)

**Issues addressed:** Issue 1 (sporadic notifications)

**Cycle budget:** 15

**Scope:**
- `scripts/orchestrate_v2/notify.py` — full rewrite: ntfy transport, outbox enqueue, helper methods
- `scripts/orchestrate_v2/__main__.py` — add `_notification_worker` task, batched wave notifications
- `scripts/orchestrate_v2/config.py` — add ntfy_topic, notify_verbosity, notify_max_attempts, notify_retry_delays, monitor_port (for click URL)

**Key files and context:**

New `notify.py` design:

```python
import hashlib, httpx
from .state import StateDB
from .config import OrchestrateConfig

PRIORITY_MAP = {
    "pass":    "default",
    "partial": "high",
    "blocked": "urgent",
    "failed":  "urgent",
    "timeout": "high",
    "stall":   "high",
    "error":   "urgent",
}

async def _send_ntfy(
    topic: str,
    message: str,
    title: str = "",
    priority: str = "default",
    tags: str = "",
    click_url: str = "",
) -> bool:
    """POST to ntfy.sh. Returns True on HTTP 200."""
    headers = {"Priority": priority}
    if title:
        headers["Title"] = title
    if tags:
        headers["Tags"] = tags
    if click_url:
        headers["Click"] = click_url
    try:
        async with httpx.AsyncClient(timeout=10) as client:
            r = await client.post(
                f"https://ntfy.sh/{topic}",
                data=message.encode(),
                headers=headers,
            )
            return r.status_code == 200
    except Exception:
        return False


class Notifier:
    def __init__(self, config: OrchestrateConfig, state: StateDB):
        self._enabled = config.notify_enabled and bool(config.ntfy_topic)
        self._topic = config.ntfy_topic
        self._verbosity = config.notify_verbosity     # all|failures_only|waves_only|final_only
        self._max_attempts = config.notify_max_attempts
        self._click_url = f"http://localhost:{config.monitor_port}" if config.monitor_port else ""
        self._state = state

    def _should_send(self, kind: str) -> bool:
        v = self._verbosity
        if v == "all":
            return True
        if v == "failures_only":
            return kind in ("segment_complete_fail", "segment_stall", "gate_fail", "error", "finished")
        if v == "waves_only":
            return kind in ("wave_complete", "gate_result", "finished", "error")
        if v == "final_only":
            return kind in ("finished", "error")
        return True

    async def enqueue(self, kind: str, message: str,
                      title: str = "", priority: str = "default", tags: str = "") -> None:
        if not self._enabled or not self._should_send(kind):
            return
        event_key = hashlib.sha256(f"{kind}:{message[:200]}".encode()).hexdigest()[:32]
        await self._state.enqueue_notification(kind, message, event_key, priority)

    # Typed helpers — all call enqueue():
    async def started(self, plan_title: str, total: int, waves: int) -> None:
        await self.enqueue("started", f"🚀 {plan_title}\n{total} segments · {waves} waves",
                           title="Orchestration started", priority="default", tags="rocket")

    async def segment_complete(self, num: int, title: str, status: str, summary: str) -> None:
        icon = {"pass":"✅","partial":"⚠️","blocked":"🚫","failed":"❌","timeout":"⏰"}.get(status,"❓")
        kind = f"segment_complete_{'fail' if status not in ('pass',) else 'pass'}"
        priority = PRIORITY_MAP.get(status, "default")
        await self.enqueue(kind, f"{icon} S{num:02d} {status.upper()}: {title}\n{summary[:300]}",
                           title=f"S{num:02d} {status.upper()}", priority=priority)

    async def wave_complete(self, wave: int, total_waves: int, results: list[tuple[int,str]]) -> None:
        # Batched: one message summarising ALL segment results in the wave
        passed = sum(1 for _,s in results if s == "pass")
        failed = [(n,s) for n,s in results if s != "pass"]
        lines = [f"Wave {wave}/{total_waves}: {passed}/{len(results)} passed"]
        for n, s in failed:
            lines.append(f"  ❌ S{n:02d} {s}")
        priority = "urgent" if failed else "default"
        tags = "x" if failed else "white_check_mark"
        await self.enqueue("wave_complete", "\n".join(lines),
                           title=f"Wave {wave} complete", priority=priority, tags=tags)

    async def gate_result(self, wave: int, passed: bool, detail: str) -> None:
        kind = "gate_result" if passed else "gate_fail"
        priority = "urgent" if not passed else "low"
        msg = f"{'✅' if passed else '🚨'} Gate Wave {wave}: {'PASSED' if passed else 'FAILED'}"
        if not passed:
            msg += f"\n{detail[:300]}"
        await self.enqueue(kind, msg, title=f"Gate Wave {wave}", priority=priority)

    async def stall(self, seg_num: int, minutes: int, activity: str) -> None:
        await self.enqueue("segment_stall",
                           f"⚠️ S{seg_num:02d} stalled ({minutes}min no output)\n{activity[:200]}",
                           title=f"S{seg_num:02d} stalled", priority="high", tags="warning")

    async def network_down(self, waited_sec: int) -> None:
        await self.enqueue("network_down",
                           f"📡 Network unreachable for {waited_sec}s\nOrchestration paused",
                           title="Network outage", priority="high", tags="satellite")

    async def finished(self, plan_title: str, progress: dict) -> None:
        total = sum(progress.values())
        passed = progress.get("pass", 0)
        icon = "🎉" if passed == total else "⚠️"
        await self.enqueue("finished",
                           f"{icon} {plan_title}\n{passed}/{total} passed\n{progress}",
                           title="Orchestration complete", priority="default", tags="checkered_flag")

    async def error(self, message: str) -> None:
        await self.enqueue("error", f"🔥 {message}", title="Orchestrator error",
                           priority="urgent", tags="fire")
```

`_notification_worker` in `__main__.py`:

```python
async def _notification_worker(
    notifier: "Notifier",
    state: StateDB,
    stop_event: asyncio.Event,
    poll_interval: int = 10,
) -> None:
    retry_delays = notifier._config.notify_retry_delays  # [10, 60, 300]
    while not stop_event.is_set():
        try:
            pending = await state.get_pending_notifications(notifier._max_attempts)
            for notif in pending:
                # Skip if not enough time since last attempt
                if notif["attempts"] > 0:
                    delay = retry_delays[min(notif["attempts"]-1, len(retry_delays)-1)]
                    if notif.get("last_attempt_at") and (time.time() - notif["last_attempt_at"]) < delay:
                        continue
                ok = await _send_ntfy(
                    notifier._topic, notif["message"],
                    priority=notif.get("priority", "default"),
                    click_url=notifier._click_url,
                )
                if ok:
                    await state.mark_notification_sent(notif["id"])
                else:
                    await state.mark_notification_failed(notif["id"], "HTTP error")
        except Exception:
            log.exception("Notification worker error")
        try:
            await asyncio.wait_for(stop_event.wait(), timeout=poll_interval)
        except asyncio.TimeoutError:
            pass
```

Note: `notifications` table needs a `last_attempt_at REAL` column — add to S1 schema.

In `__main__.py`, replace all `await notifier.segment_complete(...)` per-segment calls with a **batched wave call** after `_run_wave` returns:
```python
# After _run_wave returns results:
await notifier.wave_complete(wave_num, max_wave, results)
# Still send individual failure notifications for urgent ones:
for seg_num, status in results:
    if status not in ("pass", "skipped"):
        seg = next(s for s in pending if s.num == seg_num)
        await notifier.segment_complete(seg_num, seg.title, status, "")
```

**Config additions (`orchestrate.toml`):**
```toml
[notifications]
enabled = true
ntfy_topic = "prb-a3f8c12b9e4d7051"   # your UUID-style topic
verbosity = "all"                       # all | failures_only | waves_only | final_only
max_attempts = 3
retry_delays = [10, 60, 300]
```

`OrchestrateConfig` additions:
```python
ntfy_topic: str = ""
notify_verbosity: str = "all"
notify_max_attempts: int = 3
notify_retry_delays: list[int] = field(default_factory=lambda: [10, 60, 300])
```

Remove old `notify_contact` field (was the phone number for iMessage).

**Alternatives ruled out:**
- Keep osascript with retry: transport fundamentally unreliable. Rejected.
- apprise library: unnecessary dependency for single target. Rejected.
- In-memory asyncio.Queue: notifications lost on crash. Rejected.

**Pre-mortem risks:**
- `httpx` not installed → import error. `requirements.txt` must list it; S1 installs it.
- ntfy.sh topic guessable if too short → use UUID hex (16+ chars).
- `last_attempt_at` column needed in S1 schema — add to S1 migration list.
- Wave batching: if `_run_wave` partially fails (some `asyncio.gather` exceptions), results list may be incomplete. Handle with `return_exceptions=True` already in place.

**Segment-specific commands:**
- Build: `python -m py_compile scripts/orchestrate_v2/*.py`
- Test (targeted):
  ```bash
  python3 -c "
  import asyncio
  from pathlib import Path
  from scripts.orchestrate_v2.state import StateDB
  async def t():
      db = await StateDB.create(Path('/tmp/test_s2.db'))
      await db.enqueue_notification('test','hello','key1','default')
      await db.enqueue_notification('test','hello','key1','default')  # dedup
      pending = await db.get_pending_notifications(3)
      assert len(pending) == 1, pending
      await db.mark_notification_sent(pending[0]['id'])
      assert len(await db.get_pending_notifications(3)) == 0
      print('PASS')
      await db.close()
  asyncio.run(t())
  "
  # Also: send a real test notification
  python3 -c "
  import asyncio
  from scripts.orchestrate_v2.notify import _send_ntfy
  async def t():
      ok = await _send_ntfy('prb-TEST-TOPIC', 'Test from orchestrate_v2', title='Test', priority='default')
      print('PASS' if ok else 'FAIL - check topic name or network')
  asyncio.run(t())
  "
  ```
- Test (regression): `python -m scripts.orchestrate_v2 dry-run .claude/plans/phase2-coverage-hardening`
- Full gate: `python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening`

**Exit criteria:**
1. **Targeted:** Outbox dedup works; `get_pending_notifications` excludes sent rows; real ntfy test message arrives on phone.
2. **Regression:** `dry-run` and `status` exit 0.
3. **Build gate:** `python -m py_compile scripts/orchestrate_v2/*.py`
4. **Self-review gate:** Zero osascript references in `orchestrate_v2/`. All `Notifier` helpers route through `enqueue()`.
5. **Scope gate:** Only `scripts/orchestrate_v2/` modified.

**Risk factor:** 4/10
**Estimated complexity:** Medium
**Commit message:** `feat(orchestrate_v2): replace osascript with ntfy outbox, add batching and verbosity`

---

## Segment 3: Heartbeats + network detection + per-segment timeout

**Goal:** Write `last_seen_at`/`last_activity` every 60s per running segment; detect stalls and enqueue notifications; check network before launching segments; read per-segment `timeout` from frontmatter; record attempt history on completion.

**Depends on:** Segment 1 (async StateDB + new columns), Segment 2 (notifier.stall/network_down)

**Issues addressed:** Issue 3 (no mid-run state updates)

**Cycle budget:** 15

**Scope:**
- `scripts/orchestrate_v2/runner.py` — heartbeat task, network check, timeout override, record_attempt on completion
- `scripts/orchestrate_v2/state.py` — `update_heartbeat()`, `record_attempt()` (already declared in S1)
- `scripts/orchestrate_v2/planner.py` — read `timeout` from frontmatter
- `scripts/orchestrate_v2/__main__.py` — network pre-check, pass notifier to run_segment, stall_threshold config
- `scripts/orchestrate_v2/config.py` — `stall_threshold`, `network_retry_max`

**Key files and context:**

**Heartbeat task** (add to `runner.py`):

```python
from .monitor import _extract_text_from_stream_line  # or move to streamparse.py if circular

async def _segment_heartbeat_task(
    seg_num: int,
    raw_log: Path,
    state: StateDB,
    notifier,
    started_at: float,
    heartbeat_interval: int = 60,
    stall_threshold: int = 1800,
) -> None:
    last_size = 0
    stall_notified = False
    while True:
        await asyncio.sleep(heartbeat_interval)
        activity, current_size = "", 0
        try:
            if raw_log.exists():
                raw = raw_log.read_bytes()
                current_size = len(raw)
                tail = raw[-2048:].decode("utf-8", errors="replace")
                for line in reversed(tail.splitlines()[:-1]):
                    text = _extract_text_from_stream_line(line)
                    if text and text.strip():
                        activity = text.strip()[:500]
                        break
        except Exception:
            pass
        await state.update_heartbeat(seg_num, time.time(), activity)
        elapsed = time.time() - started_at
        if elapsed > stall_threshold and current_size == last_size:
            if not stall_notified and notifier:
                await notifier.stall(seg_num, stall_threshold // 60, activity)
                stall_notified = True
        else:
            stall_notified = False
        last_size = current_size
```

Launch alongside drain and cancel in `finally`:
```python
started_at = time.time()
heartbeat = asyncio.create_task(
    _segment_heartbeat_task(seg.num, raw_log, state, notifier,
                             started_at, stall_threshold=config.stall_threshold)
)
segment_timeout = seg.per_segment_timeout or config.segment_timeout
try:
    await asyncio.wait_for(_drain_stdout(), timeout=segment_timeout)
except asyncio.TimeoutError:
    ...
finally:
    heartbeat.cancel()
    try: await heartbeat
    except asyncio.CancelledError: pass
```

**Record attempt on completion** (add to `run_segment()` after status is determined):
```python
# Parse token usage from stream JSONL result event
tokens_in, tokens_out = _extract_token_usage(raw_log)
await state.record_attempt(
    seg.num, attempts_count, started_at, time.time(),
    status, summary, tokens_in, tokens_out
)
```

```python
def _extract_token_usage(raw_path: Path) -> tuple[int, int]:
    """Parse input/output token counts from stream-json result event."""
    try:
        with open(raw_path, errors="replace") as f:
            for line in f:
                obj = json.loads(line.strip() or "{}")
                if obj.get("type") == "result":
                    usage = obj.get("usage", {})
                    return usage.get("input_tokens", 0), usage.get("output_tokens", 0)
    except Exception:
        pass
    return 0, 0
```

**Per-segment timeout override** in `planner.py`:
```python
# In _parse_frontmatter / Segment dataclass:
timeout: int = 0   # 0 means use config default

# In load_plan:
segments.append(Segment(
    ...
    timeout=sfm.get("timeout", 0),
    ...
))
```

Segment frontmatter example:
```yaml
---
segment: 11
title: "Cross-Crate Integration Tests"
timeout: 7200   # 2 hours instead of default 1 hour
---
```

**Network outage detection** in `__main__.py` (called before each wave launch):
```python
async def _wait_for_network(
    notifier,
    max_wait: int = 600,  # config.network_retry_max
) -> None:
    waited, notified, delay = 0, False, 10
    while True:
        try:
            async with httpx.AsyncClient(timeout=5) as c:
                await c.get("https://api.anthropic.com")
            return  # reachable
        except Exception:
            pass
        waited += delay
        if waited >= max_wait:
            log.warning("Network unreachable for %ds, proceeding anyway", max_wait)
            return
        if not notified and waited >= 60:
            await notifier.network_down(waited)
            notified = True
        log.warning("Network down, retry in %ds (%d/%d)", delay, waited, max_wait)
        await asyncio.sleep(delay)
        delay = min(delay * 2, 60)
```

Call before each wave: `await _wait_for_network(notifier, config.network_retry_max)`.

**Config additions:**
```toml
[execution]
stall_threshold = 1800      # seconds
network_retry_max = 600     # seconds
```

**Circular import check:** `runner.py` imports `_extract_text_from_stream_line` from `monitor.py`. `monitor.py` does NOT import from `runner.py`. No circular dependency. If discovered during implementation, extract to `scripts/orchestrate_v2/streamparse.py`.

**Segment-specific commands:**
- Build: `python -m py_compile scripts/orchestrate_v2/*.py`
- Test (targeted):
  ```bash
  python3 -c "
  import asyncio, time
  from pathlib import Path
  from scripts.orchestrate_v2.state import StateDB
  from scripts.orchestrate_v2.planner import Segment
  async def t():
      db = await StateDB.create(Path('/tmp/test_s3.db'))
      seg = Segment(num=1, slug='test', title='Test', wave=1)
      await db.init_segments([seg])
      await db.update_heartbeat(1, time.time(), 'running cycle 3')
      row = await db.get_segment(1)
      assert row['last_activity'] == 'running cycle 3', row
      await db.record_attempt(1, 1, time.time()-60, time.time(), 'pass', 'all done', 100, 200)
      attempts = await db.get_attempts(1)
      assert len(attempts) == 1, attempts
      print('PASS')
      await db.close()
  asyncio.run(t())
  "
  ```
- Test (regression): `python -m scripts.orchestrate_v2 dry-run .claude/plans/phase2-coverage-hardening`
- Full gate: `python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening`

**Exit criteria:**
1. **Targeted:** `update_heartbeat` writes to DB; `record_attempt` stores attempt row; token extraction returns (0,0) on missing file gracefully; `Segment.timeout` parsed from frontmatter.
2. **Regression:** All prior commands exit 0.
3. **Build gate:** `python -m py_compile scripts/orchestrate_v2/*.py`
4. **Self-review gate:** Heartbeat always cancelled in `finally`. No circular imports.
5. **Scope gate:** Only `scripts/orchestrate_v2/` modified.

**Risk factor:** 4/10
**Estimated complexity:** Medium
**Commit message:** `feat(orchestrate_v2): add heartbeats, stall detection, network check, per-segment timeout`

---

## Segment 4: Mobile-first dashboard + UX features

**Goal:** Rewrite `dashboard.html` with mobile-first responsive layout (tab navigation on mobile), ETA, elapsed time per segment, log search + color coding, keyboard shortcuts, status filter, localStorage persistence, event severity colors, notification log tab, and auto-select of first running segment.

**Depends on:** Segment 2 (notifications in `/api/state`), Segment 3 (`last_seen_at` in state)

**Issues addressed:** Issue 4 (dashboard not mobile-usable)

**Cycle budget:** 15

**Scope:**
- `scripts/orchestrate_v2/dashboard.html` — full rewrite
- `scripts/orchestrate_v2/monitor.py` — expose `notifications` and `last_seen_at` in `/api/state` response (already done via `all_as_dict()` in S1), add `severity` to events SSE

**Key files and context:**

**Layout strategy:**
- Mobile (`<640px`): single column, sticky tab bar at bottom with 3 tabs: Timeline | Log | Events. Only the active tab's panel is visible (`display:none` on inactive).
- Desktop (`≥640px`): existing two-column grid (`380px 1fr`) + events bar at bottom. Unchanged visually.

```css
/* Mobile tab bar */
.tab-bar {
  display: none;
  position: fixed; bottom: 0; left: 0; right: 0;
  background: var(--surface); border-top: 1px solid var(--border);
  display: flex; height: 52px; z-index: 100;
}
.tab-btn {
  flex: 1; background: none; border: none; color: var(--text-dim);
  font-size: 11px; font-weight: 600; display: flex; flex-direction: column;
  align-items: center; justify-content: center; gap: 2px; cursor: pointer;
}
.tab-btn.active { color: var(--running); }

@media (max-width: 640px) {
  .layout { grid-template-columns: 1fr; grid-template-rows: 1fr; height: calc(100vh - 100px); }
  .log-panel, .events { display: none; }
  .log-panel.tab-active, .events.tab-active { display: flex; }
  .timeline.tab-active { display: block; }
  .tab-bar { display: flex; }
  /* Larger touch targets on mobile */
  .seg-row { padding: 10px 16px; min-height: 48px; }
}
@media (min-width: 641px) {
  .tab-bar { display: none !important; }
}
```

**ETA calculation** (in `refreshState`):
```javascript
// After computing done/running counts:
const completedSegs = data.segments.filter(s => s.finished_at && s.started_at);
if (completedSegs.length > 0) {
  const avgDuration = completedSegs.reduce((s,seg) => s + (seg.finished_at - seg.started_at), 0) / completedSegs.length;
  const pendingCount = data.segments.filter(s => s.status === 'pending').length + running;
  const etaSec = avgDuration * pendingCount / Math.max(config.max_parallel || 4, 1);
  document.getElementById('eta').textContent = etaSec > 0 ? `ETA: ~${formatElapsed(etaSec)}` : '';
}
```

**Elapsed time per segment row:**
```javascript
// In renderTimeline, for running segments:
if (s.status === 'running' && s.started_at) {
  const elapsed = Date.now()/1000 - s.started_at;
  elapsedHtml = `<span class="seg-elapsed">${formatElapsed(elapsed)}</span>`;
}
```

**Log search:**
```html
<div class="log-toolbar">
  <input id="log-search" type="text" placeholder="/ search..." autocomplete="off">
  <span id="search-count"></span>
</div>
```
```javascript
document.getElementById('log-search').addEventListener('input', function() {
  const term = this.value.toLowerCase();
  const lines = document.querySelectorAll('.log-line');
  let matches = 0;
  lines.forEach(l => {
    const show = !term || l.textContent.toLowerCase().includes(term);
    l.style.display = show ? '' : 'none';
    if (show && term) { l.classList.add('highlight'); matches++; }
    else l.classList.remove('highlight');
  });
  document.getElementById('search-count').textContent = term ? `${matches} matches` : '';
});
```

Log lines are rendered as `<div class="log-line">` elements rather than raw `textContent`, enabling per-line styling.

**Log color coding** (applied when adding each line to the log panel):
```javascript
function classifyLogLine(text) {
  if (/\b(error|ERROR|BLOCKED|failed|FAILED)\b/.test(text)) return 'log-error';
  if (/\b(warn|WARN|⚠️|stall)\b/i.test(text)) return 'log-warn';
  if (/\b(PASS|pass|✅|success)\b/.test(text)) return 'log-pass';
  if (/^→ /.test(text)) return 'log-tool';
  if (/^  ← /.test(text)) return 'log-result';
  return '';
}
```

**Keyboard shortcuts:**
```javascript
document.addEventListener('keydown', function(e) {
  if (e.target.tagName === 'INPUT') return;
  if (e.key === 'j') selectNext();
  if (e.key === 'k') selectPrev();
  if (e.key === 'Enter' && activeSeg) openLog(activeSeg);
  if (e.key === '/') { e.preventDefault(); document.getElementById('log-search').focus(); }
  if (e.key === 'Escape') { document.getElementById('log-search').value = ''; filterLog(''); }
  if (e.key === 'f') cycleFilter();  // All → Running → Failed → Blocked → All
});
```

**Status filter:**
```html
<select id="status-filter" onchange="applyFilter()">
  <option value="all">All</option>
  <option value="running">Running</option>
  <option value="failed">Failed/Blocked</option>
  <option value="pending">Pending</option>
  <option value="pass">Passed</option>
</select>
```

**localStorage persistence:**
```javascript
// On load:
activeSeg = parseInt(localStorage.getItem('activeSeg')) || null;
if (activeSeg) openLog(activeSeg);

// On segment select:
localStorage.setItem('activeSeg', num);
```

**Auto-select running segment** (in `refreshState` when `activeSeg === null`):
```javascript
if (!activeSeg) {
  const running = data.segments.find(s => s.status === 'running');
  if (running) window._selectSeg(running.num, running.title, 'running');
}
```

**Notification log tab** (fourth tab on desktop: add to events panel as a toggle):
```javascript
// In all_as_dict response, notifications array is now included.
// Render in a "Notifications" section below events feed.
function renderNotifications(notifs) {
  notifs.forEach(n => {
    const icon = n.sent_at ? '✅' : (n.attempts >= 3 ? '❌' : '⏳');
    // render row: icon, kind, message preview, attempts, sent_at
  });
}
```

**Event severity colors** (in event feed rendering):
```javascript
const severityClass = { 'error': 'ev-error', 'warn': 'ev-warn', 'info': '' }[ev.severity] || '';
div.className = `event-line ${severityClass}`;
```

**Segment-specific commands:**
- Build: open `scripts/orchestrate_v2/dashboard.html` in a browser with `python3 -m http.server` and verify layout at 375px width (iPhone) and 1440px (desktop).
- Alternatively: `python -m scripts.orchestrate_v2 run .claude/plans/phase2-coverage-hardening --monitor 8078` and open on phone.
- Test (regression): `python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening`

**Exit criteria:**
1. **Targeted tests:**
   - At 375px viewport: tab bar visible, Timeline/Log/Events switch correctly, segment rows have ≥44px height.
   - At 1440px: two-column layout unchanged, tab bar hidden.
   - Log search filters lines in real-time.
   - `j`/`k` navigate segments; `/` focuses search input.
   - Refresh with a segment selected → same segment still selected (localStorage).
   - Running segments show elapsed time. Header shows ETA when ≥2 segments complete.
2. **Regression:** All prior commands exit 0.
3. **Build gate:** `python -m py_compile scripts/orchestrate_v2/*.py`
4. **Self-review gate:** No inline `onclick` that calls functions not defined in the script block. All JS in one `<script>` block. No external dependencies (no CDN imports).
5. **Scope gate:** Only `scripts/orchestrate_v2/` modified.

**Risk factor:** 3/10
**Estimated complexity:** Medium
**Commit message:** `feat(orchestrate_v2): mobile-first dashboard with search, keyboard nav, ETA, and notifications log`

---

## Segment 5: Operator control API + dashboard buttons + CLI flags

**Goal:** Add a `/api/control` POST endpoint for skip/retry/kill actions; add contextual action buttons to each segment row in the dashboard; add `--skip` and `--retry` CLI subcommands; maintain a running PIDs registry so kill works.

**Depends on:** Segments 1–4

**Issues addressed:** Issue 5 (no operator control)

**Cycle budget:** 15

**Scope:**
- `scripts/orchestrate_v2/monitor.py` — add `/api/control` POST endpoint
- `scripts/orchestrate_v2/__main__.py` — PID registry, retry/skip logic, expose registry to monitor, add CLI subcommands
- `scripts/orchestrate_v2/runner.py` — expose process PID after launch, register with orchestrator
- `scripts/orchestrate_v2/state.py` — add `skipped` status handling, `reset_for_retry()` method
- `scripts/orchestrate_v2/dashboard.html` — action buttons per segment row

**Key files and context:**

**PID registry** in `__main__.py`:
```python
# Module-level dict, populated by runner, read by control endpoint
_running_pids: dict[int, int] = {}  # seg_num → process PID
```

`run_segment()` in `runner.py` registers the PID immediately after `create_subprocess_exec`:
```python
proc = await asyncio.create_subprocess_exec(...)
# Register PID with orchestrator (via callback or shared dict passed in)
if register_pid:
    register_pid(seg.num, proc.pid)
```

Pass `register_pid` as a callable into `run_segment()`:
```python
# In _run_one:
await run_segment(seg, config, state, log_dir,
                  notifier=notifier,
                  register_pid=lambda n, pid: _running_pids.__setitem__(n, pid),
                  unregister_pid=lambda n: _running_pids.pop(n, None))
```

**`/api/control` endpoint** in `monitor.py`:
```python
app.router.add_post("/api/control", _handle_control)

async def _handle_control(request: web.Request) -> web.Response:
    state: StateDB = request.app["state"]
    running_pids: dict = request.app["running_pids"]
    data = await request.json()
    action = data.get("action")
    seg_num = int(data.get("seg_num", 0))

    if action == "skip":
        await state.set_status(seg_num, "skipped")
        await state.log_event("operator_skip", f"S{seg_num:02d} skipped by operator", severity="warn")
        # Kill running process if any
        pid = running_pids.get(seg_num)
        if pid:
            try: os.killpg(os.getpgid(pid), signal.SIGTERM)
            except Exception: pass
        return web.json_response({"ok": True, "action": "skip", "seg_num": seg_num})

    elif action == "retry":
        await state.reset_for_retry(seg_num)
        await state.log_event("operator_retry", f"S{seg_num:02d} queued for retry by operator", severity="warn")
        # Note: segment won't actually run until the orchestrator's wave loop picks it up.
        # If the wave is already done, it won't re-run automatically in the current session.
        # The operator should restart the orchestrator to pick it up, or we handle it in a future segment.
        return web.json_response({"ok": True, "action": "retry", "seg_num": seg_num})

    elif action == "kill":
        pid = running_pids.get(seg_num)
        if pid:
            try:
                os.killpg(os.getpgid(pid), signal.SIGTERM)
                await state.log_event("operator_kill", f"S{seg_num:02d} killed by operator", severity="warn")
                return web.json_response({"ok": True, "action": "kill", "seg_num": seg_num})
            except Exception as e:
                return web.json_response({"ok": False, "error": str(e)}, status=500)
        return web.json_response({"ok": False, "error": "not running"}, status=404)

    return web.json_response({"ok": False, "error": "unknown action"}, status=400)
```

Wire `running_pids` into the app:
```python
app["running_pids"] = running_pids  # reference to _running_pids dict from __main__
```

**`reset_for_retry()`** in `state.py`:
```python
async def reset_for_retry(self, num: int) -> None:
    async with self._conn:
        await self._conn.execute(
            "UPDATE segments SET status='pending', attempts=0, started_at=NULL, finished_at=NULL WHERE num=?",
            (num,)
        )
```

**Dashboard action buttons** — added to each segment row contextually:

```javascript
function actionButtons(seg) {
  const btns = [];
  if (seg.status === 'running') {
    btns.push(`<button class="act-btn kill" onclick="controlSeg(${seg.num},'kill')" title="Kill">✕</button>`);
  }
  if (['failed','blocked','partial','timeout'].includes(seg.status)) {
    btns.push(`<button class="act-btn retry" onclick="controlSeg(${seg.num},'retry')" title="Retry">↺</button>`);
    btns.push(`<button class="act-btn skip" onclick="controlSeg(${seg.num},'skip')" title="Skip">⏭</button>`);
  }
  if (seg.status === 'pending') {
    btns.push(`<button class="act-btn skip" onclick="controlSeg(${seg.num},'skip')" title="Skip">⏭</button>`);
  }
  return btns.join('');
}

window.controlSeg = async function(num, action) {
  if (!confirm(`${action} S${String(num).padStart(2,'0')}?`)) return;
  const r = await fetch('/api/control', {
    method: 'POST',
    headers: {'Content-Type':'application/json'},
    body: JSON.stringify({action, seg_num: num}),
  });
  const data = await r.json();
  if (!data.ok) alert(`Failed: ${data.error}`);
  else refreshState();
};
```

**CLI subcommands** in `__main__.py`:

```python
# In main():
skip_p = sub.add_parser("skip", help="Mark a segment as skipped")
skip_p.add_argument("seg_num", type=int)
skip_p.add_argument("plan_dir", type=Path)

retry_p = sub.add_parser("retry", help="Reset a segment for retry")
retry_p.add_argument("seg_num", type=int)
retry_p.add_argument("plan_dir", type=Path)

# Handlers:
elif args.command == "skip":
    async def do_skip():
        db = await StateDB.create(args.plan_dir / "state.db")
        await db.set_status(args.seg_num, "skipped")
        await db.log_event("operator_skip", f"S{args.seg_num:02d} skipped via CLI", severity="warn")
        await db.close()
        print(f"S{args.seg_num:02d} marked as skipped")
    asyncio.run(do_skip())

elif args.command == "retry":
    async def do_retry():
        db = await StateDB.create(args.plan_dir / "state.db")
        await db.reset_for_retry(args.seg_num)
        await db.log_event("operator_retry", f"S{args.seg_num:02d} reset for retry via CLI", severity="warn")
        await db.close()
        print(f"S{args.seg_num:02d} reset to pending (restart orchestrator to run)")
    asyncio.run(do_retry())
```

Usage:
```bash
python -m scripts.orchestrate_v2 skip 11 .claude/plans/phase2-coverage-hardening
python -m scripts.orchestrate_v2 retry 11 .claude/plans/phase2-coverage-hardening
```

**Attempt history in `--status` CLI** — show collapsible attempt list:
```python
# In cmd_status, after segment status line:
attempts = await db.get_attempts(seg["num"])
for a in attempts:
    dur = f"{int(a['finished_at'] - a['started_at'])}s" if a['finished_at'] else "--"
    tok = f"{a['tokens_in']+a['tokens_out']:,} tok" if a['tokens_in'] else ""
    print(f"      attempt {a['attempt']}: {a['status']} ({dur}) {tok}")
```

**Pre-mortem risks:**
- Kill sends SIGTERM to process group. If the claude subprocess spawns its own children, they're in the same group (due to `start_new_session=True` in runner.py — actually this creates a new session, not just a new group). Check: `start_new_session=True` sets the process as a new session leader, so `os.getpgid(pid)` returns `pid` itself. `os.killpg(pid, SIGTERM)` kills the whole group. This should work correctly.
- Retry via API resets the DB row but the wave may already be past that segment (wave loop doesn't re-run completed waves). Document this limitation: CLI retry is useful between runs; API retry is useful if the wave is still running. Add a note in the confirmation dialog.
- `running_pids` is a plain dict shared between the aiohttp request handler and the asyncio orchestrator loop — both run in the same thread (asyncio is single-threaded), so no locking needed.

**Segment-specific commands:**
- Build: `python -m py_compile scripts/orchestrate_v2/*.py`
- Test (targeted):
  ```bash
  # CLI skip test
  python -m scripts.orchestrate_v2 skip 99 .claude/plans/phase2-coverage-hardening 2>&1 | head -5
  python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening | grep S99

  # API test (requires running orchestrator with --monitor)
  curl -s -X POST http://localhost:8078/api/control \
    -H 'Content-Type: application/json' \
    -d '{"action":"skip","seg_num":99}' | python3 -m json.tool
  ```
- Test (regression): `python -m scripts.orchestrate_v2 dry-run .claude/plans/phase2-coverage-hardening`
- Full gate: `python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening`

**Exit criteria:**
1. **Targeted tests:**
   - `skip` CLI sets segment to `skipped` status in DB; `status` CLI shows it.
   - `retry` CLI resets segment to `pending` with `attempts=0`.
   - `/api/control` with action=skip returns `{"ok": true}`.
   - Dashboard shows Skip/Retry buttons on failed/blocked rows; Kill button on running rows.
2. **Regression:** All prior commands exit 0.
3. **Build gate:** `python -m py_compile scripts/orchestrate_v2/*.py`
4. **Self-review gate:** Kill uses SIGTERM (not SIGKILL). Confirmation dialog before all destructive actions. `reset_for_retry` does NOT clear `segment_attempts` history — attempt records are preserved.
5. **Scope gate:** Only `scripts/orchestrate_v2/` modified.

**Risk factor:** 5/10
**Estimated complexity:** Medium
**Commit message:** `feat(orchestrate_v2): operator control API with skip/retry/kill, dashboard buttons, CLI flags`

---

## Updated Dependency Diagram

```
S1 (aiosqlite + full schema)
├── S2 (ntfy outbox) ────────────────────────────────┐
├── S3 (heartbeats + network + timeout override) ──┐  │
│                                                   │  │
└──────────────────────────────────────────────────S4 (dashboard)
                                                       │
                                                       S5 (operator control)
```

S2 ∥ S3 (can run in parallel after S1). S4 after S2. S5 last.

---

## Execution Instructions

All work goes into `scripts/orchestrate_v2/`. Pre-steps already done.

Execute in this order (S2 and S3 can be parallelized if desired):

```bash
# 1. Install deps first
pip install aiosqlite httpx aiohttp

# 2. Segment 1 — aiosqlite migration
# Verify:
python -m py_compile scripts/orchestrate_v2/*.py
python -m scripts.orchestrate_v2 dry-run .claude/plans/phase2-coverage-hardening

# 3a. Segment 2 — ntfy outbox (can run alongside S3)
# Test: send a real ntfy message to your phone

# 3b. Segment 3 — heartbeats + network + timeout
# Test: verify last_seen_at updates in DB after 60s

# 4. Segment 4 — dashboard
# Test: open at 375px on phone, verify tabs work

# 5. Segment 5 — operator control
# Test: skip a segment via CLI, verify dashboard shows it

# Final smoke test:
python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening
python -m scripts.orchestrate_v2 run .claude/plans/phase2-coverage-hardening --monitor 8078
```

---

## Total Estimated Scope

- **Segments:** 5
- **Overall complexity:** High (S1 aiosqlite migration is highest risk; rest are additive)
- **Total risk budget:** 7 + 4 + 4 + 3 + 5 = 23/50
- **No segment at risk 8+** — no risk budget flag
- **Files modified:** `state.py`, `notify.py`, `runner.py`, `__main__.py`, `config.py`, `monitor.py`, `dashboard.html`, `planner.py`
- **New files:** `requirements.txt`, possibly `streamparse.py` (if circular import)
- **New capabilities vs v1:** ntfy push (no osascript), persistent outbox, mid-run heartbeats, stall detection, network resilience, per-segment timeouts, attempt history, token tracking, mobile dashboard, log search + color coding, keyboard nav, operator skip/retry/kill
