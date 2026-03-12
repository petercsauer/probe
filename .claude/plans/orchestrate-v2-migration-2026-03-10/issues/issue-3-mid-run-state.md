---
id: "3"
title: "No mid-run state updates — crash leaves segments in the dark"
risk: 4/10
addressed_by_segments: [3]
---

# Issue 3: No mid-run state updates — crash leaves segments in the dark

## Core Problem

A segment's DB row is written exactly twice: once when it starts (`status=running, started_at=now`) and once when it ends. If the orchestrator is killed mid-run, there is no way to know how long a segment has been running, whether it was making progress, or whether it was stuck. The `reset_stale_running()` startup fix just resets everything to `pending` — losing all progress info.

Three sub-problems:
1. No heartbeat writes during execution — no liveness signal
2. All segments share a single global timeout — heavy segments (S11 integration) get the same 3600s as trivial ones
3. The Python v2 dropped the bash version's network outage detection — if AWS Bedrock goes flaky mid-run, segments silently fail

## Root Cause

No watchdog task. No `timeout` field in segment frontmatter. No network health check before launching new segments.

## Proposed Fix

**1. Heartbeat task per running segment:**
- Wakes every 60s, reads tail of `.stream.jsonl` (last 2KB), extracts most recent text snippet
- Writes `last_seen_at=now` and `last_activity=<snippet>` to the segment row
- Logs `segment_heartbeat` event with severity=info
- If file size unchanged for >stall_threshold seconds (default 1800s), enqueues `stall` notification

**2. Per-segment timeout override:**
- Add `timeout: int = 0` to `Segment` dataclass and frontmatter parsing in `planner.py`
- In `runner.py`: `segment_timeout = seg.per_segment_timeout or config.segment_timeout`
- Example frontmatter: `timeout: 7200` for heavy integration segments

**3. Network outage detection:**
- `_wait_for_network(notifier, max_wait)` polls `https://api.anthropic.com` with 5s timeout
- On failure: exponential backoff (10s→60s, capped at 60s), max_wait=600s
- Sends ntfy notification once after 60s of downtime
- Called before each wave launch

**4. Attempt history:**
- `record_attempt(seg_num, attempt, started_at, finished_at, status, summary, tokens_in, tokens_out)` after each segment completes
- Parse token usage from stream-json `result` event: `obj.get("usage", {}).get("input_tokens", 0)`

## Existing Solutions Evaluated

N/A — internal monitoring. Pattern inspired by Sidekiq/Celery worker heartbeats, adapted for single-process asyncio orchestrator.

## Alternatives Considered

- Parse stream in real-time during drain (producer/consumer pattern): more complex, no benefit over tailing the file. Rejected.
- `inotify`/`watchdog` for file change events: adds dependency, overkill for 60s polling. Rejected.

## Pre-Mortem

- Reading `.stream.jsonl` from heartbeat task while `_drain_stdout` writes to it: safe (read-only vs append-only). Partial last line: read only complete lines, discard last fragment.
- If heartbeat task throws (file not yet created, JSON parse error), it must not crash `run_segment()` — wrap body in `try/except`.
- Heartbeat task must be cancelled in `finally` even if `_drain_stdout` raises.
- `_extract_text_from_stream_line` is in `monitor.py` — importing it in `runner.py` may cause circular import if `monitor.py` ever imports from `runner.py`. Audit; if circular, extract to `streamparse.py`.
- Network check uses `httpx` — must be installed (pinned in requirements.txt from S1).

## Risk Factor

4/10 — Additive changes, no existing code paths modified beyond adding the heartbeat task alongside the existing drain.

## Evidence for Optimality

- *Codebase*: `monitor.py` already has `_extract_text_from_stream_line()` — heartbeat task reuses it directly.
- *External*: Sidekiq, Celery, Resque all implement heartbeat writes for liveness detection. 30min stall threshold = 50% of segment timeout — standard "something is wrong" signal.

## Blast Radius

- Direct: `runner.py` (heartbeat task, network check, token parsing), `planner.py` (timeout frontmatter), `state.py` (update_heartbeat, record_attempt methods)
- Ripple: `config.py` (stall_threshold, network_retry_max), `__main__.py` (network check call, pass notifier to run_segment)
