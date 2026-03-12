# Orchestrator Interject Feature - Pause and Interject Approach

**Plan Date:** 2026-03-12
**Approach:** Pause-and-Interject (Recommended over Real-Time Stdin Injection)
**Total Segments:** 4
**Estimated Time:** 2-3 hours
**Risk Budget:** 27.5% (11/40 points)

---

## Goal

Add capability for users to interject messages to running orchestrator segments via the dashboard. When a segment is running, users can pause it, provide feedback or corrections, and the segment will restart with the message appended to its prompt.

---

## Approach Overview

**Pause-and-Interject Pattern:**
1. User clicks "Interject" button on running segment in dashboard
2. System kills the running process gracefully
3. User message stored in database (segment_interjections table)
4. Segment status reset to "pending"
5. Orchestrator detects pending status and re-runs segment
6. Prompt builder appends stored message before "Begin now."
7. Agent receives operator feedback and adjusts approach

**Why this approach:**
- Lower risk (27.5% vs 40% for real-time stdin)
- Faster implementation (2-3h vs 4-6h)
- Simpler failure recovery (just restart vs async coordination)
- Better fit for use case (segments run 10-30 min, interjections are rare deliberate corrections)

---

## Dependency Graph

```
B1 (Database Schema) ──> B2 (Prompt Augmentation) ──> B3 (API Endpoint) ──> B4 (Dashboard UI)
```

All segments are sequential - each depends on the previous.

---

## Segment Index

| # | Slug | Title | Status | Risk | Complexity | LOC | Notes |
|---|------|-------|--------|------|------------|-----|-------|
| 1 | database-schema | Database Schema for Interjections | ✅ complete | 2/10 | Low | 64 | Commit: 1f73c65 |
| 2 | prompt-augmentation | Prompt Augmentation Logic | in-progress | 3/10 | Low | 35 | Core logic |
| 3 | api-endpoint | Kill-and-Interject API Endpoint | pending | 4/10 | Low | 30 | Integration |
| 4 | dashboard-ui | Dashboard Interject UI | pending | 2/10 | Low | 80 | User interface |

**Total Estimated Lines:** ~244 lines across 4 files

---

## Execution Order

1. B1 - Database Schema (foundation, no dependencies)
2. B2 - Prompt Augmentation (depends on B1 schema)
3. B3 - API Endpoint (depends on B1, B2)
4. B4 - Dashboard UI (depends on B3 API)

No parallelization possible - linear dependency chain.

---

## Exit Criteria (Plan-Level)

After all segments complete:

1. [ ] Database has segment_interjections table with proper schema
2. [ ] Prompts include pending interject messages before "Begin now."
3. [ ] POST /api/control with action="interject" kills process and stores message
4. [ ] Dashboard shows "Interject" button for running segments only
5. [ ] Full flow works: Click interject → Enter message → Segment restarts → Message in prompt
6. [ ] No regressions: Existing orchestrator functionality still works
7. [ ] All segments committed with proper commit messages

---

## Testing Strategy

**Per-segment:** Each segment has specific test commands in its brief

**Integration testing:**
- Start orchestrator on test plan
- Wait for segment to be running
- Click "Interject" button
- Enter test message
- Verify segment killed, reset to pending, restarted
- Check logs show message was included in prompt

**Regression testing:**
- Existing orchestrator features work (start, monitor, kill, skip, retry)
- Database upgrades cleanly on existing state.db files
- Dashboard UI still functional (log viewer, segment selection, control buttons)

---

## Risk Mitigation

1. **Database migration:** Test on copy of existing state.db before deploying
2. **Message validation:** Limit to 2000 chars to prevent prompt bloat
3. **Atomic action:** Kill+store+reset in single control endpoint to prevent races
4. **UI confirmation:** Require explicit confirm before killing segment
5. **Audit trail:** Log all interject events with timestamps

---

## Known Limitations

1. **Lost work:** Killing segment loses current work-in-progress (not suitable if 95% done)
2. **No authentication:** API endpoints have same security posture as existing /api/control
3. **Single message:** Only latest unconsumed interject is used (multiple queued interjections not supported)
4. **Restart delay:** ~5-10 seconds for process kill → restart cycle

---

## Future Enhancements (Out of Scope)

- Real-time stdin injection for chat-like UX (Approach A)
- Message history viewer in dashboard
- Scheduled interjections (run at specific time or condition)
- Multi-turn conversation threading
- Rich text formatting in messages

---

## References

Research artifacts from deep-plan:
- `/Users/psauer/probe/subprocess_stdin_research.md` - Python stdin patterns (for future Approach A)
- `/Users/psauer/probe/.claude/research/chat-ui-ux-patterns-2026-03-12.md` - UI/UX research

Codebase files modified:
- `scripts/orchestrate_v2/state.py` - Database schema and methods
- `scripts/orchestrate_v2/runner.py` - Prompt construction
- `scripts/orchestrate_v2/monitor.py` - API endpoint
- `scripts/orchestrate_v2/dashboard.html` - UI components
