---
segment: 02
title: Prompt Augmentation Logic
depends_on: [1]
risk: 3
complexity: Low
cycle_budget: 10
estimated_lines: ~35 lines
status: pending
---

# Segment 02: Prompt Augmentation Logic

## Goal

Modify the prompt builder to query for pending interject messages and append them to segment prompts before "Begin now.", enabling operators to inject feedback into restarted segments.

## Context

The runner constructs prompts using `_build_prompt()` function which assembles text from preamble files, segment files, and extra rules. We need to add a step that queries the database for pending interjections and includes them in the prompt with clear visual markers.

## Current State

**Prompt construction:** `scripts/orchestrate_v2/runner.py:86-110`

Current structure:
1. System role ("You are an iterative-builder...")
2. Preamble file references
3. Segment file reference
4. Extra rules (with template interpolation)
5. "Begin now."

**Call site:** `runner.py:349` - `prompt = _build_prompt(seg, config)`

**No interject querying currently** - prompts are static once built.

## Implementation Plan

### 1. Update _build_prompt() Signature

Add optional parameter: `interject: str | None = None`

```python
def _build_prompt(
    seg: "Segment",
    config: OrchestrateConfig,
    interject: str | None = None,
) -> str:
```

Location: `runner.py:86`

### 2. Add Interject Section to Prompt

After extra_rules section, before "Begin now.":

```python
if interject:
    parts.append("\n" + "="*60)
    parts.append("⚠️  OPERATOR INTERJECT MESSAGE")
    parts.append("="*60)
    parts.append(interject)
    parts.append("")
    parts.append("Address this feedback and continue the segment.")
    parts.append("="*60 + "\n")

parts.append("Begin now.")
```

Location: After `runner.py:107`, before line 110

**Visual markers:**
- Clear separator lines (=== bars)
- ⚠️ emoji for visual distinction
- Instruction to address feedback
- Extra newline for spacing

### 3. Query for Pending Interject in run_segment()

Before prompt construction, query database:

```python
# Check for pending operator interject
pending_interject = await state.get_pending_interject(seg.num)
interject_msg = pending_interject["message"] if pending_interject else None
interject_id = pending_interject["id"] if pending_interject else None
```

Location: `runner.py:~345` (before prompt construction on line 349)

### 4. Pass Interject to _build_prompt()

Update call site:

```python
prompt = _build_prompt(seg, config, interject=interject_msg)
```

Location: `runner.py:349`

### 5. Consume Interject After Successful Launch

After process spawn succeeds (after line 391):

```python
# Mark interject as consumed
if interject_id:
    await state.consume_interject(interject_id)
    await state.log_event(
        "interject_consumed",
        f"S{seg.num:02d} restarted with operator message",
        severity="info"
    )
```

### 6. Log Event for Audit Trail

Whenever an interject is consumed, log it to events table for debugging and audit purposes.

## Exit Criteria

1. [ ] `_build_prompt()` accepts optional `interject` parameter
2. [ ] Interject message appears in prompt with clear visual markers
3. [ ] Query for pending interject before building prompt in `run_segment()`
4. [ ] Pass interject message to `_build_prompt()` call
5. [ ] Consume interject after successful process spawn
6. [ ] Log event when interject is consumed
7. [ ] Test: Build prompt with interject, verify it appears before "Begin now."
8. [ ] Test: Build prompt without interject, verify no changes
9. [ ] No breaking changes: Existing prompts still work

## Commands

**Build:** `cargo build --workspace` (validation)

**Test (targeted):**
```python
python3 -c "
from scripts.orchestrate_v2.runner import _build_prompt
from scripts.orchestrate_v2.planner import Segment
from scripts.orchestrate_v2.config import OrchestrateConfig
from pathlib import Path

# Create minimal test objects
seg = Segment(num=1, slug='test', title='Test', wave=1, depends_on=[])
config = OrchestrateConfig(
    plan_name='test',
    plan_goal='test',
    workspace_root=Path('/tmp'),
    plan_root=Path('/tmp'),
    preamble=[],
    max_concurrent=1
)

# Test without interject
prompt_plain = _build_prompt(seg, config)
assert 'OPERATOR INTERJECT' not in prompt_plain
print('✓ Plain prompt works')

# Test with interject
prompt_with_msg = _build_prompt(seg, config, interject='Fix the typo on line 42')
assert 'OPERATOR INTERJECT MESSAGE' in prompt_with_msg
assert 'Fix the typo on line 42' in prompt_with_msg
assert prompt_with_msg.index('OPERATOR INTERJECT') < prompt_with_msg.index('Begin now.')
print('✓ Interject appears in correct location')

print('All tests passed!')
"
```

**Test (regression):**
```bash
# Start orchestrator, verify segments still spawn
python3.11 -m scripts.orchestrate_v2 run --dry-run /Users/psauer/probe/.claude/plans/orchestrator-interject-2026-03-12
```

**Test (full gate):**
```bash
# Integration test - store interject, run segment, verify consumed
python3 -c "
import asyncio
from pathlib import Path
from scripts.orchestrate_v2.state import StateDB
from scripts.orchestrate_v2.planner import Segment, load_plan

async def test():
    db_path = Path('/tmp/test_interject.db')
    db_path.unlink(missing_ok=True)

    db = await StateDB.create(db_path)

    # Insert test segment
    seg = Segment(num=1, slug='test', title='Test', wave=1, depends_on=[])
    await db.init_segment(seg)

    # Store interject
    msg_id = await db.enqueue_interject(1, 'Test operator message')
    print(f'Stored interject: {msg_id}')

    # Verify pending
    pending = await db.get_pending_interject(1)
    assert pending is not None
    print(f'Pending interject: {pending[\"message\"]}')

    # Simulate consumption (would happen in run_segment)
    await db.consume_interject(msg_id)

    # Verify consumed
    pending_after = await db.get_pending_interject(1)
    assert pending_after is None
    print('✓ Interject consumed correctly')

    # Check history
    history = await db.get_interject_history(1)
    assert len(history) == 1
    assert history[0]['consumed_at'] is not None
    print('✓ History tracking works')

    await db.close()
    db_path.unlink()
    print('All integration tests passed!')

asyncio.run(test())
"
```

## Risk Factors

**Risk: 3/10** - Low-medium risk, modifies prompt construction

**Potential issues:**
- Prompt too long with large messages (MITIGATED: API validates max length)
- Message placement disrupts agent flow (TESTABLE: manual review)
- Database query fails (HANDLED: None fallback, logs error)

## Pre-Mortem: What Could Go Wrong

1. **Interject appears in wrong location** → Agent confused
   - Mitigation: Clear visual markers, placement just before "Begin now."
2. **Query fails/times out** → Process spawn hangs
   - Mitigation: Try/except around query, log error, proceed without interject
3. **Message not consumed** → Same interject used repeatedly
   - Mitigation: Consume immediately after successful spawn, not after completion
4. **Prompt exceeds token limit** → API rejects prompt
   - Mitigation: Validate message length in API endpoint (next segment)

## Alternatives Ruled Out

- **Prefix message before preamble:** Rejected - operator feedback should be after context
- **System message instead of user message:** Rejected - operator is external, not system
- **Inline in segment file:** Rejected - dynamic content shouldn't modify static files

## Files Modified

- `scripts/orchestrate_v2/runner.py` (~35 lines modified/added)

## Commit Message

```
feat(orchestrate): augment prompts with operator interject messages

Modify _build_prompt() to query for pending operator messages and
include them in segment prompts. Messages appear after all context but
before "Begin now." with clear visual markers.

- Add optional interject parameter to _build_prompt()
- Query pending interject in run_segment()
- Insert message with visual separators and instruction
- Consume interject after successful process spawn
- Log event for audit trail
```
