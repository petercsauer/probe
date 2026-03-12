---
segment: 3
title: "FileOps utility abstraction"
depends_on: [2]
cycle_budget: 15
risk: 3
complexity: "Medium"
commit_message: "refactor(orchestrate): Add FileOps utility for atomic file operations"
---

# Segment 3: FileOps utility abstraction

## Goal

Create centralized file operations utility with atomic writes and consistent error handling, replace usage in runner.py and monitor.py.

## Context

File I/O operations scattered across 8 modules with inconsistent error handling. 44 occurrences of direct `path.write_text()`, `path.read_text()`, `path.unlink()` with no error wrapping. TOCTOU bugs from `if path.exists()` checks before operations.

## Scope

- **Create:** `fileops.py` (~150 lines: FileOps class + FileOpsError)
- **Modify:** `runner.py` (7 file operation call sites)
- **Modify:** `monitor.py` (8 file operation call sites)
- **Create:** `test_fileops.py` (~150 lines)

## Implementation Approach

1. **Create `fileops.py`:**
   ```python
   class FileOpsError(Exception):
       """File operation failed."""
       pass

   class FileOps:
       @staticmethod
       def read_text(path: Path, encoding: str = "utf-8",
                     errors: str = "replace") -> str:
           # Wrap with FileOpsError

       @staticmethod
       def write_text_atomic(path: Path, content: str,
                            encoding: str = "utf-8") -> None:
           # temp file + os.replace()

       @staticmethod
       def remove_safe(path: Path) -> bool:
           # Returns bool, handles FileNotFoundError

       @staticmethod
       def ensure_dir(path: Path) -> None:
           # mkdir with parents=True, exist_ok=True
   ```

2. **Replace in `runner.py`:**
   - Line 374: `prompt_file.write_text(...)` → `FileOps.write_text_atomic(...)`
   - Lines 378-379: raw_log and human_log → FileOps equivalents
   - Archive operations: Use FileOps for atomic rename

3. **Replace in `monitor.py`:**
   - Line 49: dashboard.html read → `FileOps.read_text(...)`
   - Prompt loading: Use FileOps.read_text with try/except
   - Log file checks: Replace `path.exists()` with try/except

4. **Write `test_fileops.py`:**
   - Test atomic write (verify .tmp file created then replaced)
   - Test error handling (permission denied, file not found)
   - Test encoding edge cases
   - Test safe remove

## Pre-Mortem Risks

- **Atomic writes on network filesystems:** `os.replace()` may not be atomic on NFS
  - Mitigation: Orchestrator runs locally
- **Error wrapping hides stack traces:** `raise ... from e` preserves chain
  - Mitigation: Document in FileOpsError docstring

## Exit Criteria

1. **Targeted tests:** test_fileops.py passes (10+ test cases)
2. **Regression tests:** Existing tests still pass
3. **Full build gate:** No syntax errors
4. **Full test gate:** All tests pass
5. **Self-review gate:** All file ops converted (grep for `.write_text`, `.read_text`, `.unlink`)
6. **Scope verification gate:** Only fileops.py, test_fileops.py, runner.py, monitor.py modified

## Commands

```bash
# Build
python -m py_compile scripts/orchestrate_v3/fileops.py

# Test (targeted)
pytest scripts/orchestrate_v3/test_fileops.py -v

# Test (regression)
pytest scripts/orchestrate_v3/ -v

# Test (full gate)
pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3/fileops.py --cov-report=term
```
