---
id: "2"
title: "sqlite3 sync calls blocking the asyncio event loop"
risk: 7/10
addressed_by_segments: [1]
---

# Issue 2: sqlite3 sync calls blocking the asyncio event loop

## Core Problem

`StateDB` uses `sqlite3.connect(..., check_same_thread=False)` and all methods are synchronous. When multiple segments finish simultaneously and call `state.set_status()` / `state.log_event()`, these blocking I/O calls execute on the asyncio event loop thread, stalling all other coroutines: heartbeat tasks, the monitor SSE stream, and the notification worker all freeze until the write completes.

## Root Cause

`check_same_thread=False` disables SQLite's thread-safety check but doesn't make I/O non-blocking. There is no `await` in any `StateDB` method. The event loop cannot yield while waiting for disk I/O.

## Proposed Fix

Migrate `StateDB` from `sqlite3` to `aiosqlite` (v0.22.1). Every method becomes `async def`, every query becomes `await conn.execute(...)`. `aiosqlite` runs the sqlite3 connection on a dedicated background thread with an internal asyncio queue — all blocking I/O is off the event loop thread. Single long-lived connection matches the existing usage pattern.

Also add all new schema in this segment (since it's the schema foundation):
- `notifications` table (for outbox, Issue 1)
- `segment_attempts` table (for attempt history, Issue 3)
- `severity TEXT DEFAULT 'info'` column on `events`
- `last_seen_at REAL`, `last_activity TEXT`, `per_segment_timeout INTEGER` columns on `segments`
- `last_attempt_at REAL` column on `notifications`

Schema additions use `ALTER TABLE ... ADD COLUMN` wrapped in try/except (SQLite raises `OperationalError: duplicate column name` if column already exists — idempotent).

## Existing Solutions Evaluated

- `aiosqlite` (omnilib/aiosqlite, MIT, v0.22.1, 2.5k ★, Python 3.8+, 2024 CPU/locking improvements): **adopted**. Standard recommendation for asyncio + SQLite.
- `databases` (encode/databases): async DB abstraction layer. Overkill for a single-file schema. Rejected.
- `run_in_executor(ThreadPoolExecutor(max_workers=1))`: works but requires manual wrapping of every method. Rejected.

## Alternatives Considered

- Keep sync SQLite + asyncio.Lock: prevents concurrent writes but still blocks the event loop. Does not solve the core problem. Rejected.
- Move to PostgreSQL: no PostgreSQL in this environment, single-machine tool. Rejected.

## Pre-Mortem

- Missing `await` compiles silently (returns coroutine object). After migration: `python -m py_compile scripts/orchestrate_v2/*.py` + grep for un-awaited state calls.
- `aiosqlite.connect(path)` returns a connection object when awaited directly (not `async with`) — use the long-lived pattern, not the context manager pattern.
- `cmd_status` and `cmd_dry_run` are sync entry points that call `asyncio.run()`. `StateDB.create()` must be called inside the async context.
- `migrate_from_json()` reads a file + does DB writes — must become async too.

## Risk Factor

7/10 — Touches all state call sites across 4 files. Missing a single `await` causes subtle runtime bugs.

## Evidence for Optimality

- *External*: aiosqlite docs explicitly recommend a single long-lived connection shared across coroutines — matches existing usage pattern.
- *External*: SO #63813922 confirms aiosqlite preferred over run_in_executor for deterministic thread behavior.

## Blast Radius

- Direct: `state.py` (full rewrite), `__main__.py` (all state calls add `await`), `runner.py` (same), `monitor.py` (same)
- Ripple: any future code that instantiates `StateDB`
