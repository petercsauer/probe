---
segment: 6
title: "Runner.py comprehensive tests (Priority 1)"
depends_on: [2, 3]
cycle_budget: 22
risk: 6
complexity: "High"
commit_message: "test(orchestrate): Add comprehensive runner.py tests (85% coverage)"
---

# Segment 6: Runner.py comprehensive tests

## Goal

Achieve 85%+ test coverage for runner.py (run_segment, circuit breaker, prompt building, heartbeat, log archival, interject integration).

## Context

runner.py (583 lines) is completely untested. Critical logic needs verification: subprocess execution, timeout handling, circuit breaker patterns, heartbeat updates, log archival on retry, interject message integration.

## Scope

- **Create:** `test_runner.py` (~400 lines)
- **Target coverage:** 85%+ for runner.py
- **Dependencies:** Needs S3 FileOps for mocking

## Implementation Approach

1. **Test CircuitBreaker:**
   - Parametrize all PERMANENT_PATTERNS (nested_session, missing_file, permission_denied, etc.)
   - Test retryable errors pass through
   - Test add_pattern extensibility

2. **Test helper functions:**
   - `test_resolve_isolation_env()` - Test template expansion
   - `test_build_prompt()` - Test with/without interject, preambles
   - `test_build_env()` - Test auth_env, isolation_env merge

3. **Test run_segment (mock subprocess):**
   ```python
   @pytest.mark.asyncio
   async def test_run_segment_success(mock_state_db, default_config,
                                       mock_segment, tmp_path, mocker):
       # Mock asyncio.create_subprocess_exec
       mock_proc = AsyncMock()
       mock_proc.returncode = 0
       mock_proc.pid = 12345
       mocker.patch('asyncio.create_subprocess_exec', return_value=mock_proc)

       # Mock FileOps
       mocker.patch('scripts.orchestrate_v3.runner.FileOps.write_text_atomic')

       status, summary = await run_segment(...)
       assert status == "pass"
   ```

4. **Test timeout handling:**
   - Mock asyncio.wait_for timeout
   - Verify SIGTERM sent to process

5. **Test heartbeat updates:**
   - Mock file reading
   - Verify DB writes
   - Test stall detection

6. **Test log archival:**
   ```python
   @pytest.mark.asyncio
   async def test_log_archival_on_second_attempt(tmp_path):
       # First attempt creates standard logs
       # Second attempt should rename them to -attempt1.log
       log_dir = tmp_path / "logs"
       log_dir.mkdir()

       # Simulate first attempt
       (log_dir / "S05.log").write_text("attempt 1 log")
       (log_dir / "S05.stream.jsonl").write_text("attempt 1 stream")

       # Run with attempt_num=2
       # Verify files renamed to S05-attempt1.log
       assert (log_dir / "S05-attempt1.log").exists()
       assert (log_dir / "S05-attempt1.stream.jsonl").exists()
   ```
   - Test archival on attempt_num > 1
   - Verify no archival on first attempt
   - Test correct file naming: S{NN}-attempt{N}.log

7. **Test interject integration:**
   ```python
   @pytest.mark.asyncio
   async def test_get_pending_interject_integration(mock_state_db):
       # Mock state.get_pending_interject() returning a message
       mock_state_db.get_pending_interject = AsyncMock(
           return_value={"id": 1, "message": "Fix the import error"}
       )

       # Run segment
       # Verify interject message added to prompt
       # Verify state.consume_interject() called
   ```

8. **Test token extraction from stream.jsonl**

## Pre-Mortem Risks

- **Mock complexity:** Subprocess mocking is intricate
  - Mitigation: Use pytest-mock's AsyncMock
- **Flaky timeout tests:** Timing-sensitive
  - Mitigation: Mock time.time()

## Exit Criteria

1. **Targeted tests:** test_runner.py passes (45+ tests including archival and interject)
2. **Regression tests:** All existing tests pass
3. **Full build gate:** No syntax errors
4. **Full test gate:** All tests pass
5. **Self-review gate:** All major functions tested, log archival verified, interject flow tested
6. **Scope verification gate:** Only test_runner.py created

## Commands

```bash
# Build
python -m py_compile scripts/orchestrate_v3/test_runner.py

# Test (targeted)
pytest scripts/orchestrate_v3/test_runner.py -v

# Test (regression)
pytest scripts/orchestrate_v3/ -v

# Test (full gate)
pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3/runner.py --cov-report=term
```
