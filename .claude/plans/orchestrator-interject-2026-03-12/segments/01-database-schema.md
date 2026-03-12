---
segment: 01
title: Database Schema for Interjections
depends_on: []
risk: 2
complexity: Low
cycle_budget: 10
estimated_lines: ~55 lines
status: pending
---

# Segment 01: Database Schema for Interjections

## Goal

Add database table to store pending interject messages with proper schema, migrations, and StateDB methods for querying and consuming messages.

## Context

The orchestrator uses SQLite via aiosqlite for state management. Current schema has tables for segments, events, run_meta, notifications, and segment_attempts. We need to add a new table to store operator interject messages that will be consumed when segments restart.

## Current State

**Database schema location:** `scripts/orchestrate_v2/state.py:13-63`
- Existing tables: segments, events, run_meta, notifications, segment_attempts
- Migrations list: `state.py:65-70`
- StateDB class: `state.py:100+`

**No existing interject storage** - this is net-new functionality.

## Implementation Plan

### 1. Add Schema for segment_interjections Table

Location: `state.py` in `_SCHEMA` constant (after line 50)

```python
CREATE TABLE IF NOT EXISTS segment_interjections (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    seg_num         INTEGER NOT NULL,
    created_at      REAL NOT NULL,
    message         TEXT NOT NULL,
    consumed_at     REAL,
    attempt_num     INTEGER,
    FOREIGN KEY (seg_num) REFERENCES segments(num)
);
```

**Columns:**
- `id`: Auto-incrementing primary key
- `seg_num`: Foreign key to segments table
- `created_at`: Timestamp when message was created
- `message`: The operator's message text
- `consumed_at`: Timestamp when message was used (NULL if pending)
- `attempt_num`: Which attempt this was for (tracking/audit)

### 2. Add StateDB Methods

Add four new async methods to StateDB class (after existing methods):

**`enqueue_interject(seg_num: int, message: str) -> int`**
- Insert new row with current timestamp
- Return the inserted id
- ~12 lines

**`get_pending_interject(seg_num: int) -> dict | None`**
- Query for unconsumed message (consumed_at IS NULL)
- Order by created_at DESC (latest first)
- Return dict with id, seg_num, created_at, message
- Return None if no pending message
- ~10 lines

**`consume_interject(interject_id: int) -> None`**
- Update consumed_at to current timestamp
- ~8 lines

**`get_interject_history(seg_num: int, limit: int = 10) -> list[dict]`**
- Query all interjections for segment (consumed or not)
- Order by created_at DESC
- Limit to most recent N
- Return list of dicts
- ~10 lines

### 3. Migration Entry (if needed)

If deploying to existing databases, add to `_MIGRATIONS` list:
```python
"CREATE TABLE IF NOT EXISTS segment_interjections (...)"
```

Only needed if existing state.db files need upgrade. For new databases, schema creates all tables.

## Exit Criteria

1. [ ] `segment_interjections` table added to `_SCHEMA` in state.py
2. [ ] `enqueue_interject()` method implemented and working
3. [ ] `get_pending_interject()` method returns correct message or None
4. [ ] `consume_interject()` method marks message as consumed
5. [ ] `get_interject_history()` method returns audit trail
6. [ ] Foreign key constraint to segments table works
7. [ ] Test: Create database from scratch, verify table exists
8. [ ] Test: Insert message via `enqueue_interject()`, retrieve via `get_pending_interject()`
9. [ ] Test: Consume message, verify `consumed_at` is set
10. [ ] No breaking changes to existing schema or methods

## Commands

**Build:** `cargo build --workspace` (validation - Python changes don't affect Rust)

**Test (targeted):**
```python
python3 -c "
import asyncio
from scripts.orchestrate_v2.state import StateDB

async def test():
    db = await StateDB.create(':memory:')
    # Test enqueue
    msg_id = await db.enqueue_interject(1, 'Test message')
    print(f'Created interject: {msg_id}')

    # Test get_pending
    pending = await db.get_pending_interject(1)
    assert pending is not None
    assert pending['message'] == 'Test message'
    print(f'Retrieved pending: {pending}')

    # Test consume
    await db.consume_interject(msg_id)
    pending_after = await db.get_pending_interject(1)
    assert pending_after is None
    print('Consumed successfully')

    # Test history
    history = await db.get_interject_history(1)
    assert len(history) == 1
    print(f'History: {history}')

    await db.close()
    print('All tests passed!')

asyncio.run(test())
"
```

**Test (regression):**
```bash
# Start orchestrator and verify state.db initializes
python3.11 -m scripts.orchestrate_v2 run --dry-run /Users/psauer/probe/.claude/plans/orchestrator-interject-2026-03-12
# Check if tables were created
sqlite3 /Users/psauer/probe/.claude/plans/orchestrator-interject-2026-03-12/state.db ".schema segment_interjections"
```

**Test (full gate):** All existing orchestrator tests still pass

## Risk Factors

**Risk: 2/10** - Low risk, isolated database change

**Potential issues:**
- Schema syntax error (MITIGATED: test with :memory: DB first)
- Foreign key constraint fails (MITIGATED: segments never deleted in practice)
- Migration on existing DB fails (HANDLED: CREATE IF NOT EXISTS)

## Pre-Mortem: What Could Go Wrong

1. **Typo in SQL syntax** → Database creation fails
   - Mitigation: Test with in-memory DB first, run schema validation
2. **Column name conflicts** → Query errors
   - Mitigation: Follow existing naming conventions (snake_case, descriptive)
3. **Migration breaks existing state.db** → Orchestrator won't start
   - Mitigation: Test migration on copy of real state.db file first

## Alternatives Ruled Out

- **Single column on segments table:** Rejected - loses history, only one interject at a time, pollutes segments schema
- **File-based storage:** Rejected - race conditions, harder to query, no transactional guarantees
- **JSON column for message history:** Rejected - harder to query, no indexing, violates normalization

## Files Modified

- `scripts/orchestrate_v2/state.py` (~55 lines added)

## Commit Message

```
feat(orchestrate): add segment_interjections table for operator messages

Add database schema and StateDB methods to support pause-and-interject
feature. New table stores operator messages that are injected into
segment prompts on restart.

- Add segment_interjections table with foreign key to segments
- Implement enqueue_interject() to store new messages
- Implement get_pending_interject() to retrieve unconsumed messages
- Implement consume_interject() to mark messages as used
- Implement get_interject_history() for audit trail
```
