---
id: "5"
title: "No operator control — can't intervene during a run"
risk: 5/10
addressed_by_segments: [5]
---

# Issue 5: No operator control — can't intervene during a run

## Core Problem

If a segment is blocked/failed and you want to skip it to unblock the wave, or a segment crashed and you want to retry it immediately, there's no mechanism. You can only kill the whole orchestrator and restart, or wait for `max_retries` to exhaust. Celery Flower (the gold standard Python job dashboard) has had skip/retry/kill since v1.0 — it's table stakes for any operated job system.

## Root Cause

No control API endpoint. Dashboard is read-only. CLI has no `--skip` or `--retry` commands.

## Proposed Fix

**1. `/api/control` POST endpoint** in `monitor.py`:
- `{"action": "skip", "seg_num": N}` → set status to `skipped`, SIGTERM running process if any
- `{"action": "retry", "seg_num": N}` → reset to `pending`, clear `attempts`
- `{"action": "kill", "seg_num": N}` → SIGTERM the segment's process group
- Returns `{"ok": true|false, "action": ..., "seg_num": ..., "error": ...}`

**2. Running PIDs registry** in `__main__.py`:
- `_running_pids: dict[int, int]` — seg_num → OS PID
- Populated by `run_segment()` immediately after `create_subprocess_exec`
- Passed into aiohttp app as `app["running_pids"]`
- Kill uses `os.killpg(os.getpgid(pid), signal.SIGTERM)` — works with `start_new_session=True`

**3. `reset_for_retry()` in `state.py`:**
```python
UPDATE segments SET status='pending', attempts=0, started_at=NULL, finished_at=NULL WHERE num=?
```
Preserves `segment_attempts` history — attempt records are never deleted.

**4. Dashboard buttons** (contextual per row):
- Running → Kill (✕)
- Failed/Blocked/Partial/Timeout → Retry (↺) + Skip (⏭)
- Pending → Skip (⏭)
- Confirmation dialog before all destructive actions

**5. CLI subcommands:**
```bash
python -m scripts.orchestrate_v2 skip 11 .claude/plans/phase2-coverage-hardening
python -m scripts.orchestrate_v2 retry 11 .claude/plans/phase2-coverage-hardening
```
These write directly to `state.db` — useful before/between runs without a running orchestrator.

**6. Attempt history in `--status` CLI:**
Shows each attempt's duration, status, and token count below the segment status line.

## Existing Solutions Evaluated

- Celery Flower `/api/task/revoke/{task_id}` and `/api/task/apply/{taskname}` endpoints: reference implementation for the pattern. Not a library to adopt — design pattern adopted.
- Temporal Web UI workflow terminate/cancel: same pattern. Reference only.

## Alternatives Considered

- WebSocket for bidirectional control: more complex than REST POST endpoint. Same capability, more code. Rejected.
- File-based signal (write a `skip-S11` file that the orchestrator polls): fragile, not atomic. Rejected.

## Pre-Mortem

- Kill uses `os.getpgid(pid)` then `os.killpg()`. With `start_new_session=True`, the subprocess is its own session leader, so `getpgid(pid) == pid`. This is correct — `killpg(pid, SIGTERM)` kills the whole process group rooted at the claude subprocess.
- Retry via API resets the DB row, but if the wave has already completed, the segment won't re-run automatically in the current session. Document this in the confirmation dialog: "Segment will run on next wave or orchestrator restart."
- `_running_pids` dict is shared between aiohttp handler and asyncio loop — both in the same thread (asyncio is single-threaded), no lock needed.
- `skipped` status must be handled in `_run_wave`: skip segments with status `skipped` (same as `pass`).

## Risk Factor

5/10 — Adds new API surface and PID registry. The kill pathway uses OS signals — must be tested carefully.

## Evidence for Optimality

- *External*: Celery Flower v1.0 has had revoke/retry since 2013 — 12 years of production validation for this pattern.
- *Codebase*: `runner.py` already uses `start_new_session=True` and has a `_kill_tree()` function — the PID registry just makes the target addressable from outside the coroutine.

## Blast Radius

- Direct: `monitor.py` (control endpoint), `__main__.py` (PID registry, retry/skip logic, CLI subcommands), `runner.py` (expose PID via callback), `state.py` (reset_for_retry, skipped status handling), `dashboard.html` (action buttons)
