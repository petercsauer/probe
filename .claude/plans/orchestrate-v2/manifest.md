---
plan: "Orchestrate v2 — Reliability Hardening"
goal: "Replace osascript with ntfy.sh HTTP notifications, migrate SQLite to aiosqlite, add mid-run heartbeats + stall detection + network outage handling, ship a mobile-first dashboard with operator controls, ETA, log search, and keyboard navigation."
generated: 2026-03-10
status: Ready for execution
parent_plan: ""
rules_version: 2026-03-10
---

# Orchestrate v2 — Reliability Hardening — Manifest

## Overview

The existing Python orchestrator (`scripts/orchestrate/`) has two hard reliability failures: (1) notifications are delivered via a single-shot osascript call that silently drops messages whenever macOS Messages.app is slow or asleep; (2) all SQLite writes are synchronous blocking calls on the asyncio event loop thread, stalling coroutines when multiple segments finish simultaneously. Additionally there are no mid-run state updates (a crash leaves segments permanently "running"), the dashboard is desktop-only and read-only, and there's no way to skip or retry a segment without restarting the whole orchestrator.

All work goes into `scripts/orchestrate_v2/` (a copy of the original). The original `scripts/orchestrate/` and `scripts/orchestrate_backup/` are never modified.

## Dependency Diagram

```
S1 (aiosqlite + full schema)
├── S2 (ntfy outbox + batching) ─────────────────────┐
├── S3 (heartbeats + network + timeout override) ──┐  │
│                                                   │  │
└──────────────────────────────────────────────────S4 (mobile dashboard + UX)
                                                       │
                                                       S5 (operator control API + buttons)
```

S1 must land first. **S2 ∥ S3** can run in parallel. S4 after S2. S5 last.

## Segment Index

| # | Title | File | Depends On | Risk | Complexity | Status |
|---|-------|------|------------|------|------------|--------|
| 1 | aiosqlite migration + full schema | segments/01-aiosqlite-full-schema.md | None | 7/10 | High | pending |
| 2 | ntfy outbox + batching + verbosity | segments/02-ntfy-outbox-batching.md | 1 | 4/10 | Medium | pending |
| 3 | Heartbeats + network detection + timeout | segments/03-heartbeats-network-timeout.md | 1 | 4/10 | Medium | pending |
| 4 | Mobile dashboard + UX features | segments/04-mobile-dashboard-ux.md | 2 | 3/10 | Medium | pending |
| 5 | Operator control API + dashboard buttons | segments/05-operator-control-api.md | 1,2,3,4 | 5/10 | Medium | pending |

## Parallelization

- **S2 ∥ S3**: Both depend only on S1. S2 adds the `notifications` table; S3 adds `segment_attempts`, `last_seen_at`, `last_activity`, `per_segment_timeout` columns. No schema conflicts.
- **S4**: Must follow S2 (needs notifications in `/api/state`).
- **S5**: Must follow S4 (adds buttons to dashboard + uses PID registry from orchestrator).

## Preamble Injection

Before launching any builder subagent, the orchestration agent assembles the prompt:
1. Read `.claude/commands/iterative-builder.md`
2. Read the segment file from `segments/{NN}-{slug}.md`

Assembled prompt = [preamble contents] + [segment file contents]

## Pre-step (already done)

```bash
cp -r scripts/orchestrate scripts/orchestrate_backup   # untouched reference
cp -r scripts/orchestrate scripts/orchestrate_v2       # working copy
```

## ntfy One-Time Setup

1. Generate a UUID-style topic: `python3 -c "import uuid; print('prb-' + uuid.uuid4().hex[:16])"`  
   Example: `prb-a3f8c12b9e4d7051`
2. Install the ntfy iOS or Android app, subscribe to your topic
3. Set `ntfy_topic = "prb-a3f8c12b9e4d7051"` in `.claude/plans/phase2-coverage-hardening/orchestrate.toml` under `[notifications]`

## Verified Library Versions (2026-03-10)

| Library | Version to pin | Source |
|---------|---------------|--------|
| aiosqlite | 0.22.1 | PyPI |
| httpx | 0.28.1 | PyPI |
| aiohttp | 3.13.3 | PyPI |

`requirements.txt` for `scripts/orchestrate_v2/`:
```
aiosqlite>=0.22.1
httpx>=0.28.1
aiohttp>=3.13.3
```

## Execution Instructions

Switch to Agent Mode. Execute segments in dependency order:

```bash
# Install deps first
pip install "aiosqlite>=0.22.1" "httpx>=0.28.1" "aiohttp>=3.13.3"

# S1 — aiosqlite migration (highest risk, do first)
# After S1: python -m scripts.orchestrate_v2 dry-run .claude/plans/phase2-coverage-hardening

# S2 + S3 can run in parallel
# After S2: send a real test ntfy message to phone
# After S3: verify last_seen_at updates in DB

# S4 — dashboard
# Test at 375px (mobile) and 1440px (desktop)

# S5 — operator control
# Test: python -m scripts.orchestrate_v2 skip 99 .claude/plans/phase2-coverage-hardening

# Final smoke test:
python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening
python -m scripts.orchestrate_v2 run .claude/plans/phase2-coverage-hardening --monitor 8078
```
