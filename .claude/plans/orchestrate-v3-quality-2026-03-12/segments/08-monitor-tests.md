---
segment: 8
title: "Monitor.py comprehensive tests (Priority 2)"
depends_on: [2, 4]
cycle_budget: 20
risk: 5
complexity: "High"
commit_message: "test(orchestrate): Add comprehensive monitor.py tests (80% coverage)"
---

# Segment 8: Monitor.py comprehensive tests

## Goal

Achieve 80%+ test coverage for monitor.py (HTTP endpoints, SSE streaming, control API, multi-tab log endpoints).

## Context

monitor.py (528 lines, 10 HTTP endpoints) is completely untested. Need to verify HTTP routing, SSE streaming, control API, static file serving, and new multi-tab log endpoints.

## Scope

- **Create:** `test_monitor.py` (~300 lines)
- **Target coverage:** 80%+
- **Dependencies:** Needs S4 CSS static endpoint

## Implementation Approach

1. **Setup aiohttp test client fixture:**
   ```python
   @pytest.fixture
   async def test_client(mock_state_db, tmp_path):
       from scripts.orchestrate_v3.monitor import _create_app
       app = _create_app(mock_state_db, tmp_path, plan_root=tmp_path,
                        running_pids={})
       async with aiohttp.test_utils.TestClient(app) as client:
           yield client
   ```

2. **Test GET endpoints:**
   - `test_handle_dashboard_returns_html()`
   - `test_handle_state_returns_json()`
   - `test_handle_prompt_returns_markdown()` - Create mock segment file
   - `test_handle_prompt_404_when_missing()`
   - `test_handle_static_returns_css()`
   - `test_handle_static_404_for_unknown_file()`

3. **Test POST /api/control:**
   ```python
   async def test_control_skip(test_client, mock_state_db):
       resp = await test_client.post('/api/control', json={
           'action': 'skip', 'seg_num': 5
       })
       assert resp.status == 200
       data = await resp.json()
       assert data['ok'] is True

       seg = await mock_state_db.get_segment(5)
       assert seg.status == "skipped"
   ```
   - Test skip, retry, kill actions
   - Test invalid JSON (400), invalid seg_num (400), kill non-running (404)

4. **Test SSE endpoints:**
   - `/api/events` streams events
   - `/api/logs/{seg_id}` streams logs
   - `/api/logs/{seg_id}/attempt/{N}` streams archived
   - Mock log files in tmp_path

5. **Test new endpoints (multi-tab feature):**
   ```python
   async def test_handle_segment_attempts(test_client, mock_state_db):
       # Mock state.get_attempts() returning 2 attempts
       mock_state_db.get_attempts = AsyncMock(return_value=[
           {"attempt": 1, "status": "failed", "tokens_in": 1000},
           {"attempt": 2, "status": "pass", "tokens_in": 1200}
       ])

       resp = await test_client.get('/api/segment/S05/attempts')
       assert resp.status == 200
       data = await resp.json()
       assert data['seg_num'] == 5
       assert len(data['attempts']) == 2
       assert data['attempts'][0]['has_log'] in (True, False)

   async def test_handle_archived_log_sse(test_client, tmp_path):
       # Create archived log file
       log_file = tmp_path / "S05-attempt1.log"
       log_file.write_text("Archived log content\nLine 2")

       resp = await test_client.get('/api/logs/S05/attempt/1')
       assert resp.status == 200
       assert resp.content_type == 'text/event-stream'
       # Verify SSE stream contains log content

   async def test_handle_segment_summary(test_client, mock_state_db):
       # Mock segment and attempts data
       resp = await test_client.get('/api/segment/S05/summary')
       assert resp.status == 200
       data = await resp.json()
       assert 'total_attempts' in data
       assert 'total_tokens_in' in data
       assert 'duration_seconds' in data
   ```

6. **Test error handling:** 404, 400, 500 (mock state errors)
   - Test `/api/logs/{seg_id}/attempt/{N}` with missing log file
   - Test `/api/segment/{seg_id}/attempts` with invalid seg_id format
   - Test `/api/segment/{seg_id}/summary` with nonexistent segment

## Pre-Mortem Risks

- **SSE streaming hard to test:** Reading from async iterator
  - Mitigation: Use aiohttp test client's streaming support
- **Temp file cleanup:** Mock log files might leak
  - Mitigation: tmp_path fixture auto-cleanup

## Exit Criteria

1. **Targeted tests:** test_monitor.py passes (35+ tests including multi-tab endpoints)
2. **Regression tests:** All tests pass
3. **Full build gate:** No syntax errors
4. **Full test gate:** All tests pass
5. **Self-review gate:** All HTTP methods tested, all 10 endpoints covered, error codes verified
6. **Scope verification gate:** Only test_monitor.py created

## Commands

```bash
# Build
python -m py_compile scripts/orchestrate_v3/test_monitor.py

# Test (targeted)
pytest scripts/orchestrate_v3/test_monitor.py -v

# Test (regression)
pytest scripts/orchestrate_v3/ -v

# Test (full gate)
pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3/monitor.py --cov-report=term
```
