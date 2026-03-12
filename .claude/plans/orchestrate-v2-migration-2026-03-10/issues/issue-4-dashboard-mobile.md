---
id: "4"
title: "Dashboard not mobile-usable or feature-complete"
risk: 3/10
addressed_by_segments: [4]
---

# Issue 4: Dashboard not mobile-usable or feature-complete

## Core Problem

The dashboard has a hardcoded `380px 1fr` two-column grid — completely unusable on a phone screen. No responsive breakpoints despite having the `<meta name="viewport">` tag. Additionally: no log search, no status filter, no ETA, no elapsed time per segment, no keyboard shortcuts, no localStorage persistence (selected segment lost on refresh), no notification delivery log, mouse-only navigation.

## Root Cause

Built for desktop only with no mobile breakpoints. All interactive features (log search, filter, ETA) were deferred.

## Proposed Fix

**Mobile-first layout:**
- `<640px`: single column, sticky bottom tab bar with 3 tabs (Timeline | Log | Events). Active tab visible, others `display:none`.
- `≥641px`: existing two-column grid unchanged. Tab bar hidden.
- Segment rows: `min-height: 48px` for touch targets.

**New features:**
- **ETA**: `avg_completed_duration × pending_count ÷ max_parallel` in header. Updates every 5s.
- **Elapsed time**: shown on running segment rows, updated every second.
- **Log search**: text input filters log lines in real-time. Highlight matches. Show match count.
- **Log color coding**: ERROR/BLOCKED → red; PASS/✅ → green; `→ tool:` calls → dim monospace; WARN/⚠️ → amber.
- **Keyboard shortcuts**: `j`/`k` navigate segments, `Enter` open log, `/` focus search, `Escape` clear search, `f` cycle status filter.
- **Status filter**: dropdown: All / Running / Failed-Blocked / Pending / Passed.
- **localStorage persistence**: `activeSeg` survives refresh.
- **Auto-select**: when nothing selected, auto-open first running segment's log.
- **Notification log**: fourth section showing recent `notifications` table rows (sent_at, attempts, kind, message preview) — confirms delivery.
- **Event severity colors**: error events in red, warn in amber, info in default dim.

## Existing Solutions Evaluated

N/A — internal UI. Pattern references: Temporal Web UI (saved filters, keyboard nav), Sidekiq dashboard (job duration display), log viewer UX from LazyTail/Gonzo (search, color coding).

## Alternatives Considered

- WebSocket instead of SSE: bidirectional, but SSE + REST control API (S5) achieves same outcome with less churn. Rejected.
- External JS framework (React, Vue): adds build step, no CDN imports allowed. Rejected — vanilla JS only.

## Pre-Mortem

- Log lines must be rendered as `<div class="log-line">` elements (not raw `textContent`) to support per-line styling and search filtering.
- Keyboard shortcuts must not fire when focus is inside an `<input>` — check `e.target.tagName === 'INPUT'`.
- `localStorage` saves `activeSeg` as string — parse with `parseInt()` on load.
- ETA only meaningful when ≥2 segments have completed. Show nothing otherwise.
- Auto-select should only trigger once on load, not override manual selection on every refresh.

## Risk Factor

3/10 — Pure frontend. No Python changes except minor additions to `monitor.py` to expose `severity` and `notifications` in the state API (already handled by S1+S2 schema and `all_as_dict()`).

## Evidence for Optimality

- *External*: k9s, Temporal Web UI, and Gonzo all use `j`/`k` keyboard navigation — established convention for terminal-adjacent monitoring tools.
- *External*: 48px minimum touch target size from Apple HIG and Material Design guidelines.

## Blast Radius

- Direct: `dashboard.html` (full rewrite)
- Ripple: `monitor.py` (minor: ensure `notifications` and `severity` fields appear in `/api/state` response — already done via S1+S2)
