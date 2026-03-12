---
plan: "Orchestrate v3 — Quality Improvement & Test Coverage"
goal: "Refactor orchestrator_v2 to production-grade quality: achieve 70% test coverage, decompose god module into testable OOP classes, extract duplicated file operations, unify frontend CSS"
---

# Orchestrate v3 — Quality Improvement & Test Coverage

**Generated:** 2026-03-12

**Delivery Strategy:** Confidence-first - build test infrastructure and 70% coverage BEFORE tackling high-risk architectural refactoring.

## Overview

This plan refactors `orchestrate_v2` into `orchestrate_v3` to achieve production-grade quality through:

1. **Test Coverage:** 15% → 70%+ with pytest infrastructure and systematic test writing
2. **Code Quality:** Decompose 1,285-line god module into testable OOP classes (Orchestrator, WaveRunner, SegmentExecutor)
3. **Maintainability:** Extract duplicated file operations into FileOps utility, extract CSS to external stylesheet

## Current State

**Codebase Metrics (orchestrate_v2):**
- Total lines: 6,633 Python + 1,801 lines HTML
- Module count: 14 Python files + 1 HTML file
- Test coverage: 15-20% (only recovery.py and worktree_pool.py tested)
- Largest module: `__main__.py` (1,399 lines)
- Longest function: `_orchestrate_inner()` (388 lines)
- File I/O operations: 7+ scattered with inconsistent error handling
- CSS declarations: 159 inline in dashboard.html with 21 custom properties
- **New features since planning:** interject system, gate tracking, recovery agent, worktree pool

## Issues Addressed

1. **God Module:** `__main__.py` (1,285 lines) mixes concerns - needs OOP decomposition
2. **Duplicated File Ops:** 44 occurrences across 8 modules - needs FileOps utility
3. **Frontend Monolith:** 64 KB inline CSS/JS - needs external stylesheet
4. **Missing Test Infrastructure:** No pytest, no fixtures, no async support
5. **Low Coverage:** 15-20% → target 70%

## Execution Order

**Confidence-First Strategy:**
1. S1 - Create v3 copy (foundation)
2. S2 - Test infrastructure (enables all testing)
3. S3 ∥ S4 - FileOps + CSS (parallel, independent)
4. S5 ∥ S6 ∥ S7 - State + Runner + Planner tests (parallel, builds coverage)
5. S8 ∥ S9 - Monitor + Config/Notify tests (parallel, completes coverage)
6. S10a - SignalHandler + background workers (simpler extraction)
7. S10b - Orchestrator class (protected by test coverage)
8. S12 - WaveRunner/SegmentExecutor (completes decomposition)

## Expected Outcomes

- Test coverage: 70-75% overall
- `__main__.py`: 1,399 → ~450 lines
- All file operations through FileOps utility
- External CSS stylesheet with design tokens
- Testable OOP architecture with dependency injection

## Segments

12 segments across 3 complexity tiers:
- **Low (2):** S1, S4
- **Medium (6):** S2, S3, S5, S7, S9, S10a
- **High (4):** S6, S8, S10b, S12

Estimated cycles: 140-150 with parallelization (199 total sequential)
