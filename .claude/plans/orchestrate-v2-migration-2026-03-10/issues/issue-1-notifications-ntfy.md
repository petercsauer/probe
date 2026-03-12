---
id: "1"
title: "Notifications silently dropped — replace osascript with ntfy.sh"
risk: 4/10
addressed_by_segments: [2]
---

# Issue 1: Notifications silently dropped — replace osascript with ntfy.sh

## Core Problem

`notify.py`'s `_send_imessage()` makes a single osascript call with a 15-second timeout. macOS `Messages.app` has a hardcoded 10-second AppleScript handler timeout and is documented to delay sends by up to 5 minutes during idle. Any transient failure silently drops the notification forever — no retry, no queue, no persistence. Callers never check the return value of `Notifier.send()`.

## Root Cause

Two compounding failures: (1) the osascript → Messages.app transport is inherently unreliable on macOS; (2) fire-and-forget with zero retry. Either alone causes sporadic misses; both together guarantee them.

## Proposed Fix

Replace osascript with **ntfy.sh HTTP transport** + **transactional outbox**:

1. `_send_ntfy(topic, message, title, priority, tags, click_url)` — plain `httpx.AsyncClient.post()` to `https://ntfy.sh/{topic}`. No account, no API key.
2. `notifications` table in `state.db`: `(id, created_at, event_key UNIQUE, kind, message, priority, sent_at, attempts, last_attempt_at, last_error)`.
3. `Notifier.enqueue(kind, message, ...)` writes a row atomically (INSERT OR IGNORE on `event_key` for dedup).
4. `_notification_worker` asyncio task: polls every 10s, retries with exponential backoff (10s→60s→300s).
5. Notification batching: wave completions send one message summarising all segment results.
6. Verbosity config: `all` | `failures_only` | `waves_only` | `final_only`.
7. ntfy priority levels: `urgent` for failures/blocked, `high` for stalls/wave failures, `default` for progress, `min` for heartbeats.
8. Click-through URL: ntfy `Click` header → `http://localhost:{monitor_port}`.

## Existing Solutions Evaluated

- `apprise` (PyPI): routes to 100+ services including ntfy. Adds a dependency with no benefit for single-target use. Rejected.
- `python-telegram-bot`: requires account + bot token. Rejected (no key desired).
- Pushover: $5 + API key. Rejected.
- Hand-rolled ntfy + httpx outbox: **adopted**.

## Alternatives Considered

- Keep osascript with retry: transport fundamentally unreliable regardless. Rejected.
- asyncio.Queue in-memory: notifications lost on crash. Rejected — outbox in DB is the whole point.

## Pre-Mortem

- ntfy.sh cloud could be down (rare). Worker retries cover brief outages.
- `httpx` not installed → import error. Pin in `requirements.txt`.
- At-least-once: duplicate notifications possible if worker crashes after send but before `sent_at` write. Mitigated by `event_key` dedup on INSERT OR IGNORE.
- ntfy free tier anonymous limit: 17,280 messages/12h per IP. A full 24-segment run sends ~50. No practical limit.
- UUID-style topic (16+ hex chars) is security-by-obscurity: guessable only by brute force. Acceptable for personal overnight notifications.

## Risk Factor

4/10 — HTTP is far more debuggable than osascript. Failure modes are standard HTTP error codes.

## Evidence for Optimality

- *External*: ntfy.sh documented as zero-setup HTTP pub/sub with offline message caching; community consensus for script self-notification (homelabstarter.com, selfhosting.sh).
- *External*: Transactional outbox is the 2026 consensus for at-least-once delivery without a message broker (james-carr.org Jan 2026, medium.com/@dsbraz Feb 2026).

## Blast Radius

- Direct: `notify.py` (full rewrite), `state.py` (outbox schema + methods), `__main__.py` (worker task)
- Ripple: `config.py` (ntfy_topic, verbosity, retry fields), `monitor.py` (expose notifications in API)
