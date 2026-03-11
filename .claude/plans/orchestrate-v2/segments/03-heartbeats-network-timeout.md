---
segment: 3
title: "Heartbeats + network detection + per-segment timeout"
depends_on: [1]
risk: 4/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(orchestrate_v2): add heartbeats, stall detection, network check, per-segment timeout"
---

# Segment 3: Heartbeats + network detection + per-segment timeout

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Write `last_seen_at`/`last_activity` every 60 seconds per running segment; detect stalls and enqueue notifications; check network before launching segments; read per-segment `timeout` from frontmatter; record attempt history on completion.

**Depends on:** Segment 1 (async StateDB with `last_seen_at`, `last_activity`, `per_segment_timeout` columns on `segments`, `segment_attempts` table). Segment 2 should be done first for `notifier.stall()` and `notifier.network_down()` to exist, but can be run in parallel — if S2 isn't done, stub those calls.

## Context: Issues Addressed

**Issue 3 — No mid-run state updates:**
A segment's DB row is only written at start and end. A crash mid-run leaves segments permanently `running` with no progress info. Additionally: all segments share a single global timeout (heavy segments like S11 integration need more), and the Python v2 dropped the bash version's network outage detection.

Fixes:
1. **Heartbeat task** per running segment: every 60s, reads tail of `.stream.jsonl` (last 2KB), extracts most recent text snippet via `_extract_text_from_stream_line`, writes `last_seen_at` + `last_activity` to DB.
2. **Stall detection**: if file size unchanged for >stall_threshold seconds, enqueues `notifier.stall()`.
3. **Per-segment timeout**: `timeout` frontmatter field, falls back to `config.segment_timeout`.
4. **Network check**: poll `https://api.anthropic.com` before each wave, wait with backoff if down.
5. **Attempt history**: `record_attempt()` after each segment completes; token usage parsed from stream-json `result` event.

## Scope

- `scripts/orchestrate_v2/runner.py` — `_segment_heartbeat_task`, token parsing, `record_attempt` call, timeout override, `register_pid`/`unregister_pid` callbacks
- `scripts/orchestrate_v2/planner.py` — add `timeout: int = 0` to `Segment` dataclass and frontmatter parsing
- `scripts/orchestrate_v2/state.py` — `update_heartbeat()` and `record_attempt()` already declared in S1; implement them here if not done
- `scripts/orchestrate_v2/__main__.py` — `_wait_for_network()`, pass `notifier` to `run_segment`, `stall_threshold`/`network_retry_max` from config
- `scripts/orchestrate_v2/config.py` — add `stall_threshold: int = 1800`, `network_retry_max: int = 600`

## Key Files and Context

**`runner.py` current `run_segment()` core (lines 197–205):**
```python
try:
    await asyncio.wait_for(_drain_stdout(), timeout=config.segment_timeout)
except asyncio.TimeoutError:
    log.warning("S%02d timed out after %ds", seg.num, config.segment_timeout)
    if proc.pid:
        await _kill_tree(proc.pid)
    state.set_status(seg.num, "timeout", ...)
    return "timeout", f"Killed after {config.segment_timeout}s"
```

**New heartbeat task:**
```python
from .monitor import _extract_text_from_stream_line  # audit for circular import first

async def _segment_heartbeat_task(
    seg_num: int, raw_log: Path, state: StateDB, notifier,
    started_at: float, heartbeat_interval: int = 60, stall_threshold: int = 1800,
) -> None:
    last_size, stall_notified = 0, False
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

**Launch alongside drain in `run_segment()`:**
```python
started_at = time.time()
segment_timeout = getattr(seg, 'per_segment_timeout', 0) or config.segment_timeout
heartbeat = asyncio.create_task(
    _segment_heartbeat_task(
        seg.num, raw_log, state, notifier, started_at,
        stall_threshold=config.stall_threshold,
    )
)
try:
    await asyncio.wait_for(_drain_stdout(), timeout=segment_timeout)
except asyncio.TimeoutError:
    ...
finally:
    heartbeat.cancel()
    try:
        await heartbeat
    except asyncio.CancelledError:
        pass
```

**Token parsing:**
```python
def _extract_token_usage(raw_path: Path) -> tuple[int, int]:
    try:
        with open(raw_path, errors="replace") as f:
            for line in f:
                line = line.strip()
                if not line: continue
                try: obj = json.loads(line)
                except: continue
                if obj.get("type") == "result":
                    u = obj.get("usage", {})
                    return u.get("input_tokens", 0), u.get("output_tokens", 0)
    except Exception:
        pass
    return 0, 0
```

Call after `_parse_stream_jsonl`:
```python
tokens_in, tokens_out = _extract_token_usage(raw_log)
await state.record_attempt(
    seg.num, state_attempts_count, started_at, time.time(),
    status, summary, tokens_in, tokens_out
)
```

**Per-segment timeout in `planner.py`:**
```python
@dataclass
class Segment:
    ...
    timeout: int = 0   # 0 = use config default

# In load_plan():
segments.append(Segment(
    ...
    timeout=sfm.get("timeout", 0),
))
```

**`run_segment()` signature** — add `notifier` and PID callbacks:
```python
async def run_segment(
    seg: Segment,
    config: OrchestrateConfig,
    state: StateDB,
    log_dir: Path,
    notifier=None,
    register_pid=None,
    unregister_pid=None,
) -> tuple[str, str]:
    ...
    proc = await asyncio.create_subprocess_exec(...)
    if register_pid:
        register_pid(seg.num, proc.pid)
    ...
    finally:
        if unregister_pid:
            unregister_pid(seg.num)
```

**`_wait_for_network` in `__main__.py`:**
```python
async def _wait_for_network(notifier, max_wait: int = 600) -> None:
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
        if not notified and waited >= 60 and notifier:
            await notifier.network_down(waited)
            notified = True
        await asyncio.sleep(delay)
        delay = min(delay * 2, 60)
```

Call before each wave: `await _wait_for_network(notifier, config.network_retry_max)`.

**`--status` CLI update** — show elapsed and last activity for running segments:
```python
for seg in data["segments"]:
    ...
    if seg.get("last_seen_at") and seg["status"] == "running":
        age = int(time.time() - seg["last_seen_at"])
        act = (seg.get("last_activity") or "")[:60]
        print(f"    └─ last seen {age}s ago: {act}")
    for att in seg.get("attempts_history", []):
        dur = f"{int(att['finished_at'] - att['started_at'])}s" if att.get('finished_at') else "--"
        tok = f"{att['tokens_in']+att['tokens_out']:,} tok" if att.get('tokens_in') else ""
        print(f"    attempt {att['attempt']}: {att['status']} ({dur}) {tok}")
```

## Implementation Approach

1. Check for circular import: does `monitor.py` import from `runner.py`? If yes, extract `_extract_text_from_stream_line` to `scripts/orchestrate_v2/streamparse.py` and update both importers.
2. Add `_segment_heartbeat_task` to `runner.py`.
3. Add `_extract_token_usage` to `runner.py`.
4. Update `run_segment()` to launch heartbeat, use per-segment timeout, record attempt, accept `notifier`/PID callbacks.
5. Add `timeout` to `Segment` dataclass and frontmatter parsing in `planner.py`.
6. Add `_wait_for_network` to `__main__.py`. Call before each wave in orchestration loop.
7. Update `_run_one` in `__main__.py` to pass `notifier` + PID callbacks to `run_segment`.
8. Add `stall_threshold` and `network_retry_max` to `config.py` and TOML parsing.
9. Update `cmd_status` display.

## Alternatives Ruled Out

- Parse stream in real-time during drain (producer/consumer): more complex, no benefit over tailing the file. Rejected.
- `inotify`/`watchdog` for file change detection: external dependency, overkill. Rejected.

## Pre-Mortem Risks

- Heartbeat task reads file while drain writes: safe (read-only vs append-only). Discard partial last line (take `splitlines()[:-1]`).
- Heartbeat body wrapped in `try/except` — any exception must not propagate out of the task.
- Heartbeat must be cancelled in `finally` even if `_drain_stdout` itself raises.
- `httpx` must be installed (from S1 requirements.txt).
- Circular import between `runner.py` and `monitor.py`: audit before importing. If circular, extract to `streamparse.py`.
- `record_attempt` needs `state_attempts_count` — get it from `state.increment_attempts(seg.num)` return value (already called in `_run_one` in `__main__.py`). Pass it into `run_segment()` as a parameter or retrieve it from the DB.

## Build and Test Commands

- **Build**: `python -m py_compile scripts/orchestrate_v2/*.py`
- **Test (targeted)**:
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
      await db.record_attempt(1, 1, time.time()-60, time.time(), 'pass', 'done', 100, 200)
      atts = await db.get_attempts(1)
      assert len(atts) == 1 and atts[0]['tokens_in'] == 100, atts
      print('PASS')
      await db.close()
  asyncio.run(t())
  "
  # Verify per-segment timeout is parsed from frontmatter:
  python3 -c "
  from scripts.orchestrate_v2.planner import load_plan
  from pathlib import Path
  _, segs = load_plan(Path('.claude/plans/phase2-coverage-hardening'))
  s11 = next(s for s in segs if s.num == 11)
  print(f'S11 timeout: {s11.timeout}')  # 0 if no frontmatter field yet
  "
  ```
- **Test (regression)**: `python -m scripts.orchestrate_v2 dry-run .claude/plans/phase2-coverage-hardening`
- **Test (full gate)**: `python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening`

## Exit Criteria

1. **Targeted tests:**
   - `update_heartbeat()` writes `last_seen_at` and `last_activity` to segment row.
   - `record_attempt()` stores attempt with correct token counts.
   - `Segment.timeout` parsed from frontmatter (add `timeout: 7200` to segment 11's `.md` frontmatter to test).
   - `_extract_token_usage` returns (0, 0) gracefully on missing/empty file.
2. **Regression tests:** `dry-run` and `status` exit 0.
3. **Full build gate:** `python -m py_compile scripts/orchestrate_v2/*.py`
4. **Full test gate:** `status .claude/plans/phase2-coverage-hardening`
5. **Self-review gate:** Heartbeat cancelled in `finally`. No circular imports. `_extract_text_from_stream_line` imported cleanly.
6. **Scope verification gate:** Only `scripts/orchestrate_v2/` modified.
