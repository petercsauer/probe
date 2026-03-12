---
segment: 2
title: "Test infrastructure setup (pytest + fixtures)"
depends_on: [1]
cycle_budget: 15
risk: 4
complexity: "Medium"
commit_message: "test(orchestrate): Add pytest infrastructure with async fixtures"
---

# Segment 2: Test infrastructure setup

## Goal

Establish pytest infrastructure with async support, shared fixtures, and coverage measurement.

## Context

Current tests (`test_recovery.py`, `test_worktree_pool.py`) are standalone scripts using `unittest.mock` with manual `asyncio.run()` calls. Need pytest for test discovery, shared fixtures, parametrization, and coverage measurement.

## Scope

- **Create:** `conftest.py` (~150 lines of fixtures)
- **Create:** `pytest.ini` (~15 lines of config)
- **Modify:** `requirements.txt` (+4 dependencies)
- **Convert:** `test_recovery.py`, `test_worktree_pool.py`

## Key Files and Context

- Current test framework: unittest.mock with manual asyncio.run()
- Target: pytest with pytest-asyncio, pytest-cov, pytest-mock
- Need fixtures for: temp directories, mock StateDB, default config, mock segments, mock notifier

## Implementation Approach

1. **Add dependencies to `requirements.txt`:**
   ```
   pytest>=8.0.0
   pytest-asyncio>=0.23.0
   pytest-cov>=4.1.0
   pytest-mock>=3.12.0
   ```

2. **Create `conftest.py` with fixtures:**
   - `@pytest.fixture async def temp_dir()` - TemporaryDirectory wrapper
   - `@pytest.fixture async def mock_state_db(temp_dir)` - In-memory SQLite StateDB
   - `@pytest.fixture def default_config()` - OrchestrateConfig with safe defaults
   - `@pytest.fixture def mock_segment()` - Factory for creating test Segment objects
   - `@pytest.fixture def mock_notifier()` - Captures notification calls

3. **Create `pytest.ini`:**
   ```ini
   [pytest]
   testpaths = scripts/orchestrate_v3
   python_files = test_*.py
   asyncio_mode = auto
   addopts = --cov=scripts/orchestrate_v3 --cov-report=term-missing --cov-report=html -v
   ```

4. **Convert `test_recovery.py`:**
   - Remove `if __name__ == "__main__"` block
   - Convert class methods to module-level functions
   - Add `@pytest.mark.asyncio` to async tests
   - Remove manual `asyncio.run()` calls

5. **Convert `test_worktree_pool.py`:**
   - Same conversion pattern
   - Use `temp_dir` fixture instead of hardcoded paths

6. **Verify:** Run `pytest scripts/orchestrate_v3/ -v`

## Alternatives Ruled Out

- **Keep unittest.mock:** Rejected (verbose, no fixture composability, poor async support)
- **Use tox:** Rejected (overkill for single Python version)

## Pre-Mortem Risks

- **Async fixture cleanup issues:** Teardown might not run if test crashes
  - Mitigation: pytest-asyncio handles cleanup properly
- **Import path confusion:** pytest might import wrong version
  - Mitigation: Run from repo root with `pytest scripts/orchestrate_v3/`
- **Coverage too low initially:** Only ~15-20% with existing tests
  - Mitigation: Expected, next segments add tests

## Exit Criteria

1. **Targeted tests:** pytest discovers and runs test_recovery.py (8+ tests pass)
2. **Regression tests:** Both test files pass with pytest
3. **Full build gate:** `pip install` succeeds, all dependencies resolve
4. **Full test gate:** `pytest scripts/orchestrate_v3/ -v` passes
5. **Self-review gate:** No print() in tests, all async tests have @pytest.mark.asyncio
6. **Scope verification gate:** Only orchestrate_v3 files touched, conftest.py has no business logic

## Commands

```bash
# Build
pip install -r scripts/orchestrate_v3/requirements.txt

# Test (targeted)
pytest scripts/orchestrate_v3/test_recovery.py -v

# Test (regression)
pytest scripts/orchestrate_v3/ -v

# Test (full gate)
pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3 --cov-report=term
```
