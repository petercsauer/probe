---
segment: 5
title: "State.py comprehensive tests (Priority 1)"
depends_on: [2]
cycle_budget: 18
risk: 5
complexity: "High"
commit_message: "test(orchestrate): Add comprehensive StateDB tests (90% coverage)"
---

# Segment 5: State.py comprehensive tests

## Goal

Achieve 90%+ test coverage for state.py (all StateDB methods, migrations, concurrent access).

## Context

state.py (569 lines, 17 async methods) is completely untested. Critical database operations need verification: segments CRUD, events log, attempts tracking, notifications outbox, interjections, gate attempts.

## Scope

- **Create:** `test_state.py` (~300 lines)
- **Target coverage:** 90%+ for state.py
- **Current coverage:** 0%

## Implementation Approach

1. **Test StateDB lifecycle:**
   - `test_create_initializes_schema()` - Verify tables created
   - `test_create_applies_migrations()` - Verify migration list applied
   - `test_close_cleans_up()` - Verify connection closed

2. **Test segment operations:**
   - `test_init_segments_inserts_all()`
   - `test_get_segment_returns_row()`
   - `test_set_status_updates_row()`
   - `test_increment_attempts_increments()`
   - `test_reset_stale_running_resets_only_running()`
   - Parametrize status values: pass, failed, timeout, blocked, etc.

3. **Test events log:**
   - `test_log_event_inserts()` - Verify INSERT with timestamp
   - `test_get_events_limits()` - Verify LIMIT clause
   - `test_get_events_after_id_filters()` - Verify WHERE id > ?
   - Test severity levels: info, warn, error

4. **Test attempts tracking:**
   - `test_record_attempt_inserts()`
   - `test_get_attempts_returns_ordered()` - Verify ORDER BY
   - Test token counts stored correctly

5. **Test notifications outbox, interjections, migrations**

6. **Test concurrent access:**
   - Launch 4 async tasks writing simultaneously
   - Use `asyncio.gather()` to parallelize

7. **Use fixtures:** mock_state_db for most tests, temp_dir for migrations

## Pre-Mortem Risks

- **Async timing issues:** SQLite WAL mode handles concurrency
  - Mitigation: Use asyncio.gather properly
- **Schema changes break tests:** Tests focus on behavior not schema details
  - Mitigation: Update tests with migrations

## Exit Criteria

1. **Targeted tests:** test_state.py passes (30+ tests)
2. **Regression tests:** All existing tests pass
3. **Full build gate:** No syntax errors
4. **Full test gate:** All tests pass
5. **Self-review gate:** All 17 StateDB methods tested, concurrent access tested
6. **Scope verification gate:** Only test_state.py created, state.py unchanged

## Commands

```bash
# Build
python -m py_compile scripts/orchestrate_v3/test_state.py

# Test (targeted)
pytest scripts/orchestrate_v3/test_state.py -v

# Test (regression)
pytest scripts/orchestrate_v3/ -v

# Test (full gate)
pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3/state.py --cov-report=term
```
