---
segment: 5
title: "Operator control API + dashboard buttons + CLI flags"
depends_on: [1, 2, 3, 4]
risk: 5/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(orchestrate_v2): operator control API with skip/retry/kill, dashboard buttons, CLI flags"
---

# Segment 5: Operator control API + dashboard buttons + CLI flags

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Add a `/api/control` POST endpoint for skip/retry/kill actions; add contextual action buttons per segment row in the dashboard; add `--skip` and `--retry` CLI subcommands; maintain a running PIDs registry so kill works.

**Depends on:** Segments 1–4 (async StateDB, notifier, PID callbacks from S3, dashboard from S4)

## Context: Issues Addressed

**Issue 5 — No operator control:**
If a segment is blocked/failed and you want to skip it to unblock the wave, or a segment crashed and you want to retry it, there's no mechanism. You must kill the entire orchestrator or wait for `max_retries` to exhaust.

Fix:
- `/api/control` REST endpoint: skip/retry/kill per segment
- Running PIDs registry: `_running_pids: dict[int, int]` (seg_num → OS PID) in `__main__.py`, populated by `run_segment()`, read by control endpoint
- Dashboard action buttons: Skip/Retry/Kill per row (contextual by status)
- CLI: `python -m scripts.orchestrate_v2 skip N <plan_dir>` and `retry N <plan_dir>`
- `reset_for_retry()` in StateDB: resets status/attempts without deleting attempt history

**Kill mechanism:** `run_segment()` uses `start_new_session=True` — the subprocess is its own session leader, so `os.getpgid(pid) == pid`. `os.killpg(pid, signal.SIGTERM)` kills the whole process group. The existing `_kill_tree()` function in `runner.py` already does this — the PID registry just makes the target addressable from outside the coroutine.

**Retry semantics:** Retry via API/CLI resets the DB row to `pending` with `attempts=0`. If the wave has already passed, the segment won't re-run automatically in the current session — the operator must restart the orchestrator. This limitation is documented in the confirmation dialog.

## Scope

- `scripts/orchestrate_v2/monitor.py` — add `/api/control` POST endpoint; wire `running_pids` into app
- `scripts/orchestrate_v2/__main__.py` — `_running_pids` dict, skip/retry logic, `skip`/`retry` CLI subcommands, PID registry wiring
- `scripts/orchestrate_v2/runner.py` — accept `register_pid`/`unregister_pid` callbacks (already added in S3 — verify and implement if not done)
- `scripts/orchestrate_v2/state.py` — `reset_for_retry()` method (already declared in S1 — implement if not done); handle `skipped` status in `_run_wave`
- `scripts/orchestrate_v2/dashboard.html` — action buttons per segment row (Skip/Retry/Kill)

## Key Files and Context

**`_running_pids` registry in `__main__.py`:**
```python
# Module-level dict (thread-safe since asyncio is single-threaded)
_running_pids: dict[int, int] = {}  # seg_num → OS PID
```

Pass into aiohttp app and into `_run_one`:
```python
app["running_pids"] = _running_pids

# In _run_one (inside _run_wave):
await run_segment(
    seg, config, state, log_dir,
    notifier=notifier,
    register_pid=lambda n, pid: _running_pids.__setitem__(n, pid),
    unregister_pid=lambda n: _running_pids.pop(n, None),
)
```

**`/api/control` endpoint in `monitor.py`:**
```python
import os, signal as signal_mod

app.router.add_post("/api/control", _handle_control)

async def _handle_control(request: web.Request) -> web.Response:
    state: StateDB = request.app["state"]
    pids: dict = request.app["running_pids"]
    try:
        data = await request.json()
    except Exception:
        return web.json_response({"ok": False, "error": "invalid JSON"}, status=400)

    action = data.get("action")
    try:
        seg_num = int(data.get("seg_num", 0))
    except (TypeError, ValueError):
        return web.json_response({"ok": False, "error": "invalid seg_num"}, status=400)

    if action == "skip":
        pid = pids.get(seg_num)
        if pid:
            try:
                os.killpg(os.getpgid(pid), signal_mod.SIGTERM)
            except Exception:
                pass
        await state.set_status(seg_num, "skipped")
        await state.log_event("operator_skip", f"S{seg_num:02d} skipped by operator", severity="warn")
        return web.json_response({"ok": True, "action": "skip", "seg_num": seg_num})

    elif action == "retry":
        await state.reset_for_retry(seg_num)
        await state.log_event("operator_retry",
                              f"S{seg_num:02d} reset for retry (restart orchestrator to run)",
                              severity="warn")
        return web.json_response({"ok": True, "action": "retry", "seg_num": seg_num})

    elif action == "kill":
        pid = pids.get(seg_num)
        if not pid:
            return web.json_response({"ok": False, "error": "not running"}, status=404)
        try:
            os.killpg(os.getpgid(pid), signal_mod.SIGTERM)
            await state.log_event("operator_kill", f"S{seg_num:02d} killed by operator", severity="warn")
            return web.json_response({"ok": True, "action": "kill", "seg_num": seg_num})
        except Exception as e:
            return web.json_response({"ok": False, "error": str(e)}, status=500)

    return web.json_response({"ok": False, "error": f"unknown action: {action}"}, status=400)
```

**`reset_for_retry()` in `state.py`:**
```python
async def reset_for_retry(self, num: int) -> None:
    async with self._conn:
        await self._conn.execute(
            """UPDATE segments
               SET status='pending', attempts=0, started_at=NULL, finished_at=NULL,
                   last_seen_at=NULL, last_activity=NULL
               WHERE num=?""",
            (num,),
        )
    # Note: segment_attempts rows are NOT deleted — history is preserved
```

**Handle `skipped` status in `_run_wave`** — skip segments already skipped (same as `pass`):
```python
# In _orchestrate_inner, when building `pending` list:
pending = [
    s for s in wave_segs
    if state_row.status not in ("pass", "skipped")
]
```

Also in `_run_one`: if segment status is `skipped` when acquired, return immediately:
```python
async def _run_one(seg: Segment) -> tuple[int, str]:
    if shutting_down.is_set():
        return seg.num, "skipped"
    async with sem:
        # Re-check status in case operator skipped it while waiting for semaphore
        current = await state.get_segment(seg.num)
        if current and current.status == "skipped":
            return seg.num, "skipped"
        ...
```

**Dashboard action buttons** (add to `renderTimeline` in `dashboard.html`):
```javascript
function actionButtons(seg) {
  const btns = [];
  if (seg.status === 'running') {
    btns.push(`<button class="act-btn act-kill" onclick="event.stopPropagation();controlSeg(${seg.num},'kill')" title="Kill process">✕</button>`);
  }
  if (['failed','blocked','partial','timeout'].includes(seg.status)) {
    btns.push(`<button class="act-btn act-retry" onclick="event.stopPropagation();controlSeg(${seg.num},'retry')" title="Retry">↺</button>`);
    btns.push(`<button class="act-btn act-skip" onclick="event.stopPropagation();controlSeg(${seg.num},'skip')" title="Skip">⏭</button>`);
  }
  if (seg.status === 'pending') {
    btns.push(`<button class="act-btn act-skip" onclick="event.stopPropagation();controlSeg(${seg.num},'skip')" title="Skip">⏭</button>`);
  }
  return `<span class="act-btns">${btns.join('')}</span>`;
}

window.controlSeg = async function(num, action) {
  const messages = {
    kill: `Kill S${String(num).padStart(2,'0')}? This terminates the running process.`,
    retry: `Reset S${String(num).padStart(2,'0')} for retry? (Restart orchestrator to run it.)`,
    skip: `Skip S${String(num).padStart(2,'0')}? The wave will proceed without it.`,
  };
  if (!confirm(messages[action] || `${action} S${String(num).padStart(2,'0')}?`)) return;
  try {
    const r = await fetch('/api/control', {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({action, seg_num: num}),
    });
    const data = await r.json();
    if (!data.ok) {
      alert(`Failed: ${data.error || 'unknown error'}`);
    } else {
      refreshState();
    }
  } catch (e) {
    alert(`Request failed: ${e.message}`);
  }
};
```

CSS for action buttons:
```css
.act-btns { display: flex; gap: 4px; flex-shrink: 0; }
.act-btn {
  border: none; border-radius: 4px; padding: 2px 6px; font-size: 11px;
  cursor: pointer; font-weight: 700; opacity: 0.7; transition: opacity 0.1s;
}
.act-btn:hover { opacity: 1; }
.act-kill  { background: var(--fail-bg); color: var(--fail); border: 1px solid var(--fail); }
.act-retry { background: var(--running-bg); color: var(--running); border: 1px solid var(--running); }
.act-skip  { background: var(--pending-bg); color: var(--text-muted); border: 1px solid var(--border); }
```

Add `${actionButtons(s)}` to the `seg-row` HTML in `renderTimeline`.

**CLI subcommands in `__main__.py`:**
```python
# In main() parser setup:
skip_p = sub.add_parser("skip", help="Mark a segment as skipped")
skip_p.add_argument("seg_num", type=int, metavar="SEG_NUM")
skip_p.add_argument("plan_dir", type=Path)

retry_p = sub.add_parser("retry", help="Reset a segment for retry")
retry_p.add_argument("seg_num", type=int, metavar="SEG_NUM")
retry_p.add_argument("plan_dir", type=Path)

# In main() dispatch:
elif args.command == "skip":
    async def _do_skip():
        db = await StateDB.create(args.plan_dir / "state.db")
        await db.set_status(args.seg_num, "skipped")
        await db.log_event("operator_skip", f"S{args.seg_num:02d} skipped via CLI", severity="warn")
        await db.close()
        print(f"S{args.seg_num:02d} marked as skipped")
    asyncio.run(_do_skip())

elif args.command == "retry":
    async def _do_retry():
        db = await StateDB.create(args.plan_dir / "state.db")
        await db.reset_for_retry(args.seg_num)
        await db.log_event("operator_retry",
                           f"S{args.seg_num:02d} reset for retry via CLI (restart orchestrator to run)",
                           severity="warn")
        await db.close()
        print(f"S{args.seg_num:02d} reset to pending — restart orchestrator to run it")
    asyncio.run(_do_retry())
```

## Implementation Approach

1. Add `_running_pids: dict[int, int] = {}` to `__main__.py`.
2. Wire PID callbacks into `_run_one` → `run_segment()` (S3 should have added the callback params — verify).
3. Add `/api/control` endpoint to `monitor.py`. Wire `app["running_pids"] = _running_pids`.
4. Implement `reset_for_retry()` in `state.py` if not done in S1.
5. Add `skipped` status handling in `_run_wave` pending filter and `_run_one` re-check.
6. Add action buttons to `dashboard.html`: `actionButtons()` function + `controlSeg()` + CSS.
7. Add `skip` and `retry` subcommands to `main()` in `__main__.py`.

## Alternatives Ruled Out

- WebSocket for control: REST POST is simpler, same capability. Rejected.
- File-based signals (write `skip-S11` file): not atomic, fragile. Rejected.
- SIGKILL instead of SIGTERM for kill: gives the process no chance to clean up. Use SIGTERM first; if process survives after grace period, it'll be handled by the existing `_kill_tree` function. Rejected (SIGKILL as first signal).

## Pre-Mortem Risks

- `os.getpgid(pid)` fails if process already exited (ProcessLookupError) — wrap in try/except. Already handled in `_kill_tree()`.
- `event.stopPropagation()` on button click prevents triggering the row's `onclick` log open — required so clicking Skip doesn't also open the log.
- `_running_pids` dict is shared between request handler and asyncio loop — safe because both run in the same asyncio thread (no GIL issue, no lock needed).
- Retry via API when wave is already complete: segment won't run. Confirmation dialog must say "restart orchestrator to run." Don't pretend it's queued.
- `skipped` must appear in `waveStatus()` JS function as a terminal state (like `pass`) so wave is considered done.

## Build and Test Commands

- **Build**: `python -m py_compile scripts/orchestrate_v2/*.py`
- **Test (targeted)**:
  ```bash
  # CLI skip test
  python -m scripts.orchestrate_v2 skip 24 .claude/plans/phase2-coverage-hardening
  python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening | grep S24

  # CLI retry test
  python -m scripts.orchestrate_v2 retry 24 .claude/plans/phase2-coverage-hardening
  python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening | grep S24

  # API test (requires running orchestrator with --monitor 8078)
  curl -s -X POST http://localhost:8078/api/control \
    -H 'Content-Type: application/json' \
    -d '{"action":"skip","seg_num":24}' | python3 -m json.tool

  # Unknown action test (should return 400)
  curl -s -o /dev/null -w "%{http_code}" -X POST http://localhost:8078/api/control \
    -H 'Content-Type: application/json' -d '{"action":"explode","seg_num":1}'
  # Expected: 400
  ```
- **Test (regression)**: `python -m scripts.orchestrate_v2 dry-run .claude/plans/phase2-coverage-hardening`
- **Test (full gate)**: `python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening`

## Exit Criteria

1. **Targeted tests:**
   - `skip 24 <plan>` CLI sets S24 to `skipped`; `status` shows it.
   - `retry 24 <plan>` CLI resets S24 to `pending` with `attempts=0`; `segment_attempts` history preserved.
   - `/api/control` skip returns `{"ok": true}`; status shows `skipped`.
   - `/api/control` unknown action returns HTTP 400.
   - Dashboard shows Skip button on pending/blocked rows; Retry on failed rows; Kill on running rows.
   - `event.stopPropagation()` prevents row log from opening when button clicked.
2. **Regression tests:** All prior commands exit 0.
3. **Full build gate:** `python -m py_compile scripts/orchestrate_v2/*.py`
4. **Self-review gate:** Kill uses SIGTERM, not SIGKILL. Confirmation dialogs before all destructive actions. `reset_for_retry` does NOT delete `segment_attempts` rows. `skipped` treated as terminal status in wave completion logic.
5. **Scope verification gate:** Only `scripts/orchestrate_v2/` modified.
