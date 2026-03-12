---
segment: 9
title: "Config, Notify, Streamparse utility tests"
depends_on: [2]
cycle_budget: 15
risk: 3
complexity: "Medium"
commit_message: "test(orchestrate): Add config, notify, streamparse tests (80% coverage each)"
---

# Segment 9: Config, Notify, Streamparse utility tests

## Goal

Achieve 80%+ combined coverage for config.py, notify.py, streamparse.py.

## Context

Three utility modules (config: 242 lines, notify: 212 lines, streamparse: 162 lines) are completely untested. Need to verify configuration loading, notification logic, stream parsing.

## Scope

- **Create:** `test_config.py` (~100 lines)
- **Create:** `test_notify.py` (~80 lines)
- **Create:** `test_streamparse.py` (~60 lines)
- **Target coverage:** 80%+ each

## Implementation Approach

### test_config.py:

1. **Test _resolve_env_refs:**
   ```python
   def test_resolve_env_refs_with_default(monkeypatch):
       monkeypatch.delenv('MISSING_VAR', raising=False)
       result = _resolve_env_refs("Value: ${MISSING_VAR:-default}")
       assert result == "Value: default"
   ```

2. **Test RetryPolicy:**
   - Exponential backoff (jitter=False)
   - With jitter (verify bounds)
   - should_retry parametrized by status

3. **Test OrchestrateConfig loading from TOML**

### test_notify.py:

1. **Test Notifier class:**
   ```python
   @pytest.mark.asyncio
   async def test_notifier_segment_complete(mock_state_db, mocker):
       mock_client = mocker.patch('httpx.AsyncClient')
       mock_post = AsyncMock()
       mock_client.return_value.__aenter__.return_value.post = mock_post

       notifier = Notifier(config, mock_state_db)
       await notifier.segment_complete(1, "Test", "pass", "Summary")
       # Verify notification queued
   ```

2. **Test _send_ntfy:** Mock POST, verify headers/body
3. **Test notification batching, retry, deduplication**

### test_streamparse.py:

1. **Test _parse_stream_line_rich:**
   - Text delta, tool use, thinking, invalid JSON

2. **Test _extract_text_from_stream_line:**
   - Valid JSON, invalid JSON

## Pre-Mortem Risks

- **Monkeypatch env vars:** pytest isolates fixtures
  - Mitigation: Each test gets clean env
- **Mock httpx wrong:** Follow httpx docs
  - Mitigation: Use httpx testing patterns
- **Jitter tests flaky:** Random values
  - Mitigation: Run 100 iterations, check all pass

## Exit Criteria

1. **Targeted tests:** All 3 test files pass (40+ combined tests)
2. **Regression tests:** Full suite passes
3. **Full build gate:** No syntax errors
4. **Full test gate:** All tests pass
5. **Self-review gate:** All RetryPolicy methods tested
6. **Scope verification gate:** Only 3 new test files

## Commands

```bash
# Build
python -m py_compile scripts/orchestrate_v3/test_config.py \
  scripts/orchestrate_v3/test_notify.py \
  scripts/orchestrate_v3/test_streamparse.py

# Test (targeted)
pytest scripts/orchestrate_v3/test_config.py \
  scripts/orchestrate_v3/test_notify.py \
  scripts/orchestrate_v3/test_streamparse.py -v

# Test (regression)
pytest scripts/orchestrate_v3/ -v

# Test (full gate)
pytest scripts/orchestrate_v3/ \
  --cov=scripts/orchestrate_v3/{config,notify,streamparse}.py \
  --cov-report=term
```
