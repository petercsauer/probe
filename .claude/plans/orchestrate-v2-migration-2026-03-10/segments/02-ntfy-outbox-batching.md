---
segment: 2
title: "ntfy outbox + batching + verbosity"
depends_on: [1]
risk: 4/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(orchestrate_v2): replace osascript with ntfy outbox, add batching and verbosity"
---

# Segment 2: ntfy outbox + batching + verbosity

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Replace fire-and-forget osascript with a persistent ntfy.sh HTTP outbox: enqueue notifications atomically to SQLite, retry with exponential backoff, batch wave completions into single messages, and use ntfy priority/tag/click headers.

**Depends on:** Segment 1 (async StateDB + `notifications` table with `event_key`, `priority`, `last_attempt_at` columns)

## Context: Issues Addressed

**Issue 1 — Notifications silently dropped:**
`notify.py` makes a single osascript call with a 15s timeout. macOS Messages.app has a hardcoded 10s handler timeout and delays sends by up to 5 minutes during idle. Any failure silently drops the notification — no retry, no queue, no persistence.

Fix: replace with ntfy.sh HTTP (`https://ntfy.sh/{topic}`) + transactional outbox. All sends go through the outbox table; a background worker polls every 10s with exponential backoff retries (10s→60s→300s). Wave completions are batched into one message. Verbosity config controls which events generate notifications.

**ntfy.sh rate limits:** Anonymous = 17,280 messages/12 hours per IP. A 24-segment run sends ~50. No practical limit. Topic must be UUID-style (16+ hex chars) for security by obscurity.

**ntfy headers to use:**
- `Title`: short summary shown as notification title
- `Priority`: `urgent`/`high`/`default`/`low`/`min`
- `Tags`: emoji tags (e.g. `rocket`, `x`, `white_check_mark`, `warning`, `fire`)
- `Click`: URL to open — set to `http://localhost:{monitor_port}` for dashboard click-through

## Scope

- `scripts/orchestrate_v2/notify.py` — full rewrite: ntfy transport, outbox enqueue, typed helpers
- `scripts/orchestrate_v2/__main__.py` — add `_notification_worker` task; replace per-segment notifications with batched wave call
- `scripts/orchestrate_v2/config.py` — add `ntfy_topic`, `notify_verbosity`, `notify_max_attempts`, `notify_retry_delays`; remove old `notify_contact`

## Key Files and Context

**Current `notify.py` Notifier.send() (lines 54–57):**
```python
async def send(self, message: str) -> None:
    if not self._enabled:
        return
    await _send_imessage(self._contact, message)
```
Problem: one-shot, failure silently discarded.

**New `_send_ntfy` function:**
```python
import httpx

async def _send_ntfy(
    topic: str, message: str,
    title: str = "", priority: str = "default",
    tags: str = "", click_url: str = "",
) -> bool:
    headers = {"Priority": priority}
    if title: headers["Title"] = title
    if tags: headers["Tags"] = tags
    if click_url: headers["Click"] = click_url
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
```

**New `Notifier` class skeleton:**
```python
import hashlib

PRIORITY_MAP = {
    "pass": "default", "partial": "high", "blocked": "urgent",
    "failed": "urgent", "timeout": "high", "stall": "high", "error": "urgent",
}

class Notifier:
    def __init__(self, config: OrchestrateConfig, state: StateDB):
        self._enabled = config.notify_enabled and bool(config.ntfy_topic)
        self._topic = config.ntfy_topic
        self._verbosity = config.notify_verbosity  # all|failures_only|waves_only|final_only
        self._max_attempts = config.notify_max_attempts
        self._retry_delays = config.notify_retry_delays  # [10, 60, 300]
        self._click_url = f"http://localhost:{config.monitor_port}" if config.monitor_port else ""
        self._state = state
        self._config = config

    def _should_send(self, kind: str) -> bool:
        v = self._verbosity
        if v == "all": return True
        if v == "failures_only":
            return kind in ("segment_complete_fail","segment_stall","gate_fail","error","finished")
        if v == "waves_only":
            return kind in ("wave_complete","gate_result","finished","error")
        if v == "final_only":
            return kind in ("finished","error")
        return True

    async def enqueue(self, kind: str, message: str,
                      title: str = "", priority: str = "default", tags: str = "") -> None:
        if not self._enabled or not self._should_send(kind):
            return
        event_key = hashlib.sha256(f"{kind}:{message[:200]}".encode()).hexdigest()[:32]
        await self._state.enqueue_notification(kind, message, event_key, priority)
```

**Typed helper methods** (all call `enqueue()`):
```python
async def started(self, plan_title, total, waves):
    await self.enqueue("started", f"🚀 {plan_title}\n{total} segments · {waves} waves",
                       title="Orchestration started", priority="default", tags="rocket")

async def wave_complete(self, wave, total_waves, results: list[tuple[int,str]]):
    # Batched: ONE message for the whole wave
    passed = sum(1 for _,s in results if s == "pass")
    failed = [(n,s) for n,s in results if s != "pass"]
    lines = [f"Wave {wave}/{total_waves}: {passed}/{len(results)} passed"]
    for n, s in failed:
        lines.append(f"  ❌ S{n:02d} {s}")
    priority = "urgent" if failed else "default"
    await self.enqueue("wave_complete", "\n".join(lines),
                       title=f"Wave {wave} complete",
                       priority=priority, tags="x" if failed else "white_check_mark")

async def segment_complete(self, num, title, status, summary):
    # Only called for individual failures (urgent ones), not all segments
    icon = {"pass":"✅","partial":"⚠️","blocked":"🚫","failed":"❌","timeout":"⏰"}.get(status,"❓")
    kind = "segment_complete_fail" if status not in ("pass",) else "segment_complete_pass"
    await self.enqueue(kind, f"{icon} S{num:02d} {status.upper()}: {title}\n{summary[:300]}",
                       title=f"S{num:02d} {status.upper()}",
                       priority=PRIORITY_MAP.get(status, "default"))

async def gate_result(self, wave, passed, detail):
    kind = "gate_result" if passed else "gate_fail"
    await self.enqueue(kind,
                       f"{'✅' if passed else '🚨'} Gate Wave {wave}: {'PASSED' if passed else 'FAILED'}" +
                       (f"\n{detail[:300]}" if not passed else ""),
                       title=f"Gate Wave {wave}", priority="urgent" if not passed else "low")

async def stall(self, seg_num, minutes, activity):
    await self.enqueue("segment_stall",
                       f"⚠️ S{seg_num:02d} stalled ({minutes}min no output)\n{activity[:200]}",
                       title=f"S{seg_num:02d} stalled", priority="high", tags="warning")

async def network_down(self, waited_sec):
    await self.enqueue("network_down",
                       f"📡 Network unreachable for {waited_sec}s\nOrchestration paused",
                       title="Network outage", priority="high", tags="satellite")

async def finished(self, plan_title, progress):
    total = sum(progress.values())
    passed = progress.get("pass", 0)
    icon = "🎉" if passed == total else "⚠️"
    await self.enqueue("finished",
                       f"{icon} {plan_title}\n{passed}/{total} passed\n{progress}",
                       title="Orchestration complete", tags="checkered_flag")

async def error(self, message):
    await self.enqueue("error", f"🔥 {message}", title="Orchestrator error",
                       priority="urgent", tags="fire")
```

**`_notification_worker` in `__main__.py`:**
```python
async def _notification_worker(
    notifier: Notifier, state: StateDB,
    stop_event: asyncio.Event, poll_interval: int = 10,
) -> None:
    retry_delays = notifier._config.notify_retry_delays  # [10, 60, 300]
    while not stop_event.is_set():
        try:
            pending = await state.get_pending_notifications(notifier._max_attempts)
            for notif in pending:
                # Exponential backoff: skip if not enough time since last attempt
                if notif["attempts"] > 0 and notif.get("last_attempt_at"):
                    delay = retry_delays[min(notif["attempts"]-1, len(retry_delays)-1)]
                    if (time.time() - notif["last_attempt_at"]) < delay:
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

Wire into `_orchestrate_inner`: create task alongside heartbeat task, stop via same `_stop_event`.

**In `_run_wave` result handling** — replace per-segment `notifier.segment_complete()` calls with:
```python
results = await _run_wave(...)
await notifier.wave_complete(wave_num, max_wave, results)
# Also notify individually for urgent failures:
for seg_num, status in results:
    if status not in ("pass", "skipped"):
        seg = next((s for s in pending if s.num == seg_num), None)
        if seg:
            await notifier.segment_complete(seg_num, seg.title, status, "")
```

**Config changes (`config.py`):**
Remove: `notify_contact: str`
Add:
```python
ntfy_topic: str = ""
notify_verbosity: str = "all"   # all|failures_only|waves_only|final_only
notify_max_attempts: int = 3
notify_retry_delays: list[int] = field(default_factory=lambda: [10, 60, 300])
```

Parse from TOML `[notifications]`:
```python
ntfy_topic=notifications.get("ntfy_topic", ""),
notify_verbosity=notifications.get("verbosity", "all"),
notify_max_attempts=notifications.get("max_attempts", 3),
notify_retry_delays=notifications.get("retry_delays", [10, 60, 300]),
```

**`orchestrate.toml` update (in `.claude/plans/phase2-coverage-hardening/`):**
```toml
[notifications]
enabled = true
ntfy_topic = "prb-REPLACE-WITH-YOUR-UUID-TOPIC"
verbosity = "all"
max_attempts = 3
retry_delays = [10, 60, 300]
```

## Implementation Approach

1. Write the new `notify.py` with `_send_ntfy` + `Notifier` class.
2. Update `config.py` — remove `notify_contact`, add ntfy fields.
3. Update `Notifier.__init__` to accept `state: StateDB` (second arg).
4. Add `_notification_worker` to `__main__.py`.
5. Wire: create task, add to stop sequence.
6. Replace per-segment notification calls in `_run_wave` with batched wave call.
7. Update `orchestrate.toml` to add new fields (leave ntfy_topic as placeholder).

## Alternatives Ruled Out

- Keep osascript with retry: transport fundamentally unreliable. Rejected.
- asyncio.Queue in-memory: notifications lost on crash. Rejected.
- apprise library: unnecessary dependency for single target. Rejected.

## Pre-Mortem Risks

- `httpx` must be installed (in requirements.txt from S1).
- `Notifier` now takes `state` as second arg — update all `Notifier(config)` call sites to `Notifier(config, state)`.
- `last_attempt_at` column: `mark_notification_failed()` must also write `last_attempt_at=time.time()`.
- Wave batching: `_run_wave` must return `list[tuple[int,str]]` (seg_num, status). Check current return type in `__main__.py`.

## Build and Test Commands

- **Build**: `python -m py_compile scripts/orchestrate_v2/*.py`
- **Test (targeted)**:
  ```bash
  # Dedup test
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
  # Real ntfy send test (replace TOPIC with your actual topic):
  python3 -c "
  import asyncio
  from scripts.orchestrate_v2.notify import _send_ntfy
  async def t():
      ok = await _send_ntfy('prb-TEST-REPLACE', 'Test from orchestrate_v2 ✅', title='S2 test', priority='default')
      print('PASS' if ok else 'FAIL - check topic or network')
  asyncio.run(t())
  "
  ```
- **Test (regression)**: `python -m scripts.orchestrate_v2 dry-run .claude/plans/phase2-coverage-hardening`
- **Test (full gate)**: `python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening`

## Exit Criteria

1. **Targeted tests:**
   - Outbox dedup: calling `enqueue_notification` twice with same `event_key` inserts only one row.
   - `get_pending_notifications` excludes rows where `sent_at IS NOT NULL`.
   - `mark_notification_sent` sets `sent_at`; subsequent call to `get_pending` excludes it.
   - Real ntfy message received on subscribed device.
2. **Regression tests:** `dry-run` and `status` commands exit 0.
3. **Full build gate:** `python -m py_compile scripts/orchestrate_v2/*.py`
4. **Full test gate:** `status .claude/plans/phase2-coverage-hardening` shows correct statuses.
5. **Self-review gate:** Zero osascript references in `orchestrate_v2/`. All `Notifier` helpers route through `enqueue()`. No direct `_send_ntfy` calls outside `_notification_worker`.
6. **Scope verification gate:** Only `scripts/orchestrate_v2/` modified.
