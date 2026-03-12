---
segment: 03
title: Kill-and-Interject API Endpoint
depends_on: [1, 2]
risk: 4
complexity: Low
cycle_budget: 10
estimated_lines: ~30 lines
status: pending
---

# Segment 03: Kill-and-Interject API Endpoint

## Goal

Add "interject" action to the `/api/control` endpoint that atomically kills a running process, stores the operator message, and resets the segment to pending status.

## Context

The monitor serves an HTTP API at `/api/control` for operator actions (skip, retry, kill, set_status). We need to add a new "interject" action that combines kill+store+reset in a single atomic operation.

## Current State

**Control endpoint:** `scripts/orchestrate_v2/monitor.py:57-118`

Existing actions:
- `skip` - Set status to skipped
- `retry` - Reset to pending (requires orchestrator restart)
- `kill` - Terminate process with SIGTERM
- `set_status` - Manually change status

**Kill logic:** Lines 92-101 - uses `os.killpg()` to terminate process group

**Access to state:** Monitor has reference to StateDB instance via `request.app["state"]`

**Access to PIDs:** Monitor has reference to `_running_pids` dict via `request.app["running_pids"]`

## Implementation Plan

### 1. Add "interject" Action Case

Location: `monitor.py` after line 101 (after "kill" action)

```python
elif action == "interject":
    message = data.get("message", "")
    if not message:
        return web.json_response(
            {"ok": False, "error": "message parameter required"},
            status=400
        )

    # Validate message length
    if len(message) > 2000:
        return web.json_response(
            {"ok": False, "error": "message too long (max 2000 chars)"},
            status=400
        )

    pid = pids.get(seg_num)
    if not pid:
        return web.json_response(
            {"ok": False, "error": "segment not running"},
            status=404
        )

    try:
        # Step 1: Kill the process
        os.killpg(os.getpgid(pid), signal.SIGTERM)

        # Step 2: Store the interject message
        await state.enqueue_interject(seg_num, message)

        # Step 3: Reset segment to pending
        await state.reset_for_retry(seg_num)

        # Step 4: Log compound event
        await state.log_event(
            "operator_interject",
            f"S{seg_num:02d} killed and reset with operator message: {message[:100]}{'...' if len(message) > 100 else ''}",
            severity="warn"
        )

        return web.json_response({
            "ok": True,
            "action": "interject",
            "seg_num": seg_num,
            "message_preview": message[:100]
        })
    except Exception as e:
        return web.json_response(
            {"ok": False, "error": str(e)},
            status=500
        )
```

### 2. Atomic Operation Considerations

The three steps (kill, store, reset) should happen in order:
1. **Kill first** - Prevents segment from continuing with stale state
2. **Store message** - Ensures message is persisted before reset
3. **Reset last** - Makes segment eligible for immediate re-run

If any step fails, the error propagates to the client. The segment may be in an inconsistent state (killed but not reset), but operator can use "retry" action to recover.

### 3. Error Handling

- **No message:** 400 Bad Request
- **Message too long:** 400 Bad Request (prevents prompt bloat)
- **Segment not running:** 404 Not Found
- **Process kill fails:** 500 Internal Server Error
- **Database operation fails:** 500 Internal Server Error

### 4. Response Format

Success response:
```json
{
  "ok": true,
  "action": "interject",
  "seg_num": 1,
  "message_preview": "First 100 chars of message..."
}
```

Error response:
```json
{
  "ok": false,
  "error": "message too long (max 2000 chars)"
}
```

## Exit Criteria

1. [ ] "interject" action added to control endpoint
2. [ ] Validates message parameter (required, non-empty)
3. [ ] Validates message length (max 2000 chars)
4. [ ] Returns 404 if segment not running
5. [ ] Kills process with SIGTERM
6. [ ] Stores message via `enqueue_interject()`
7. [ ] Resets segment via `reset_for_retry()`
8. [ ] Logs event with message preview
9. [ ] Returns success JSON with confirmation
10. [ ] Error handling for all failure cases

## Commands

**Build:** `cargo build --workspace` (validation)

**Test (targeted):**
```bash
# Test with curl (requires running orchestrator)
# First start orchestrator with a test plan that has at least one segment

# Test missing message
curl -X POST http://localhost:8081/api/control \
  -H 'Content-Type: application/json' \
  -d '{"action":"interject","seg_num":1}'
# Expected: {"ok": false, "error": "message parameter required"}

# Test message too long
curl -X POST http://localhost:8081/api/control \
  -H 'Content-Type: application/json' \
  -d "{\"action\":\"interject\",\"seg_num\":1,\"message\":\"$(python3 -c 'print(\"a\"*2001)')\"}"
# Expected: {"ok": false, "error": "message too long..."}

# Test not running
curl -X POST http://localhost:8081/api/control \
  -H 'Content-Type: application/json' \
  -d '{"action":"interject","seg_num":99,"message":"test"}'
# Expected: {"ok": false, "error": "segment not running"}

# Test valid interject (on running segment)
curl -X POST http://localhost:8081/api/control \
  -H 'Content-Type: application/json' \
  -d '{"action":"interject","seg_num":1,"message":"Fix the bug on line 42"}'
# Expected: {"ok": true, "action": "interject", "seg_num": 1, ...}
```

**Test (regression):**
```bash
# Verify existing control actions still work
curl -X POST http://localhost:8081/api/control \
  -H 'Content-Type: application/json' \
  -d '{"action":"skip","seg_num":1}'

# Verify state endpoint still works
curl http://localhost:8081/api/state | python3 -m json.tool
```

**Test (full gate):**
```bash
# Integration test: Full flow from interject to segment restart
# 1. Start orchestrator
# 2. Wait for segment to be running
# 3. Send interject via API
# 4. Verify segment status changes: running → pending → running
# 5. Check logs to verify message was included in prompt
# 6. Verify segment completes successfully
```

## Risk Factors

**Risk: 4/10** - Low-medium risk, combines multiple operations

**Potential issues:**
- Kill succeeds but DB write fails (ACCEPTED: operator can retry)
- Race between kill and restart (MITIGATED: reset after kill)
- Message not properly escaped in logs (HANDLED: truncate for preview)

## Pre-Mortem: What Could Go Wrong

1. **Process kill fails but returns success** → Segment continues running
   - Mitigation: os.killpg raises exception on failure, propagates to client
2. **Database write fails** → Message lost, segment in limbo
   - Mitigation: Log error, return 500, operator can use "retry" to recover
3. **Segment reset fails** → Message stored but not re-run
   - Mitigation: Error propagates to client, operator sees failure
4. **Multiple rapid interjections** → Messages queue up
   - Handled: Only latest unconsumed message is used (by prompt augmentation logic)
5. **Long message causes prompt overflow** → API rejects prompt
   - Mitigation: Validate max 2000 chars in this endpoint

## Alternatives Ruled Out

- **Separate endpoints for kill and interject:** Rejected - race condition if not atomic
- **Store message before kill:** Rejected - message might be stale if kill fails
- **WebSocket for real-time updates:** Rejected - adds complexity, HTTP adequate for this use case

## Files Modified

- `scripts/orchestrate_v2/monitor.py` (~30 lines added)

## Commit Message

```
feat(orchestrate): add interject control action for kill+message+retry

Add atomic "interject" action to /api/control endpoint that kills a
running segment, stores operator message, and resets status to pending
for immediate re-run with feedback.

- Validate message parameter (required, max 2000 chars)
- Kill process with SIGTERM
- Store message via enqueue_interject()
- Reset segment via reset_for_retry()
- Log compound event for audit trail
- Return success/error with appropriate HTTP status codes
```
