---
segment: 1
title: "Create orchestrate_v3 copy"
depends_on: []
cycle_budget: 10
risk: 2
complexity: "Low"
commit_message: "feat(orchestrate): Create orchestrate_v3 as refactored copy of v2"
---

# Segment 1: Create orchestrate_v3 copy

## Goal

Create `scripts/orchestrate_v3/` as a working copy of `orchestrate_v2/` with updated imports and verified functionality.

## Context

This segment establishes the foundation for all refactoring work by creating a separate v3 directory. This allows us to iterate safely while keeping v2 stable.

## Scope

- **New directory:** `scripts/orchestrate_v3/`
- **Update:** All internal imports
- **Source:** `scripts/orchestrate_v2/` (13 Python files, 1 HTML file, 1 requirements.txt, 2 test files)

## Key Files and Context

- Source: `scripts/orchestrate_v2/` contains all modules
- All internal imports use relative imports: `from .module import X`
- Python path changes to `scripts.orchestrate_v3.module`
- Entry point: `python -m scripts.orchestrate_v3 <plan_dir>`

## Implementation Approach

1. Copy entire `orchestrate_v2/` directory to `orchestrate_v3/`
2. Update `__main__.py` module docstring to indicate v3
3. Verify no hardcoded v2 paths in code (grep for "orchestrate_v2")
4. Test imports: `python -c "from scripts.orchestrate_v3 import runner, state, monitor"`
5. Smoke test: Run `python -m scripts.orchestrate_v3 --help` (should print usage)
6. Document in `orchestrate_v3/README.md`: "Refactored version of orchestrate_v2 with improved code quality, 70% test coverage, and modular architecture"

## Alternatives Ruled Out

- **In-place refactoring of v2:** Rejected (user requested separate v3 copy for safety)
- **Git branch instead of directory:** Rejected (harder to compare v2 vs v3 side-by-side)

## Pre-Mortem Risks

- **Absolute imports break:** Some code might use `scripts.orchestrate_v2` absolute imports
  - Mitigation: Grep for all imports, convert to relative or update to v3
- **Circular imports surface:** Copy might reveal hidden import cycles
  - Mitigation: Python will error immediately, fix import order

## Exit Criteria

1. **Targeted tests:** Import all modules successfully (no ImportError)
2. **Regression tests:** Existing test scripts run and pass
3. **Full build gate:** `python -m py_compile scripts/orchestrate_v3/*.py` (no syntax errors)
4. **Full test gate:** `python -m scripts.orchestrate_v3 --help` prints usage without crashing
5. **Self-review gate:** No absolute `orchestrate_v2` imports remain, no orphaned files
6. **Scope verification gate:** Only files in `scripts/orchestrate_v3/` modified

## Commands

```bash
# Build
python -m py_compile scripts/orchestrate_v3/*.py

# Test (targeted)
python -c "from scripts.orchestrate_v3 import runner, state, monitor, planner, config"

# Test (regression)
python scripts/orchestrate_v3/test_recovery.py && python scripts/orchestrate_v3/test_worktree_pool.py

# Test (full gate)
python -m scripts.orchestrate_v3 --help
```
