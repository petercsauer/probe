---
segment: 04
title: Dashboard Interject UI
depends_on: [3]
risk: 2
complexity: Low
cycle_budget: 10
estimated_lines: ~80 lines
status: pending
---

# Segment 04: Dashboard Interject UI

## Goal

Add "Interject" button to the dashboard that appears for running segments, prompts the user for a message, and sends it to the `/api/control` endpoint with confirmation.

## Context

The dashboard is a single HTML file with embedded CSS and JavaScript. It displays segments in a table with action buttons (skip, kill, retry). We need to add an "Interject" button that follows the existing patterns.

## Current State

**Dashboard file:** `scripts/orchestrate_v2/dashboard.html` (1154 lines)

**Action buttons location:** Lines ~845-850 - rendered dynamically per segment

**Existing patterns:**
- Control button: `.act-btn` class (CSS at ~205-212)
- POST request: `controlSeg()` function (lines 879-893)
- Segment selection: `_selectSeg()` function (lines 1084+)
- Status-based visibility: Buttons only show for certain statuses

**Current buttons:**
- Skip: Only for `pending` status
- Kill: Only for `running` status
- Retry: Only for `failed` status

## Implementation Plan

### 1. Add CSS for Interject Button

Location: After `.act-btn` styles (~line 212)

```css
.act-interject {
  background: var(--running);  /* Blue, matches running status */
  color: white;
}

.act-interject:hover {
  background: #1a5fbf;
  opacity: 0.9;
}
```

~8 lines

### 2. Add Interject Button to Segment Actions

Location: In `_buildActBtns()` function (~line 845-850)

Add after the "kill" button case:

```javascript
if (seg.status === 'running') {
  btns.push(`<button class="act-btn act-interject" onclick="event.stopPropagation();interjectSeg(${seg.num})" title="Interject with message">✏️</button>`);
}
```

~3 lines (add within existing logic that checks for 'running' status)

### 3. Create interjectSeg() Function

Location: After `controlSeg()` function (~line 894)

```javascript
window.interjectSeg = async function(num) {
  // Prompt for message
  const message = prompt(
    `Interject message for S${String(num).padStart(2,'0')}:\n\n` +
    `This will kill the segment and restart it with your message.\n` +
    `Message will appear in the segment's prompt.`,
    ''
  );

  // User cancelled
  if (message === null) return;

  // Validate message
  if (!message.trim()) {
    alert('Message cannot be empty');
    return;
  }

  if (message.length > 2000) {
    alert('Message too long (max 2000 characters)');
    return;
  }

  // Confirm action
  if (!confirm(
    `Kill S${String(num).padStart(2,'0')} and restart with message?\n\n` +
    `Preview: ${message.substring(0, 100)}${message.length > 100 ? '...' : ''}`
  )) {
    return;
  }

  // Send to API
  try {
    const r = await fetch('/api/control', {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({
        action: 'interject',
        seg_num: num,
        message: message
      }),
    });
    const data = await r.json();
    if (!data.ok) {
      alert(`Interject failed: ${data.error || 'unknown error'}`);
    } else {
      // Success - refresh state to show pending → running transition
      refreshState();
      alert(`Interject sent! S${String(num).padStart(2,'0')} will restart with your message.`);
    }
  } catch (e) {
    alert(`Request failed: ${e.message}`);
  }
};
```

~40 lines

### 4. UI Flow

1. User clicks ✏️ button on running segment
2. Browser shows `prompt()` dialog for message input
3. JavaScript validates message (not empty, max length)
4. Browser shows `confirm()` dialog with message preview
5. On confirm, POST to `/api/control` with action="interject"
6. On success, `refreshState()` updates dashboard
7. User sees segment transition: running → pending → running (with new prompt)

### 5. Visual Design

**Button appearance:**
- Emoji: ✏️ (pencil, indicates editing/feedback)
- Color: Blue (matches --running color)
- Tooltip: "Interject with message"

**Dialogs:**
- First dialog (prompt): Multi-line input, shows instructions
- Second dialog (confirm): Preview of message, final confirmation

**Alternative (for future enhancement):**
- Custom modal with textarea instead of prompt()
- Character counter
- Better formatting/preview

## Exit Criteria

1. [ ] CSS added for `.act-interject` button
2. [ ] Button appears for running segments only
3. [ ] Button uses ✏️ emoji and blue color
4. [ ] `interjectSeg()` function implemented
5. [ ] Prompt dialog shows clear instructions
6. [ ] Message validation (not empty, max 2000 chars)
7. [ ] Confirm dialog with message preview
8. [ ] POST to `/api/control` with correct parameters
9. [ ] Success: Calls `refreshState()` and shows confirmation
10. [ ] Error: Shows alert with error message
11. [ ] Test: Click button, enter message, verify API call
12. [ ] No regressions: Existing buttons (skip, kill, retry) still work

## Commands

**Build:** `cargo build --workspace` (validation)

**Test (targeted):**
```bash
# Manual UI testing:
# 1. Start orchestrator with test plan
# 2. Open http://localhost:8081 in browser
# 3. Wait for segment to show 'running' status
# 4. Verify ✏️ button appears next to segment
# 5. Click button
# 6. Verify prompt dialog appears with instructions
# 7. Enter test message: "This is a test interject"
# 8. Verify confirm dialog shows message preview
# 9. Click OK
# 10. Verify alert shows success message
# 11. Verify segment status changes: running → pending → running
# 12. Check segment logs to verify message appears in prompt

# Test validation:
# - Empty message: Should show "Message cannot be empty"
# - Cancel prompt: Should do nothing
# - Cancel confirm: Should do nothing
# - Message > 2000 chars: Should show "Message too long"
```

**Test (regression):**
```bash
# Verify existing UI features still work:
# 1. Segment table renders correctly
# 2. Log viewer shows logs for selected segment
# 3. Skip button works (for pending segments)
# 4. Kill button works (for running segments)
# 5. Retry button works (for failed segments)
# 6. State refreshes every 5 seconds
# 7. No JavaScript errors in browser console
```

**Test (full gate):**
```bash
# Full integration test:
# 1. Start orchestrator on real plan
# 2. Let segment run for ~30 seconds
# 3. Click interject button
# 4. Enter: "Please add more detailed comments to the code"
# 5. Confirm
# 6. Verify segment kills and restarts
# 7. Check logs (/api/logs/S01) to see message in prompt
# 8. Verify agent responds to the feedback
# 9. Verify segment completes successfully
```

## Risk Factors

**Risk: 2/10** - Very low risk, isolated UI changes

**Potential issues:**
- JavaScript syntax error breaks dashboard (MITIGATED: test in browser first)
- Button clicks don't work (MITIGATED: follow existing `controlSeg()` pattern)
- prompt() dialog is ugly (ACCEPTED: can enhance with custom modal later)

## Pre-Mortem: What Could Go Wrong

1. **User accidentally clicks interject** → Segment killed unintentionally
   - Mitigation: Two-step process (prompt + confirm) with clear warnings
2. **Long message makes prompt dialog ugly** → Poor UX
   - Mitigation: Validate max length, preview in confirm dialog
   - Future: Custom modal with textarea
3. **Button shows for non-running segments** → Confusing UX
   - Mitigation: Status check in `_buildActBtns()`, only show for 'running'
4. **Network error during POST** → User doesn't know if interject worked
   - Mitigation: Try/catch around fetch, show error alert
5. **State refresh doesn't show pending → running transition** → User confused
   - Mitigation: Call `refreshState()` after success, alert confirms action

## Alternatives Ruled Out

- **Always-visible chat input bar:** Rejected - wastes space, pause workflow is intentional
- **Inline editing in log viewer:** Rejected - too complex for this use case
- **WebSocket for real-time updates:** Rejected - SSE already used for logs, HTTP adequate for control actions
- **Custom modal dialog:** Deferred - native prompt() is adequate for MVP, can enhance later

## Files Modified

- `scripts/orchestrate_v2/dashboard.html` (~80 lines added)

## Commit Message

```
feat(orchestrate): add interject button to dashboard

Add UI for pause-and-interject feature. Running segments now show a
pencil button that prompts for operator message, confirms action, and
sends to /api/control endpoint.

- Add .act-interject CSS styling (blue button)
- Add pencil emoji button for running segments only
- Implement interjectSeg() function with validation
- Two-step flow: prompt for message, confirm with preview
- Show success/error alerts
- Refresh state after successful interject
```
