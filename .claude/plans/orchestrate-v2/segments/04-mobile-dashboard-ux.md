---
segment: 4
title: "Mobile-first dashboard + UX features"
depends_on: [2]
risk: 3/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(orchestrate_v2): mobile-first dashboard with search, keyboard nav, ETA, and notifications log"
---

# Segment 4: Mobile-first dashboard + UX features

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Rewrite `dashboard.html` with mobile-first responsive layout (tab navigation on mobile), ETA, elapsed time per segment, log search + color coding, keyboard shortcuts, status filter, localStorage persistence, event severity colors, notification log, and auto-select of first running segment.

**Depends on:** Segment 2 (notifications in `/api/state` response), Segment 1 (`last_seen_at`, `severity` in events). S3 heartbeat data (`last_seen_at`) improves elapsed display but is not required.

## Context: Issues Addressed

**Issue 4 — Dashboard not mobile-usable:**
The dashboard has a hardcoded `380px 1fr` two-column grid — unusable on phone screens. No responsive breakpoints. No log search, filter, ETA, elapsed time, keyboard shortcuts, or localStorage.

Fix: mobile-first CSS with `<640px` single-column tab layout. All new features added. No external JS or CSS libraries — vanilla only, no CDN imports.

## Scope

- `scripts/orchestrate_v2/dashboard.html` — full rewrite
- `scripts/orchestrate_v2/monitor.py` — minor: ensure `notifications` array and event `severity` field are included in `/api/state` response (should already work via S1+S2 `all_as_dict()` — verify and adjust if not)

## Key Files and Context

**`/api/state` response shape after S1+S2:**
```json
{
  "plan_title": "...",
  "current_wave": "2",
  "max_wave": 6,
  "progress": {"pass": 10, "running": 1, "pending": 13},
  "segments": [
    {
      "num": 11, "title": "...", "wave": 2, "status": "running",
      "started_at": 1741234567.0, "finished_at": null,
      "last_seen_at": 1741234627.0, "last_activity": "→ Bash: cargo test...",
      "attempts_history": [{"attempt": 1, "status": "failed", "tokens_in": 45000, ...}]
    }, ...
  ],
  "events": [{"id": 42, "ts": 1741234567.0, "kind": "wave_start", "detail": "...", "severity": "info"}, ...],
  "notifications": [{"id": 1, "kind": "wave_complete", "message": "...", "sent_at": 1741234570.0, "attempts": 1}, ...]
}
```

**Mobile layout strategy:**
```css
/* Tab bar — mobile only */
.tab-bar {
  display: none;
  position: fixed; bottom: 0; left: 0; right: 0; height: 52px;
  background: var(--surface); border-top: 1px solid var(--border);
  z-index: 100;
}
.tab-btn {
  flex: 1; background: none; border: none; color: var(--text-dim);
  font-size: 11px; font-weight: 600; cursor: pointer;
  display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 2px;
}
.tab-btn.active { color: var(--running); }

@media (max-width: 640px) {
  .layout {
    display: block;
    height: calc(100vh - 100px - 52px); /* header + tab bar */
    padding-bottom: 52px;
  }
  .timeline { display: none; height: 100%; overflow-y: auto; }
  .log-panel { display: none; height: 100%; }
  .events { display: none; max-height: calc(100vh - 100px - 52px); }
  .timeline.tab-active, .log-panel.tab-active, .events.tab-active { display: flex; }
  .timeline.tab-active { display: block; }
  .seg-row { min-height: 48px; padding: 10px 16px; }
  .tab-bar { display: flex; }
}
@media (min-width: 641px) {
  .layout { display: grid; grid-template-columns: 380px 1fr; grid-template-rows: 1fr auto; height: calc(100vh - 90px); }
  .tab-bar { display: none !important; }
}
```

**ETA calculation:**
```javascript
function computeEta(segments, maxParallel) {
  const done = segments.filter(s => s.finished_at && s.started_at);
  if (done.length < 2) return '';
  const avg = done.reduce((s, seg) => s + (seg.finished_at - seg.started_at), 0) / done.length;
  const pending = segments.filter(s => ['pending','running'].includes(s.status)).length;
  const etaSec = avg * pending / Math.max(maxParallel, 1);
  return etaSec > 30 ? `ETA: ~${formatElapsed(etaSec)}` : '';
}
```

**Elapsed time on running segment rows:**
```javascript
// In renderTimeline, for s.status === 'running':
const elapsedStr = s.started_at ?
  `<span class="seg-elapsed">${formatElapsed(Date.now()/1000 - s.started_at)}</span>` : '';
```

**Log lines as DOM elements** (replace `logEl.textContent +=` with):
```javascript
function appendLogLine(logEl, text) {
  const div = document.createElement('div');
  div.className = 'log-line ' + classifyLogLine(text);
  div.textContent = text;
  logEl.appendChild(div);
  // Apply current search filter
  const term = document.getElementById('log-search').value.toLowerCase();
  if (term && !text.toLowerCase().includes(term)) div.style.display = 'none';
  logEl.scrollTop = logEl.scrollHeight;
}

function classifyLogLine(text) {
  if (/\b(error|ERROR|BLOCKED|blocked)\b/.test(text)) return 'log-error';
  if (/\b(warn|WARN|⚠️|stall)\b/i.test(text)) return 'log-warn';
  if (/\b(PASS|✅|success|SUCCESS)\b/.test(text)) return 'log-pass';
  if (/^→ /.test(text)) return 'log-tool';
  if (/^  ← /.test(text)) return 'log-result';
  return '';
}
```

CSS for log line classes:
```css
.log-error { color: var(--fail); }
.log-warn  { color: var(--partial); }
.log-pass  { color: var(--pass); }
.log-tool  { color: var(--text-muted); font-style: italic; }
.log-result{ color: var(--text-muted); }
.log-line.highlight { background: rgba(210, 153, 34, 0.15); }
```

**Log search:**
```html
<div class="log-toolbar">
  <input id="log-search" type="text" placeholder="/ search..." autocomplete="off" spellcheck="false">
  <span id="search-count" class="search-count"></span>
</div>
```
```javascript
document.getElementById('log-search').addEventListener('input', function() {
  const term = this.value.toLowerCase();
  const lines = document.querySelectorAll('.log-line');
  let matches = 0;
  lines.forEach(l => {
    const vis = !term || l.textContent.toLowerCase().includes(term);
    l.style.display = vis ? '' : 'none';
    l.classList.toggle('highlight', vis && !!term);
    if (vis && term) matches++;
  });
  document.getElementById('search-count').textContent = term ? `${matches}` : '';
});
```

**Keyboard shortcuts:**
```javascript
document.addEventListener('keydown', function(e) {
  if (['INPUT','TEXTAREA','SELECT'].includes(e.target.tagName)) return;
  if (e.key === 'j') { e.preventDefault(); selectNext(); }
  if (e.key === 'k') { e.preventDefault(); selectPrev(); }
  if (e.key === 'Enter' && activeSeg != null) openLog(activeSeg);
  if (e.key === '/') { e.preventDefault(); document.getElementById('log-search').focus(); }
  if (e.key === 'Escape') {
    document.getElementById('log-search').value = '';
    document.getElementById('log-search').dispatchEvent(new Event('input'));
  }
  if (e.key === 'f') cycleStatusFilter();
});
```

**Status filter:**
```html
<select id="status-filter" onchange="applyStatusFilter()">
  <option value="all">All</option>
  <option value="running">Running</option>
  <option value="fail">Failed/Blocked</option>
  <option value="pending">Pending</option>
  <option value="pass">Passed</option>
</select>
```

**localStorage persistence:**
```javascript
// On load:
const savedSeg = parseInt(localStorage.getItem('orchestrate_activeSeg'));
if (savedSeg) activeSeg = savedSeg;

// On select:
function _selectSeg(num, title, status) {
  activeSeg = num;
  localStorage.setItem('orchestrate_activeSeg', num);
  ...
}
```

**Auto-select first running segment** (in `refreshState`, only if nothing manually selected):
```javascript
if (activeSeg == null) {
  const running = data.segments.find(s => s.status === 'running');
  if (running) window._selectSeg(running.num, running.title, 'running');
}
```

**Notification log** (render in events panel below event feed, or as a separate collapsible section):
```javascript
function renderNotifications(notifs) {
  const el = document.getElementById('notif-feed');
  if (!el || !notifs?.length) return;
  let html = '';
  notifs.forEach(n => {
    const icon = n.sent_at ? '✅' : (n.attempts >= 3 ? '❌' : '⏳');
    const ts = n.sent_at ? formatTs(n.sent_at) : `${n.attempts} att.`;
    const preview = (n.message || '').split('\n')[0].slice(0, 60);
    html += `<div class="notif-line">${icon} <span class="ts">${ts}</span> <span class="kind">${n.kind}</span> ${preview}</div>`;
  });
  el.innerHTML = html;
}
```

**Event severity colors:**
```css
.ev-error { color: var(--fail); }
.ev-warn  { color: var(--partial); }
```
```javascript
// In event feed rendering:
const svClass = {error:'ev-error', warn:'ev-warn'}[ev.severity] || '';
div.className = `event-line ${svClass}`;
```

**Tab bar HTML** (add just before `</body>`):
```html
<div class="tab-bar">
  <button class="tab-btn active" data-tab="timeline" onclick="switchTab('timeline')">
    <span>☰</span><span>Timeline</span>
  </button>
  <button class="tab-btn" data-tab="log" onclick="switchTab('log')">
    <span>📄</span><span>Log</span>
  </button>
  <button class="tab-btn" data-tab="events" onclick="switchTab('events')">
    <span>📡</span><span>Events</span>
  </button>
</div>
```

```javascript
function switchTab(tab) {
  document.querySelectorAll('.tab-btn').forEach(b => b.classList.toggle('active', b.dataset.tab === tab));
  document.querySelector('.timeline').classList.toggle('tab-active', tab === 'timeline');
  document.querySelector('.log-panel').classList.toggle('tab-active', tab === 'log');
  document.querySelector('.events').classList.toggle('tab-active', tab === 'events');
}
// Initialize: timeline active by default
switchTab('timeline');
```

## Implementation Approach

1. Start with the CSS — add mobile breakpoints and tab bar styles.
2. Add the tab bar HTML and `switchTab()` JS.
3. Add log search toolbar HTML and search JS.
4. Switch log rendering from `textContent +=` to `appendLogLine()` with classification.
5. Add ETA to header, elapsed time to segment rows.
6. Add keyboard shortcuts.
7. Add status filter dropdown.
8. Add localStorage read/write.
9. Add auto-select logic in `refreshState`.
10. Add notification log section to events panel.
11. Add event severity color classes.

## Alternatives Ruled Out

- WebSocket instead of SSE: more complex, SSE + REST (S5) achieves same result. Rejected.
- External JS framework (React/Vue): adds build step. Rejected — vanilla JS only.
- CDN imports: rejected — dashboard must work without internet on the monitoring machine.

## Pre-Mortem Risks

- Log lines must be `<div>` elements (not textContent) for search to work — replacing `logEl.textContent +=` is the key behavioral change. SSE `onmessage` handler must be updated everywhere log lines are appended.
- Keyboard shortcuts: check `e.target.tagName` before acting — must not fire inside the search input.
- Auto-select: only trigger when `activeSeg == null` AND on the first poll; don't override manual selection on every 5s refresh.
- ETA: show only when ≥2 segments have completed (avg is noisy with 1 data point).
- `localStorage` key must be unique enough not to collide if multiple plans run: use `orchestrate_activeSeg_{plan_slug}` if needed.
- Tab bar adds 52px to bottom — adjust `layout` height calculation (`calc(100vh - 90px - 52px)` on mobile).

## Build and Test Commands

- **Build**: `python -m py_compile scripts/orchestrate_v2/*.py` (Python unchanged; HTML has no build step)
- **Test (targeted)**:
  ```bash
  # Start a local HTTP server to serve the dashboard statically:
  cd scripts/orchestrate_v2 && python3 -m http.server 9090 &
  # Open http://localhost:9090/dashboard.html in browser devtools at 375px width
  # Verify: tab bar visible, tabs switch correctly, rows are ≥48px
  # Then at 1440px: two-column grid, no tab bar
  ```
  Or run with the real orchestrator: `python -m scripts.orchestrate_v2 run .claude/plans/phase2-coverage-hardening --monitor 8078`
- **Test (regression)**: `python -m scripts.orchestrate_v2 dry-run .claude/plans/phase2-coverage-hardening`
- **Test (full gate)**: `python -m scripts.orchestrate_v2 status .claude/plans/phase2-coverage-hardening`

## Exit Criteria

1. **Targeted tests:**
   - At 375px viewport: tab bar visible, Timeline/Log/Events switch correctly, segment rows ≥48px height.
   - At 1440px: two-column layout unchanged, tab bar hidden.
   - Log search filters lines in real-time; `/` keypress focuses search input.
   - `j`/`k` navigate through segment rows; `f` cycles status filter.
   - Refresh page with a segment selected → same segment still selected (localStorage).
   - Running segments show elapsed time. Header shows ETA when ≥2 segments have completed.
   - Notification log section shows sent/pending/failed rows.
2. **Regression tests:** `dry-run` and `status` exit 0.
3. **Full build gate:** `python -m py_compile scripts/orchestrate_v2/*.py`
4. **Self-review gate:** No CDN/external imports. No inline `onclick` handlers calling undefined functions. All JS in one `<script>` block. `switchTab('timeline')` called on page load.
5. **Scope verification gate:** Only `scripts/orchestrate_v2/` modified.
