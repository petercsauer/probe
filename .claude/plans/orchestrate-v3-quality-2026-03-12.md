# Orchestrate v3 — Quality Improvement & Test Coverage

**Goal:** Refactor orchestrator_v2 to production-grade quality: achieve 70% test coverage, decompose god module into testable OOP classes, extract duplicated file operations, unify frontend CSS.

**Generated:** 2026-03-12

**Entry point:** A (Fresh Goal)

**Status:** Ready for execution

---

## Execution Log

| Segment | Est. Complexity | Risk | Cycles Used | Status | Notes |
|---------|----------------|------|-------------|--------|-------|
| 1: Create v3 copy | Low | 2/10 | -- | -- | Foundation |
| 2: Test infrastructure | Medium | 4/10 | -- | -- | pytest + fixtures |
| 3: FileOps utility | Medium | 3/10 | -- | -- | Parallel with S4 |
| 4: CSS extraction | Low | 2/10 | -- | -- | Parallel with S3 |
| 5: State.py tests | High | 5/10 | -- | -- | Priority 1 coverage |
| 6: Runner.py tests | High | 6/10 | -- | -- | Priority 1 coverage |
| 7: Planner.py tests | Medium | 3/10 | -- | -- | Priority 1 coverage |
| 8: Monitor.py tests | High | 5/10 | -- | -- | Priority 2 coverage |
| 9: Config/Notify tests | Medium | 3/10 | -- | -- | Priority 2 coverage |
| 10: Orchestrator class | High | 8/10 | -- | -- | Decomposition Part 1 |
| 11: WaveRunner/SegmentExecutor | High | 8/10 | -- | -- | Decomposition Part 2 |

**Total estimated cycles:** 100-120 (with parallelization)

**Deep-verify result:** --

**Follow-up plans:** --

---

## Current State Analysis

**Codebase Metrics (orchestrate_v2):**
- Total lines: 4,463 Python + 64 KB HTML
- Module count: 9 Python files + 1 HTML file
- Test coverage: 15-20% (only recovery.py and worktree_pool.py tested)
- Largest module: `__main__.py` (1,285 lines)
- Longest function: `_orchestrate_inner()` (269 lines)
- File I/O operations: 44 scattered across 8 modules with inconsistent error handling
- CSS declarations: 159 inline in dashboard.html with 21 custom properties

**Pain Points:**
1. **God module:** `__main__.py` mixes concerns (wave execution, segment execution, worktree merge, heartbeats, notifications, CLI)
2. **Untestable code:** Long functions (50-269 lines), deep nesting (3+ levels), no dependency injection
3. **Duplicated file ops:** `path.write_text()` × 23, `path.read_text()` × 15, inconsistent error handling
4. **Frontend monolith:** 800 lines inline CSS, 1,600 lines inline JS, no external stylesheet
5. **Low coverage:** 7 of 9 modules completely untested, critical paths unverified

---

## Issue Analysis Briefs

### Issue 1: God Module — __main__.py orchestration logic (1,285 lines)

**Core Problem:**
`__main__.py` contains a 1,285-line monolithic orchestration script with multiple concerns mixed together: wave execution (`_run_wave`: 228 lines), single-segment execution (`_run_one`: 142 lines nested inside `_run_wave`), core loop (`_orchestrate_inner`: 269 lines), worktree merging (`_merge_worktree_changes`: 93 lines), heartbeats, notifications, signal handling, and CLI parsing. Functions exceed 50-200 lines with deep nesting (3+ levels), making them impossible to unit test without running full orchestration. No dependency injection — all collaborators instantiated inline.

**Root Cause:**
Grew organically from a procedural bash script migration without object-oriented refactoring. Each feature added more branches to existing mega-functions rather than extracting new classes.

**Proposed Fix:**
Decompose into OOP architecture with clear separation of concerns:

1. **`Orchestrator`** class — high-level coordinator
   - Constructor takes dependencies: `StateDB`, `Config`, `Notifier`, `WorktreePool`
   - `async def run()` — main entry point
   - `async def run_wave(wave_num)` — delegates to WaveRunner

2. **`WaveRunner`** class — executes one wave with bounded parallelism
   - `async def execute(segments, max_parallel)` → list of results
   - Manages semaphore, dependency validation, retry orchestration
   - Delegates single-segment execution to `SegmentExecutor`

3. **`SegmentExecutor`** class — runs one segment with retry logic
   - `async def execute(segment, attempt_num)` → (status, summary)
   - Encapsulates circuit breaker, retry policy, worktree acquisition
   - Calls `run_segment()` from `runner.py` (keep low-level logic there)

4. **`WorktreeManager`** class — wraps WorktreePool + merge logic
   - `async def execute_in_worktree(segment, fn)` — acquires, executes, merges
   - Encapsulates merge logic

5. **`SignalHandler`** class — manages graceful shutdown
   - Registers signal handlers, sets shutdown event
   - Cleans up resources (pool, monitor, database)

**Existing Solutions Evaluated:**
- **Celery** (https://github.com/celery/celery, MIT, 24k★, active) — Distributed task queue with worker pools. Rejected: Requires message broker (Redis/RabbitMQ), overkill for local sequential orchestration.
- **Luigi** (https://github.com/spotify/luigi, Apache-2.0, 17k★, maintained) — Batch job pipeline framework from Spotify. Rejected: Designed for Hadoop/batch ETL, assumes idempotent tasks, poor fit for iterative AI segment building.
- **Temporal** (https://github.com/temporalio/temporal, MIT, 12k★, active) — Durable workflow engine with replay. Rejected: Requires server infrastructure, too heavyweight for CLI tool.
- **Prefect** (https://github.com/PrefectHQ/prefect, Apache-2.0, 18k★, active) — Modern data workflow orchestration. Rejected: Cloud-first design, requires API server.
- **Hand-rolled Coordinator pattern** — Adopted: Simple, testable, no external dependencies, full control over orchestration semantics.

**Alternatives Considered:**
1. **Keep procedural but extract functions** — Break `_run_wave` into smaller functions. Rejected: Doesn't solve testing problem (still need to mock global state), doesn't reduce coupling.
2. **Actor model with asyncio queues** — Each segment is an actor receiving messages. Rejected: Over-engineered for sequential dependency graph, harder to reason about control flow.

**Pre-Mortem — What Could Go Wrong:**
- **Regression in retry logic** — Circuit breaker state machine is subtle (permanent failure detection). Need comprehensive tests covering all status transitions.
- **Shutdown race conditions** — Signal handlers must coordinate with in-flight segment tasks. Use asyncio.Event properly, test with simulated SIGTERM.
- **Worktree merge conflicts missed** — New abstraction must preserve exact git merge conflict detection logic. Copy existing tests from `test_worktree_pool.py`.
- **Dependency injection breaks existing code** — All callers of orchestration must pass dependencies. Migration must be done atomically or behind feature flag.
- **Performance regression** — Class instantiation overhead negligible, but verify with profiling that no new bottlenecks introduced.

**Risk Factor:** 8/10 — Cross-cutting architectural change affecting core orchestration loop

**Evidence for Optimality:**
- **Codebase evidence**: Existing `CircuitBreaker`, `RetryPolicy` classes prove OOP patterns work well for orchestrator's needs.
- **Project conventions**: Rust crates use clear module boundaries (`prb-core`, `prb-cli`) — orchestrator should follow similar separation.
- **External evidence**: Coordinator pattern (Fowler, PoEAA) is standard for orchestration systems — central coordinator delegates to specialized handlers.
- **Testing evidence**: `test_recovery.py` and `test_worktree_pool.py` successfully test isolated components — proves testing strategy works.

**Blast Radius:**
- **Direct changes**: `__main__.py` (full rewrite ~600 lines removed, ~400 new class definitions)
- **Potential ripple**: Any external callers of orchestration (CLI entry points, tests) must adapt to new `Orchestrator` class API

---

### Issue 2: Duplicated File Operations (44 occurrences across 8 modules)

**Core Problem:**
File I/O operations scattered across 8 modules with inconsistent error handling, encoding strategies, and atomicity guarantees. `path.write_text(content, encoding="utf-8")` appears 23 times with no error handling; `path.read_text(encoding="utf-8", errors="replace")` appears 15 times with mixed error strategies (some use `replace`, some don't); `path.unlink(missing_ok=True)` appears 6 times (sometimes with flag, sometimes without causing crashes). TOCTOU bugs from `if path.exists()` checks before operations (`monitor.py:260`, `runner.py:379`).

**Root Cause:**
No file operations abstraction layer. Each module directly uses `pathlib.Path` methods, duplicating error handling logic or omitting it entirely.

**Proposed Fix:**
Create `scripts/orchestrate_v3/fileops.py` utility module with atomic, error-handled operations:

```python
from pathlib import Path
from typing import Optional
import tempfile
import os

class FileOpsError(Exception):
    """Base exception for file operations failures."""
    pass

class FileOps:
    """Atomic file operations with consistent error handling."""

    @staticmethod
    def read_text(path: Path, encoding: str = "utf-8",
                  errors: str = "replace") -> str:
        """Read text file with explicit encoding and error handling."""
        try:
            return path.read_text(encoding=encoding, errors=errors)
        except FileNotFoundError as e:
            raise FileOpsError(f"File not found: {path}") from e
        except PermissionError as e:
            raise FileOpsError(f"Permission denied: {path}") from e
        except Exception as e:
            raise FileOpsError(f"Failed to read {path}: {e}") from e

    @staticmethod
    def write_text_atomic(path: Path, content: str,
                         encoding: str = "utf-8") -> None:
        """Write text file atomically (temp file + rename)."""
        temp_path = path.with_suffix(path.suffix + ".tmp")
        try:
            temp_path.write_text(content, encoding=encoding)
            # os.replace() is atomic on POSIX and Windows 3.3+
            os.replace(temp_path, path)
        except Exception as e:
            temp_path.unlink(missing_ok=True)
            raise FileOpsError(f"Failed to write {path}: {e}") from e

    @staticmethod
    def remove_safe(path: Path) -> bool:
        """Remove file, return True if existed, False if already gone."""
        try:
            path.unlink()
            return True
        except FileNotFoundError:
            return False
        except Exception as e:
            raise FileOpsError(f"Failed to remove {path}: {e}") from e

    @staticmethod
    def ensure_dir(path: Path) -> None:
        """Create directory and parents if needed."""
        path.mkdir(parents=True, exist_ok=True)
```

Replace all 44 direct file operations with `FileOps` calls.

**Existing Solutions Evaluated:**
- **pathlib.Path** (stdlib) — Already used, but lacks error handling abstractions and atomic write support. Insufficient alone.
- **aiofiles** (https://github.com/Tinche/aiofiles, Apache-2.0, 2.5k★, active, already a dependency) — Async file I/O wrappers. Rejected: Doesn't add error handling or atomicity, just async wrappers.
- **fs** (https://github.com/PyFilesystem/pyfilesystem2, MIT, 2k★, maintained) — Filesystem abstraction layer (local, S3, memory). Rejected: Overkill for local-only needs, heavyweight dependency.
- **atomicwrites** (https://github.com/untitaker/python-atomicwrites, MIT, 500★, inactive since 2021) — Atomic file writes. Rejected: Unmaintained, trivial to implement ourselves with `os.replace()`.
- **Custom FileOps utility class** — Adopted: Thin wrapper over pathlib with consistent error handling, explicit encoding, atomic writes via temp+rename pattern.

**Alternatives Considered:**
1. **Async-first with aiofiles** — Make all file ops async. Rejected: Adds complexity (all callers become async), orchestrator bottleneck is subprocess execution not file I/O.
2. **Use context managers for all writes** — `with FileOps.atomic_write(path) as f:`. Rejected: Overkill for simple write_text use cases, adds boilerplate.

**Pre-Mortem — What Could Go Wrong:**
- **Atomic writes break on network filesystems** — `os.replace()` may not be atomic on NFS. Mitigation: Orchestrator runs locally, not on network shares.
- **Encoding errors surface previously hidden issues** — Explicit UTF-8 encoding may fail on files with mixed encodings. Mitigation: Keep `errors="replace"` for reads where robustness matters (logs).
- **Exception wrapping loses stack traces** — `raise FileOpsError(...) from e` preserves chain, but log formatters might not show it. Mitigation: Add `__cause__` inspection to error messages.
- **Temp file cleanup on crash** — If Python crashes between write and rename, `.tmp` files left behind. Mitigation: Acceptable (rare), or add background cleanup task.

**Risk Factor:** 3/10 — Isolated utility module, low coupling to business logic

**Evidence for Optimality:**
- **Codebase evidence**: 44 scattered file operations prove centralization needed; inconsistent error handling shows danger of duplication.
- **External evidence**: PEP 597 (Python 3.10+) warns on implicit encoding — explicit UTF-8 is best practice.
- **External evidence**: POSIX atomic rename pattern is standard for safe file updates (used by databases, text editors, package managers).
- **Simplicity**: 100-line utility module vs 1k-line dependency — YAGNI principle favors simple solution.

**Blast Radius:**
- **Direct changes**: New `fileops.py` module (~100 lines), 44 call sites updated across 8 modules
- **Potential ripple**: Error handling becomes stricter — callers must handle `FileOpsError` or let it propagate (currently many errors silently ignored)

---

### Issue 3: Frontend Monolith — dashboard.html (64 KB, 159 CSS declarations)

**Core Problem:**
`dashboard.html` contains 64 KB of inline CSS (~800 lines in `<style>`) and JavaScript (~1,600 lines in `<script>`). 159 CSS property declarations with duplicate values: `rgba(255,255,255,0.04)` appears 8 times, `font-size: 11px` appears 12 times, multiple `.btn-*` classes with 90% identical styles. No external stylesheet prevents browser caching (full HTML reload on every dashboard view). No CSS linting or minification possible. Hard to maintain — changing theme colors requires editing 20+ locations.

**Root Cause:**
Built as single-file prototype for rapid iteration. Never refactored into modular structure as features accumulated.

**Proposed Fix:**
1. **Extract CSS to external stylesheet** — Create `dashboard.css` (estimated ~600 lines after deduplication)
   - Move all `<style>` content to external file
   - Reference via `<link rel="stylesheet" href="/api/static/dashboard.css">`
   - Add `/api/static/{filename}` endpoint to `monitor.py` to serve static files

2. **Consolidate design tokens** — Centralize all color/spacing/font definitions
   ```css
   :root {
     /* Colors - semantic names */
     --color-bg: #1c2128;
     --color-surface: #161b22;
     --color-border: #30363d;
     --color-text: #e6edf3;
     --color-text-dim: #8b949e;
     --color-text-muted: #6e7681;

     /* Status colors */
     --color-success: #238636;
     --color-danger: #da3633;
     --color-warning: #d29922;
     --color-info: #1f6feb;

     /* Spacing scale */
     --space-xs: 4px;
     --space-sm: 8px;
     --space-md: 12px;
     --space-lg: 16px;
     --space-xl: 24px;

     /* Typography */
     --font-body: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
     --font-mono: 'SF Mono', monospace;
     --font-size-xs: 10px;
     --font-size-sm: 11px;
     --font-size-base: 13px;
     --font-size-lg: 14px;
   }
   ```

3. **DRY CSS with utility classes** — Extract repeated patterns
   ```css
   /* Before: Duplicated 8 times */
   .seg-row:hover { background: rgba(255,255,255,0.04); }
   .tab-btn:hover { background: rgba(255,255,255,0.04); }

   /* After: Single class */
   .hover-highlight:hover { background: rgba(255,255,255,0.04); }
   ```

4. **BEM naming convention** for new classes
   ```css
   .segment__title { /* element */ }
   .segment__status { /* element */ }
   .segment__status--running { /* modifier */ }
   ```

5. **Keep JavaScript embedded** — Don't extract JS to separate file yet (defer to future)

**Existing Solutions Evaluated:**
- **Tailwind CSS** (https://github.com/tailwindlabs/tailwindcss, MIT, 84k★, active) — Utility-first CSS framework. Rejected: Requires Node.js build step (npm, PostCSS), adds 3.5 MB dependency, forces utility class proliferation in HTML.
- **Bootstrap** (https://github.com/twbs/bootstrap, MIT, 170k★, active) — Component library with grid system. Rejected: 300 KB library for features already built, custom dashboard design incompatible with Bootstrap's opinions.
- **Open Props** (https://github.com/argyleink/open-props, MIT, 5k★, active) — CSS custom properties library with design tokens. Considered: Great inspiration for token naming, but adds dependency for what's 50 lines of custom properties. Rejected as direct dependency, adopted naming patterns.
- **Extract to external stylesheet (hand-rolled)** — Adopted: Zero dependencies, enables browser caching, works with existing HTML, no build step.

**Alternatives Considered:**
1. **CSS-in-JS (e.g., styled-components)** — Requires React/framework, build step. Rejected: Overkill for vanilla JS dashboard.
2. **CSS Modules** — Scoped CSS with build-time processing. Rejected: Requires bundler (webpack/vite), not worth complexity for single HTML file.

**Pre-Mortem — What Could Go Wrong:**
- **Cache invalidation issues** — Browser caches `dashboard.css`, users see stale styles after update. Mitigation: Add version query param `/api/static/dashboard.css?v={hash}` or use ETags.
- **CSS load race condition** — JS executes before CSS loads, causing FOUC (flash of unstyled content). Mitigation: HTML blocks on CSS load by default, ensure `<link>` in `<head>` before `<script>`.
- **Breaking existing styles during extraction** — CSS specificity changes when moving from `<style>` to external file. Mitigation: Keep exact same selectors, test in browser before/after.
- **CSS custom properties not supported in old browsers** — IE 11 doesn't support CSS variables. Mitigation: Acceptable (orchestrator is developer tool, IE 11 usage negligible).

**Risk Factor:** 2/10 — Isolated frontend change, no backend logic affected

**Evidence for Optimality:**
- **Codebase evidence**: 159 CSS declarations with duplicates prove consolidation needed; 21 CSS custom properties already present show theming intention.
- **External evidence**: Separation of concerns (HTML/CSS/JS) is fundamental web architecture principle — inline styles violate this.
- **Performance evidence**: External CSS enables browser caching (reduces bandwidth by 800 lines per pageload), enables gzip compression (CSS highly compressible).
- **Maintainability evidence**: Single source of truth for colors/spacing reduces change friction (1 edit vs 20 edits to change theme).

**Blast Radius:**
- **Direct changes**: `dashboard.html` (remove `<style>` block, add `<link>`), new `dashboard.css` file (~600 lines), `monitor.py` (add static file endpoint ~15 lines)
- **Potential ripple**: None — CSS refactoring is invisible to JavaScript logic

---

### Issue 4: Missing Test Infrastructure — No pytest, no fixtures, no async support

**Core Problem:**
Existing tests (`test_recovery.py`, `test_worktree_pool.py`) are standalone scripts using `unittest.mock` with manual `asyncio.run()` calls and custom test runner in `if __name__ == "__main__"` block. No test discovery (can't run `pytest` to find tests), no shared fixtures (each test recreates mock StateDB), no parametrized tests, no async test helpers, no coverage measurement. Makes writing new tests painful — developers copy-paste boilerplate instead of reusing test infrastructure.

**Root Cause:**
Tests written before pytest adoption. No `conftest.py` to define shared fixtures. No `pytest.ini` or `pyproject.toml` test configuration.

**Proposed Fix:**
Set up pytest infrastructure with async support and common fixtures:

1. **Add pytest dependencies** to `requirements.txt`:
   ```
   pytest>=8.0.0
   pytest-asyncio>=0.23.0
   pytest-cov>=4.1.0
   pytest-mock>=3.12.0
   ```

2. **Create `conftest.py`** with shared fixtures for temp directories, mock StateDB, default config, mock segments, mock notifier

3. **Add `pytest.ini`** configuration with coverage settings, asyncio_mode=auto, testpaths

4. **Convert existing tests** to pytest format (remove `if __name__` blocks, add `@pytest.mark.asyncio`)

5. **Add integration test helpers** for full orchestrator setup

**Existing Solutions Evaluated:**
- **pytest** (https://github.com/pytest-dev/pytest, MIT, 12k★, active) — Industry standard Python testing framework. Adopted: Better fixtures, parametrization, plugin ecosystem.
- **pytest-asyncio** (https://github.com/pytest-dev/pytest-asyncio, Apache-2.0, 1.4k★, active) — Async test support with `@pytest.mark.asyncio`. Adopted: Essential for testing async code.
- **pytest-cov** (https://github.com/pytest-dev/pytest-cov, MIT, 1.7k★, active) — Coverage measurement integrated with pytest. Adopted: Needed for 70% coverage goal.
- **pytest-mock** (https://github.com/pytest-dev/pytest-mock, MIT, 1.8k★, active) — Better mocking API via `mocker` fixture. Adopted: Cleaner than unittest.mock.
- **unittest** (stdlib) — Already partially used in existing tests. Rejected: Verbose, no fixtures, poor async support, less maintainable.
- **Hypothesis** (https://github.com/HypothesisWorks/hypothesis, MPL-2.0, 7.5k★, active) — Property-based testing for edge case discovery. Considered: Excellent for fuzzing retry logic, file path handling. Optional future enhancement (not blocking 70% coverage).

**Alternatives Considered:**
1. **Keep unittest, add fixtures manually** — Create base test classes with setUp/tearDown. Rejected: Verbose, fixtures not composable, no parametrization.
2. **Use Robot Framework** — Keyword-driven testing. Rejected: Overkill for unit/integration tests, better suited for E2E acceptance tests.

**Pre-Mortem — What Could Go Wrong:**
- **Async fixture cleanup issues** — Teardown might not run if test crashes. Mitigation: Use `pytest-asyncio`'s proper cleanup guarantees, wrap dangerous operations in try/finally.
- **Temp directory permissions** — Tests might fail on CI with restricted `/tmp`. Mitigation: Use `pytest`'s `tmp_path` fixture (creates in pytest temp dir with proper perms).
- **Coverage false positives** — Lines covered but not meaningfully tested (e.g., `pass` statements, trivial getters). Mitigation: Review coverage reports manually, focus on critical paths.
- **Test database state leaks** — Tests sharing fixtures might see each other's data. Mitigation: Each test gets fresh `mock_state_db` fixture (function scope), no shared state.
- **Import path issues** — Moving to `orchestrate_v3` breaks existing imports. Mitigation: Update all imports atomically in segment that creates v3.

**Risk Factor:** 4/10 — Affects all future test development, but backward compatible (existing tests still runnable as scripts)

**Evidence for Optimality:**
- **Industry standard**: pytest is de facto standard (12k★, used by Django, Flask, FastAPI, Requests).
- **Project evidence**: Existing tests prove async testing is needed — manual `asyncio.run()` is boilerplate pytest-asyncio eliminates.
- **Adoption evidence**: pytest-asyncio + pytest-cov are standard stack for async Python projects (evidence: FastAPI, aiohttp, httpx all use this combo).
- **Developer experience**: Fixtures reduce boilerplate from ~20 lines per test to ~2 lines (codebase evidence from existing duplicated setup code).

**Blast Radius:**
- **Direct changes**: New `conftest.py` (~150 lines), `pytest.ini` (~15 lines), `requirements.txt` (+4 dependencies), convert 2 existing test files (~50 line changes)
- **Potential ripple**: None — pytest runs existing tests unchanged (backward compatible), new tests opt into fixtures

---

### Issue 5: Low Test Coverage (15-20% → target 70%)

**Core Problem:**
Only 2 of 9 modules have tests: `recovery.py` and `worktree_pool.py` are tested (461 test lines total), while 7 critical modules are completely untested: `runner.py` (525 lines), `state.py` (569 lines), `monitor.py` (439 lines), `__main__.py` (1,285 lines), `planner.py` (163 lines), `config.py` (242 lines), `notify.py` (212 lines). Critical paths like segment execution (`run_segment()`), database operations (17 async StateDB methods), HTTP endpoints (9 handlers), retry logic, and circuit breaker are untested. Estimated current coverage: 15-20%.

**Root Cause:**
Tests written only for new features (recovery, worktree pool) added in 2026-03-10 plan. Core orchestration logic inherited from bash script without tests. No coverage requirements enforced.

**Proposed Fix:**
Systematic test writing prioritized by criticality and complexity:

**Priority 1 — Core Business Logic (40% coverage target):**
1. **state.py** tests (~300 lines) — Test all 17 async methods, schema migrations, concurrent access
2. **runner.py** tests (~400 lines) — Test `run_segment()`, circuit breaker, prompt building, heartbeat, log archival
3. **planner.py** tests (~200 lines) — Test frontmatter parsing, wave assignment, circular dependency detection

**Priority 2 — HTTP & I/O (20% coverage target):**
4. **monitor.py** tests (~300 lines) — Test all 9 HTTP endpoints, SSE streaming, control API
5. **fileops.py** tests (~150 lines) — Test atomic writes, error handling, encoding edge cases

**Priority 3 — Configuration & Utilities (10% coverage target):**
6. **config.py** tests (~150 lines) — Test TOML loading, env var substitution, RetryPolicy logic
7. **notify.py** tests (~100 lines) — Test notification queueing, retry logic, batching

**Test Writing Guidelines:**
- AAA pattern: Arrange (setup), Act (call function), Assert (verify)
- One assertion per test or related assertions grouped
- Descriptive names: `test_run_segment_timeout_returns_timeout_status()`
- Parametrize repeated logic: `@pytest.mark.parametrize("status", ["failed", "timeout", "blocked"])`
- Integration tests for I/O: Real SQLite (temp file), real files (tmp_path), real HTTP (aiohttp test client)
- Unit tests for pure logic: Mock I/O, focus on business logic

**Coverage Measurement:**
```bash
pytest --cov=scripts/orchestrate_v3 --cov-report=term-missing --cov-report=html
```

**Existing Solutions Evaluated:**
- **Coverage.py** (https://github.com/nedbat/coveragepy, Apache-2.0, 3k★, active, built into pytest-cov) — Line and branch coverage measurement. Adopted: Industry standard.
- **pytest-xdist** (https://github.com/pytest-dev/pytest-xdist, MIT, 1.4k★, active) — Parallel test execution (`pytest -n auto`). Optional nice-to-have: Speeds up test suite as it grows, not critical initially.
- **mutmut** (https://github.com/boxed/mutmut, BSD-3, 900★, active) — Mutation testing (verifies tests catch bugs). Optional future enhancement: Ensures tests are meaningful, not just achieving coverage %.
- **Hypothesis** (property-based testing) — Optional: Great for fuzzing retry delays, file paths, dependency graphs.

**Alternatives Considered:**
1. **100% coverage goal** — Aim for perfect coverage. Rejected: Diminishing returns above 80%, forces testing trivial getters/setters, slows development.
2. **E2E tests only** — Run full orchestration, verify end state. Rejected: Slow (minutes per test), hard to debug failures, doesn't isolate bugs to specific modules.

**Pre-Mortem — What Could Go Wrong:**
- **Flaky async tests** — Race conditions in concurrent DB writes, timeout-sensitive tests. Mitigation: Use `pytest-asyncio` properly, avoid hardcoded sleep delays, mock time where possible.
- **Coverage plateau at 65%** — Hard-to-test code (signal handlers, subprocess spawning) blocks 70% goal. Mitigation: Refactor for testability (Issue 1 decomposition helps), accept lower coverage for integration code with E2E test safety net.
- **Test maintenance burden** — As codebase grows, test updates lag. Mitigation: Run tests in CI (future), enforce coverage gates, treat tests as first-class code.
- **Mock explosion** — Heavy mocking makes tests brittle (break when implementation details change). Mitigation: Prefer integration tests for I/O (real DB, real files), mock only external services (httpx for ntfy).

**Risk Factor:** 5/10 — Touches all modules, but additive (no behavior changes)

**Evidence for Optimality:**
- **Industry standard**: 70% coverage is Google's bar for "well-tested" code, Microsoft's recommendation for production systems.
- **Codebase evidence**: Existing tests for recovery/worktree pool prove testing strategy works — just need to scale it.
- **Test pyramid**: 70% unit (fast, isolated), 20% integration (DB + file I/O), 10% E2E (full orchestration) balances speed and confidence.
- **Incremental approach**: Prioritize critical paths first (state, runner, planner = 40%) achieves most risk reduction with least effort.

**Blast Radius:**
- **Direct changes**: 7 new test files (~1,600 lines total), `conftest.py` fixtures, `pytest.ini` config
- **Potential ripple**: Testability refactoring might reveal bugs (good!), coverage gates might block merges (intentional)

---

## Dependency Diagram

```
S1 (Create v3 copy)
 │
 ├─→ S2 (Test infrastructure)
 │    │
 │    ├─→ S3 (FileOps utility) ──────────┐
 │    │                                   │
 │    ├─→ S4 (CSS extraction) ────────┐  │
 │    │                                │  │
 │    ├─→ S5 (State.py tests) ─────┐  │  │
 │    │                             │  │  │
 │    ├─→ S6 (Runner.py tests) ────┼──┼──┤
 │    │   [needs S3]                │  │  │
 │    │                             │  │  │
 │    ├─→ S7 (Planner.py tests) ───┼──┼──┤
 │    │                             │  │  │
 │    ├─→ S8 (Monitor.py tests) ───┼──┤  │
 │    │   [needs S4]                │  │  │
 │    │                             │  │  │
 │    └─→ S9 (Config/Notify tests)─┘  │  │
 │                                     │  │
 └─────────────────────────────────────┼──┤
                                       │  │
        S10 (Orchestrator coordinator) │  │
         [needs S2-S9 test coverage]   │  │
                 │                     │  │
                 ↓                     │  │
        S11 (WaveRunner/SegmentExecutor)│
         [completes decomposition]     │  │
                                       │  │
                                       ↓  ↓
                          [All issues resolved]
```

**Parallelization Opportunities:**
- **Wave 1**: S3 ∥ S4 (after S2)
- **Wave 2**: S5 ∥ S6 ∥ S7 (after S2+S3)
- **Wave 3**: S8 ∥ S9 (after S2+S4)

---

## Segment Briefs

## Segment 1: Create orchestrate_v3 copy
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Create `scripts/orchestrate_v3/` as a working copy of `orchestrate_v2/` with updated imports and verified functionality.

**Depends on:** None

**Issues addressed:** Prerequisite for all other segments (Issue 1-5 foundation)

**Cycle budget:** 10 (Low)

**Scope:** New directory `scripts/orchestrate_v3/`, update all internal imports

**Key files and context:**
- Source: `scripts/orchestrate_v2/` (13 Python files, 1 HTML file, 1 requirements.txt, 2 test files)
- Target: `scripts/orchestrate_v3/` (fresh copy with updated imports)
- All internal imports change: `from .module import X` remains same, but Python path becomes `scripts.orchestrate_v3.module`
- Entry point: `python -m scripts.orchestrate_v3 <plan_dir>`

**Implementation approach:**
1. Copy entire `orchestrate_v2/` directory to `orchestrate_v3/`
2. Update `__main__.py` module docstring to indicate v3
3. Verify no hardcoded v2 paths in code (grep for "orchestrate_v2")
4. Test imports: `python -c "from scripts.orchestrate_v3 import runner, state, monitor"`
5. Smoke test: Run `python -m scripts.orchestrate_v3 --help` (should print usage)
6. Document in `orchestrate_v3/README.md`: "Refactored version of orchestrate_v2 with improved code quality, 70% test coverage, and modular architecture"

**Alternatives ruled out:**
- In-place refactoring of v2: Rejected (user requested separate v3 copy for safety)
- Git branch instead of directory: Rejected (harder to compare v2 vs v3 side-by-side)

**Pre-mortem risks:**
- Absolute imports break: Some code might use `scripts.orchestrate_v2` absolute imports instead of relative. Mitigation: Grep for all imports, convert to relative or update to v3.
- Circular imports surface: Copy might reveal hidden import cycles. Mitigation: Python will error immediately, fix import order.

**Segment-specific commands:**
- Build: `python -m py_compile scripts/orchestrate_v3/*.py`
- Test (targeted): `python -c "from scripts.orchestrate_v3 import runner, state, monitor, planner, config"`
- Test (regression): `python scripts/orchestrate_v3/test_recovery.py && python scripts/orchestrate_v3/test_worktree_pool.py`
- Test (full gate): `python -m scripts.orchestrate_v3 --help` (should not crash)

**Exit criteria:**
1. Targeted tests: Import all modules successfully (no ImportError)
2. Regression tests: Existing test scripts run and pass
3. Full build gate: `python -m py_compile scripts/orchestrate_v3/*.py` (no syntax errors)
4. Full test gate: `python -m scripts.orchestrate_v3 --help` prints usage without crashing
5. Self-review gate: No absolute `orchestrate_v2` imports remain, no orphaned files
6. Scope verification gate: Only files in `scripts/orchestrate_v3/` modified

**Risk factor:** 2/10

**Estimated complexity:** Low

**Commit message:** `feat(orchestrate): Create orchestrate_v3 as refactored copy of v2`

---

## Segment 2: Test infrastructure setup (pytest + fixtures)
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Establish pytest infrastructure with async support, shared fixtures, and coverage measurement.

**Depends on:** S1 (needs orchestrate_v3 to exist)

**Issues addressed:** Issue 4 (missing test infrastructure)

**Cycle budget:** 15 (Medium)

**Scope:** `scripts/orchestrate_v3/` - add test infrastructure, convert existing tests

**Key files and context:**
- Create: `conftest.py` (~150 lines of fixtures)
- Create: `pytest.ini` (~15 lines of config)
- Modify: `requirements.txt` (+4 dependencies)
- Convert: `test_recovery.py`, `test_worktree_pool.py` (remove `if __name__` blocks, add `@pytest.mark.asyncio`)
- Current test framework: unittest.mock with manual asyncio.run()
- Target: pytest with pytest-asyncio, pytest-cov, pytest-mock

**Implementation approach:**
1. Add dependencies to `requirements.txt`: pytest>=8.0.0, pytest-asyncio>=0.23.0, pytest-cov>=4.1.0, pytest-mock>=3.12.0

2. Create `conftest.py` with fixtures:
   - `@pytest.fixture async def temp_dir()` - TemporaryDirectory wrapper
   - `@pytest.fixture async def mock_state_db(temp_dir)` - In-memory SQLite StateDB
   - `@pytest.fixture def default_config()` - OrchestrateConfig with safe defaults
   - `@pytest.fixture def mock_segment()` - Factory for creating test Segment objects
   - `@pytest.fixture def mock_notifier()` - Captures notification calls for assertions

3. Create `pytest.ini` with testpaths, asyncio_mode=auto, coverage settings

4. Convert `test_recovery.py`: Remove `if __name__ == "__main__"`, convert class methods to module-level functions, add `@pytest.mark.asyncio`, remove manual `asyncio.run()`

5. Convert `test_worktree_pool.py`: Same conversion pattern, use `temp_dir` fixture

6. Verify: Run `pytest scripts/orchestrate_v3/ -v` - should discover and run all tests

**Alternatives ruled out:**
- Keep unittest.mock: Rejected (verbose, no fixture composability, poor async support)
- Use tox for multi-Python testing: Rejected (overkill for single Python version, adds complexity)

**Pre-mortem risks:**
- Async fixture lifecycle issues: Teardown might not run if test crashes. Mitigation: pytest-asyncio handles cleanup properly, verify with intentional test failure.
- Import path confusion: pytest might import wrong orchestrate version. Mitigation: Run from repo root with `pytest scripts/orchestrate_v3/`.
- Coverage too low initially: Only ~15-20% with existing tests. Mitigation: Expected, next segments add tests.

**Segment-specific commands:**
- Build: `pip install -r scripts/orchestrate_v3/requirements.txt`
- Test (targeted): `pytest scripts/orchestrate_v3/test_recovery.py -v`
- Test (regression): `pytest scripts/orchestrate_v3/ -v` (both converted tests must pass)
- Test (full gate): `pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3 --cov-report=term`

**Exit criteria:**
1. Targeted tests: pytest discovers and runs test_recovery.py (8+ tests pass)
2. Regression tests: Both test files pass with pytest (same assertions as before)
3. Full build gate: `pip install` succeeds, all dependencies resolve
4. Full test gate: `pytest scripts/orchestrate_v3/ -v` passes (green output)
5. Self-review gate: No print() statements in tests, all async tests have @pytest.mark.asyncio
6. Scope verification gate: Only orchestrate_v3 files touched, conftest.py has no business logic

**Risk factor:** 4/10

**Estimated complexity:** Medium

**Commit message:** `test(orchestrate): Add pytest infrastructure with async fixtures`

---

## Segment 3: FileOps utility abstraction
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Create centralized file operations utility with atomic writes and consistent error handling, replace usage in runner.py and monitor.py.

**Depends on:** S2 (needs test infrastructure)

**Issues addressed:** Issue 2 (duplicated file operations)

**Cycle budget:** 15 (Medium)

**Scope:** `scripts/orchestrate_v3/` - new fileops.py, update runner.py and monitor.py

**Key files and context:**
- Create: `fileops.py` (~150 lines: FileOps class + FileOpsError)
- Modify: `runner.py` (7 file operation call sites → FileOps calls)
- Modify: `monitor.py` (8 file operation call sites → FileOps calls)
- Create: `test_fileops.py` (~150 lines: test atomic writes, encoding, errors)
- Current pattern: Direct `path.write_text()`, `path.read_text()`, `path.unlink()` with inconsistent error handling
- Anti-patterns to fix: TOCTOU bugs (`if path.exists()` before read), non-atomic writes, no error wrapping

**Implementation approach:**
1. Create `fileops.py` with FileOpsError exception and FileOps class containing:
   - `read_text(path, encoding="utf-8", errors="replace")` - with FileOpsError wrapping
   - `write_text_atomic(path, content, encoding="utf-8")` - temp file + os.replace()
   - `remove_safe(path)` - returns bool, handles FileNotFoundError
   - `ensure_dir(path)` - mkdir with parents=True, exist_ok=True

2. Replace in `runner.py`:
   - Line 374: `prompt_file.write_text(...)` → `FileOps.write_text_atomic(...)`
   - Lines 378-379: raw_log and human_log operations → FileOps equivalents
   - Archive operations (lines 375-386): Use FileOps for atomic rename

3. Replace in `monitor.py`:
   - Line 49: dashboard.html read → `FileOps.read_text(...)`
   - Prompt loading (line 198): Use FileOps.read_text with try/except FileOpsError
   - Log file checks: Replace `path.exists()` with try/except FileOpsError on read

4. Write `test_fileops.py`: Test atomic write, error handling, encoding, safe remove

**Alternatives ruled out:**
- Use aiofiles for async I/O: Rejected (file I/O not bottleneck, adds complexity)
- Import external library (fs, atomicwrites): Rejected (100-line utility is simpler than dependency)

**Pre-mortem risks:**
- Atomic writes break on network filesystems: `os.replace()` may not be atomic on NFS. Mitigation: Orchestrator runs locally, not on network shares.
- Error wrapping hides stack traces: `raise ... from e` preserves chain. Mitigation: Document in FileOpsError docstring.
- Performance regression from temp file pattern: Adds extra I/O. Mitigation: Negligible (logs/prompts are small), atomicity worth the cost.

**Segment-specific commands:**
- Build: `python -m py_compile scripts/orchestrate_v3/fileops.py`
- Test (targeted): `pytest scripts/orchestrate_v3/test_fileops.py -v`
- Test (regression): `pytest scripts/orchestrate_v3/test_recovery.py scripts/orchestrate_v3/test_worktree_pool.py -v`
- Test (full gate): `pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3/fileops.py --cov-report=term`

**Exit criteria:**
1. Targeted tests: test_fileops.py passes (10+ test cases)
2. Regression tests: Existing tests still pass (FileOps doesn't break behavior)
3. Full build gate: No syntax errors in fileops.py
4. Full test gate: `pytest scripts/orchestrate_v3/ -v` all tests pass
5. Self-review gate: All runner.py and monitor.py file ops converted (grep for `.write_text`, `.read_text`, `.unlink`)
6. Scope verification gate: Only fileops.py, test_fileops.py, runner.py, monitor.py modified

**Risk factor:** 3/10

**Estimated complexity:** Medium

**Commit message:** `refactor(orchestrate): Add FileOps utility for atomic file operations`

---

## Segment 4: Extract CSS to external stylesheet
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Extract inline CSS from dashboard.html to external dashboard.css, consolidate design tokens, add static file endpoint.

**Depends on:** S1 (needs orchestrate_v3)

**Issues addressed:** Issue 3 (frontend monolith)

**Cycle budget:** 12 (Low)

**Scope:** `scripts/orchestrate_v3/` - create dashboard.css, update dashboard.html, add static endpoint to monitor.py

**Key files and context:**
- Create: `dashboard.css` (~600 lines after deduplication)
- Modify: `dashboard.html` (remove ~800 line `<style>` block, add `<link rel="stylesheet">`)
- Modify: `monitor.py` (add static file handler ~20 lines)
- Current: 159 CSS declarations inline, 21 CSS custom properties, duplicates like `rgba(255,255,255,0.04)` × 8
- Keep: JavaScript remains embedded (1,600 lines - not extracted in this segment)

**Implementation approach:**
1. Create `dashboard.css`: Extract all content from `<style>...</style>`, consolidate duplicate values into CSS custom properties, organize by sections (Reset → Base → Components → Utilities → Media Queries)

2. Update `dashboard.html`: Remove `<style>` block, add `<link rel="stylesheet" href="/api/static/dashboard.css">` in `<head>`

3. Add static file endpoint to `monitor.py`:
   ```python
   app.router.add_get("/api/static/{filename}", _handle_static)

   async def _handle_static(request: web.Request) -> web.Response:
       filename = request.match_info["filename"]
       if filename != "dashboard.css":
           return web.Response(status=404)
       css_path = Path(__file__).parent / "dashboard.css"
       content = FileOps.read_text(css_path)
       return web.Response(text=content, content_type="text/css",
                         headers={"Cache-Control": "public, max-age=3600"})
   ```

4. Consolidate design tokens: Add --space-xs/sm/md/lg/xl, --font-size-xs/sm/base/lg tokens, replace hardcoded values

5. Apply BEM naming convention documentation in CSS header comment

**Alternatives ruled out:**
- Use CSS preprocessor (SASS/LESS): Rejected (requires build step)
- Extract JavaScript too: Rejected (defer to future segment)

**Pre-mortem risks:**
- CSS specificity changes: External `<link>` has same specificity as inline `<style>`. Mitigation: No change expected.
- Cache invalidation: Browser caches old CSS. Mitigation: Add version query param or ETag headers.
- FOUC: `<link>` in `<head>` blocks render. Mitigation: No FOUC risk.
- Path traversal security: Hardcoded whitelist (only dashboard.css allowed). Mitigation: Security check in handler.

**Segment-specific commands:**
- Build: `python -m py_compile scripts/orchestrate_v3/monitor.py`
- Test (targeted): Manual - start monitor, verify `/api/static/dashboard.css` returns CSS
- Test (regression): `pytest scripts/orchestrate_v3/ -v`
- Test (full gate): Manual browser test - dashboard renders correctly

**Exit criteria:**
1. Targeted tests: `curl http://localhost:9876/api/static/dashboard.css` returns CSS (600 lines)
2. Regression tests: pytest passes, monitor server starts
3. Full build gate: No Python syntax errors
4. Full test gate: Dashboard loads in browser, styles render correctly
5. Self-review gate: No inline styles in HTML (except rare `style=""` attributes)
6. Scope verification gate: Only dashboard.html, dashboard.css (new), monitor.py modified

**Risk factor:** 2/10

**Estimated complexity:** Low

**Commit message:** `refactor(orchestrate): Extract CSS to external stylesheet with design tokens`

---

## Segment 5: State.py comprehensive tests (Priority 1 coverage)
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Achieve 90%+ test coverage for state.py (all StateDB methods, migrations, concurrent access).

**Depends on:** S2 (needs pytest infrastructure)

**Issues addressed:** Issue 5 (test coverage - Priority 1)

**Cycle budget:** 18 (High)

**Scope:** `scripts/orchestrate_v3/test_state.py` - comprehensive StateDB tests

**Key files and context:**
- Create: `test_state.py` (~300 lines)
- Test target: `state.py` (569 lines, 17 async methods, critical path)
- Database operations: segments CRUD, events log, attempts tracking, notifications outbox, interjections, gate attempts
- Current coverage: 0% (untested)
- Target coverage: 90%+

**Implementation approach:**
1. Test StateDB lifecycle: create, migrations, close
2. Test segment operations: init_segments, get_segment, set_status, increment_attempts, reset_stale_running (parametrize status values)
3. Test events log: log_event, get_events with limits and filtering
4. Test attempts tracking: record_attempt, get_attempts ordered
5. Test notifications outbox: enqueue, dequeue, mark sent, retry logic, deduplication
6. Test interjections: add_interject, get_pending_interject, consume_interject
7. Test concurrent access: 4 async tasks writing simultaneously (use asyncio.gather)
8. Test migrations: Create old DB, apply migrations, verify new columns
9. Use fixtures: mock_state_db for most tests, temp_dir for migration tests

**Alternatives ruled out:**
- Mock aiosqlite: Rejected (defeats purpose - want to test SQL queries work)
- Use real persistent DB: Rejected (slow, state leaks between tests)

**Pre-mortem risks:**
- Async timing issues: SQLite WAL mode handles concurrency. Mitigation: Use asyncio.gather properly.
- Schema changes break tests: Mitigation: Tests focus on behavior not schema details.
- Temp DB cleanup: tmp_path fixture auto-cleans. Mitigation: Verify with `ls /tmp` after test run.

**Segment-specific commands:**
- Build: `python -m py_compile scripts/orchestrate_v3/test_state.py`
- Test (targeted): `pytest scripts/orchestrate_v3/test_state.py -v`
- Test (regression): `pytest scripts/orchestrate_v3/ -v`
- Test (full gate): `pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3/state.py --cov-report=term`

**Exit criteria:**
1. Targeted tests: test_state.py passes (30+ tests covering all StateDB methods)
2. Regression tests: All existing tests still pass
3. Full build gate: No syntax errors
4. Full test gate: `pytest scripts/orchestrate_v3/ -v` all green
5. Self-review gate: All 17 StateDB methods tested, concurrent access tested
6. Scope verification gate: Only test_state.py created, state.py unchanged

**Risk factor:** 5/10

**Estimated complexity:** High

**Commit message:** `test(orchestrate): Add comprehensive StateDB tests (90% coverage)`

---

## Segment 6: Runner.py comprehensive tests (Priority 1 coverage)
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Achieve 85%+ test coverage for runner.py (run_segment, circuit breaker, prompt building, heartbeat, log archival).

**Depends on:** S2 (pytest), S3 (FileOps for mocking)

**Issues addressed:** Issue 5 (test coverage - Priority 1)

**Cycle budget:** 20 (High)

**Scope:** `scripts/orchestrate_v3/test_runner.py` - comprehensive runner tests

**Key files and context:**
- Create: `test_runner.py` (~400 lines)
- Test target: `runner.py` (525 lines, run_segment function 150+ lines, critical path)
- Key logic: subprocess execution, timeout handling, circuit breaker, heartbeat updates, log archival on retry
- Current coverage: 0%
- Target coverage: 85%+ (some subprocess integration paths hard to test)

**Implementation approach:**
1. Test CircuitBreaker: Parametrize all PERMANENT_PATTERNS, test retryable errors, test add_pattern extensibility
2. Test _resolve_isolation_env: Test template expansion for worktree/env strategies
3. Test _build_prompt: Test basic, with interject, without interject, mock segment/config
4. Test _build_env: Test auth_env inclusion, isolation_env merge
5. Test run_segment (mock subprocess): Mock asyncio.create_subprocess_exec, mock FileOps, verify status/summary returned
6. Test timeout handling: Mock asyncio.wait_for timeout, verify SIGTERM sent
7. Test heartbeat updates: Mock file reading, verify DB writes, test stall detection
8. Test log archival: Test rename to -attempt1.log on second attempt, skip on first attempt, use real temp files
9. Test token extraction: Parse stream.jsonl for token usage, test valid/invalid JSON
10. Mock strategy: Mock subprocess (too complex for real), mock FileOps (except archival), real StateDB, real config

**Alternatives ruled out:**
- Integration tests with real Claude CLI: Rejected (slow, requires API key, nondeterministic)
- Mock everything: Rejected (defeats purpose of testing logic)

**Pre-mortem risks:**
- Mock complexity: Subprocess mocking is intricate. Mitigation: Use pytest-mock's AsyncMock, follow aiohttp test patterns.
- Flaky timeout tests: Timing-sensitive. Mitigation: Mock time.time() for deterministic timing.
- Log archival race conditions: Real file renames might fail on Windows. Mitigation: Test on POSIX primarily.

**Segment-specific commands:**
- Build: `python -m py_compile scripts/orchestrate_v3/test_runner.py`
- Test (targeted): `pytest scripts/orchestrate_v3/test_runner.py -v`
- Test (regression): `pytest scripts/orchestrate_v3/ -v`
- Test (full gate): `pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3/runner.py --cov-report=term`

**Exit criteria:**
1. Targeted tests: test_runner.py passes (40+ tests)
2. Regression tests: All existing tests pass
3. Full build gate: No syntax errors
4. Full test gate: `pytest scripts/orchestrate_v3/ -v` all green
5. Self-review gate: All major functions tested, mock strategy documented
6. Scope verification gate: Only test_runner.py created, runner.py unchanged

**Risk factor:** 6/10

**Estimated complexity:** High

**Commit message:** `test(orchestrate): Add comprehensive runner.py tests (85% coverage)`

---

## Segment 7: Planner.py comprehensive tests (Priority 1 coverage)
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Achieve 95%+ test coverage for planner.py (frontmatter parsing, wave assignment, dependency resolution).

**Depends on:** S2 (pytest)

**Issues addressed:** Issue 5 (test coverage - Priority 1)

**Cycle budget:** 12 (Medium)

**Scope:** `scripts/orchestrate_v3/test_planner.py` - comprehensive planner tests

**Key files and context:**
- Create: `test_planner.py` (~200 lines)
- Test target: `planner.py` (163 lines, pure logic - no I/O, highly testable)
- Key algorithms: topological sort (Kahn's algorithm), frontmatter parsing, transitive dependents
- Current coverage: 0%
- Target coverage: 95%+ (straightforward logic)

**Implementation approach:**
1. Test _parse_frontmatter: Valid YAML, missing frontmatter, malformed, quoted strings, empty list, comments, integers
2. Test _compute_transitive_dependents: Linear chain, diamond pattern, no dependencies
3. Test _assign_waves (topological sort): Parametrize multiple DAG structures (linear, parallel, diamond), test circular dependency detection (should raise ValueError), test missing dependency reference filtering
4. Test load_plan: Valid plan directory, missing manifest.md (FileNotFoundError), missing segments/ directory (FileNotFoundError), no segments (ValueError)
5. Edge cases: Duplicate segment numbers, non-sequential numbering, gaps in numbering

**Alternatives ruled out:**
- Integration tests with real plan files: Rejected (creates fixtures in repo, prefer tmp_path)
- Mock pathlib: Rejected (planner.py is pure logic, test with real temp files)

**Pre-mortem risks:**
- Frontmatter parser divergence from PyYAML: Hand-rolled parser might miss edge cases. Mitigation: Test thoroughly, consider switching to PyYAML if bugs found.
- Circular dependency detection incomplete: Might miss complex cycles. Mitigation: Test multiple cycle patterns (A→B→C→A, self-reference).

**Segment-specific commands:**
- Build: `python -m py_compile scripts/orchestrate_v3/test_planner.py`
- Test (targeted): `pytest scripts/orchestrate_v3/test_planner.py -v`
- Test (regression): `pytest scripts/orchestrate_v3/ -v`
- Test (full gate): `pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3/planner.py --cov-report=term`

**Exit criteria:**
1. Targeted tests: test_planner.py passes (25+ tests)
2. Regression tests: All tests pass
3. Full build gate: No syntax errors
4. Full test gate: `pytest scripts/orchestrate_v3/ -v` all green
5. Self-review gate: All edge cases tested (circular deps, malformed frontmatter, missing files)
6. Scope verification gate: Only test_planner.py created, planner.py unchanged

**Risk factor:** 3/10

**Estimated complexity:** Medium

**Commit message:** `test(orchestrate): Add comprehensive planner.py tests (95% coverage)`

---

## Segment 8: Monitor.py comprehensive tests (Priority 2 coverage)
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Achieve 80%+ test coverage for monitor.py (HTTP endpoints, SSE streaming, control API).

**Depends on:** S2 (pytest), S4 (CSS static endpoint)

**Issues addressed:** Issue 5 (test coverage - Priority 2)

**Cycle budget:** 18 (High)

**Scope:** `scripts/orchestrate_v3/test_monitor.py` - comprehensive monitor tests

**Key files and context:**
- Create: `test_monitor.py` (~300 lines)
- Test target: `monitor.py` (439 lines, 9 HTTP endpoints, SSE streaming)
- Test strategy: Use aiohttp test client (no real HTTP server needed)
- Current coverage: 0%
- Target coverage: 80%+ (some SSE edge cases hard to test)

**Implementation approach:**
1. Setup aiohttp test client fixture with mock_state_db, tmp_path
2. Test GET endpoints: dashboard HTML, state JSON, prompt markdown, prompt 404, static CSS, static 404
3. Test POST /api/control: Skip action, retry action, kill action, invalid JSON (400), invalid seg_num (400), kill non-running (404)
4. Test SSE endpoints: /api/events streams events, /api/logs/{seg_id} streams logs, /api/logs/{seg_id}/attempt/{N} streams archived, mock log files in tmp_path
5. Test new endpoints (multi-tab feature): segment_attempts metadata, archived_log_sse streaming, segment_summary aggregates
6. Test error handling: 404 missing segments, 400 malformed requests, 500 database errors (mock state.get_segment to raise)

**Alternatives ruled out:**
- Real HTTP server: Rejected (slow, requires port allocation, aiohttp test client is faster)
- Mock aiohttp: Rejected (defeats purpose, want to test request routing)

**Pre-mortem risks:**
- SSE streaming hard to test: Reading from async iterator is tricky. Mitigation: Use aiohttp test client's streaming support, read limited lines.
- Race conditions in SSE: Client disconnect vs server write. Mitigation: Wrap in try/except ConnectionResetError.
- Temp file cleanup: Mock log files might leak. Mitigation: Use tmp_path fixture auto-cleanup.

**Segment-specific commands:**
- Build: `python -m py_compile scripts/orchestrate_v3/test_monitor.py`
- Test (targeted): `pytest scripts/orchestrate_v3/test_monitor.py -v`
- Test (regression): `pytest scripts/orchestrate_v3/ -v`
- Test (full gate): `pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3/monitor.py --cov-report=term`

**Exit criteria:**
1. Targeted tests: test_monitor.py passes (30+ tests covering all 9 endpoints)
2. Regression tests: All tests pass
3. Full build gate: No syntax errors
4. Full test gate: `pytest scripts/orchestrate_v3/ -v` all green
5. Self-review gate: All HTTP methods tested, error codes verified
6. Scope verification gate: Only test_monitor.py created, monitor.py unchanged

**Risk factor:** 5/10

**Estimated complexity:** High

**Commit message:** `test(orchestrate): Add comprehensive monitor.py tests (80% coverage)`

---

## Segment 9: Config, Notify, Streamparse utility tests (Priority 2 coverage)
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Achieve 80%+ combined coverage for config.py, notify.py, streamparse.py.

**Depends on:** S2 (pytest)

**Issues addressed:** Issue 5 (test coverage - Priority 2)

**Cycle budget:** 15 (Medium)

**Scope:** `scripts/orchestrate_v3/` - test_config.py, test_notify.py, test_streamparse.py

**Key files and context:**
- Create: `test_config.py` (~100 lines), `test_notify.py` (~80 lines), `test_streamparse.py` (~60 lines)
- Test targets: config.py (242 lines), notify.py (212 lines), streamparse.py (162 lines)
- Current coverage: 0%
- Target coverage: 80%+ each

**Implementation approach:**

**test_config.py:**
- Test _resolve_env_refs: With default, with existing var (use monkeypatch)
- Test RetryPolicy: Exponential backoff, with jitter, should_retry parametrized by status
- Test OrchestrateConfig loading from TOML

**test_notify.py:**
- Test Notifier class: segment_complete queues notification (mock httpx.AsyncClient)
- Test _send_ntfy: Mock POST request, verify headers/body
- Test notification batching, retry logic, deduplication

**test_streamparse.py:**
- Test _parse_stream_line_rich: Text delta, tool use, thinking, invalid JSON
- Test _extract_text_from_stream_line: Valid JSON, invalid JSON

**Alternatives ruled out:**
- Skip utility tests: Rejected (utilities are critical path)
- Integration tests only: Rejected (unit tests catch bugs faster)

**Pre-mortem risks:**
- Monkeypatch env vars affects other tests: Mitigation: pytest isolates fixtures.
- Mock httpx wrong: Mitigation: Follow httpx docs for mocking patterns.
- Jitter tests flaky: Random values might fail bounds. Mitigation: Run 100 iterations, check all pass.

**Segment-specific commands:**
- Build: `python -m py_compile scripts/orchestrate_v3/test_config.py scripts/orchestrate_v3/test_notify.py scripts/orchestrate_v3/test_streamparse.py`
- Test (targeted): `pytest scripts/orchestrate_v3/test_config.py scripts/orchestrate_v3/test_notify.py scripts/orchestrate_v3/test_streamparse.py -v`
- Test (regression): `pytest scripts/orchestrate_v3/ -v`
- Test (full gate): `pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3/{config,notify,streamparse}.py --cov-report=term`

**Exit criteria:**
1. Targeted tests: All 3 test files pass (40+ combined tests)
2. Regression tests: Full suite passes
3. Full build gate: No syntax errors
4. Full test gate: `pytest scripts/orchestrate_v3/ -v` all green
5. Self-review gate: All RetryPolicy methods tested, env resolution tested, stream parsing tested
6. Scope verification gate: Only 3 new test files created

**Risk factor:** 3/10

**Estimated complexity:** Medium

**Commit message:** `test(orchestrate): Add config, notify, streamparse tests (80% coverage each)`

---

## Segment 10: Extract Orchestrator coordinator class
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Extract high-level orchestration logic from __main__.py into Orchestrator class with dependency injection.

**Depends on:** S2-S9 (needs test coverage as safety net)

**Issues addressed:** Issue 1 (god module - Part 1)

**Cycle budget:** 20 (High)

**Scope:** `scripts/orchestrate_v3/` - new orchestrator.py, refactor __main__.py

**Key files and context:**
- Create: `orchestrator.py` (~300 lines: Orchestrator class, SignalHandler class)
- Modify: `__main__.py` (remove _orchestrate_inner, keep wave/segment execution for now)
- Test protection: 70%+ coverage from S2-S9 catches regressions
- High risk change: 8/10 - core orchestration loop

**Implementation approach:**
1. Create `orchestrator.py` with Orchestrator class:
   - Constructor takes dependencies: StateDB, OrchestrateConfig, Notifier, MonitorServer, WorktreePool
   - `async def run(segments, waves, max_wave, meta)` — main orchestration loop
   - Starts background tasks (heartbeat, notification worker)
   - Loops through waves calling _run_wave (still in __main__.py for now)
   - `async def cleanup()` — cleanup resources (pool, monitor, state)

2. Create SignalHandler class:
   - `shutting_down` asyncio.Event
   - `register_handlers()` — sets up SIGINT/SIGTERM handlers
   - `is_shutting_down()` — checks event

3. Refactor `__main__.py`:
   - Replace `_orchestrate_inner()` body with Orchestrator instantiation + run
   - Keep `_run_wave()`, `_run_one()` as module-level functions (move in S11)

4. Create `test_orchestrator.py`: Test Orchestrator.run with mocked wave execution, test SignalHandler shutdown, test cleanup on error

5. Update imports in `__main__.py`

**Alternatives ruled out:**
- Extract everything at once: Rejected (too risky, split into 2 segments)
- Keep procedural style: Rejected (doesn't solve testability)

**Pre-mortem risks:**
- Breaking wave execution: _run_wave still in __main__.py but might have implicit dependencies. Mitigation: Pass all needed params explicitly.
- Signal handler race: Shutdown during init might leave resources unreleased. Mitigation: Use try/finally in run().
- Test suite catches regressions: 70% coverage should catch most breaks. Mitigation: Run full test suite after refactor.

**Segment-specific commands:**
- Build: `python -m py_compile scripts/orchestrate_v3/orchestrator.py`
- Test (targeted): `pytest scripts/orchestrate_v3/test_orchestrator.py -v`
- Test (regression): `pytest scripts/orchestrate_v3/ -v` (CRITICAL: All existing tests must still pass)
- Test (full gate): `pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3 --cov-report=term`

**Exit criteria:**
1. Targeted tests: test_orchestrator.py passes (10+ tests)
2. Regression tests: **All existing tests still pass** (no behavior changes)
3. Full build gate: No syntax errors
4. Full test gate: Full pytest suite passes, coverage ≥70%
5. Self-review gate: All resource cleanup in finally blocks
6. Scope verification gate: Only orchestrator.py, test_orchestrator.py (new), __main__.py (refactored)

**Risk factor:** 8/10

**Estimated complexity:** High

**Commit message:** `refactor(orchestrate): Extract Orchestrator coordinator class with DI`

---

## Segment 11: Extract WaveRunner and SegmentExecutor classes
> **Execution method:** Launch as an `iterative-builder` subagent.

**Goal:** Complete god module decomposition by extracting WaveRunner and SegmentExecutor classes, removing procedural _run_wave and _run_one functions.

**Depends on:** S10 (needs Orchestrator class foundation)

**Issues addressed:** Issue 1 (god module - Part 2, completion)

**Cycle budget:** 20 (High)

**Scope:** `scripts/orchestrate_v3/` - new wave_runner.py, segment_executor.py, refactor orchestrator.py and __main__.py

**Key files and context:**
- Create: `wave_runner.py` (~250 lines: WaveRunner class)
- Create: `segment_executor.py` (~150 lines: SegmentExecutor class)
- Modify: `orchestrator.py` (use WaveRunner instead of calling __main__._run_wave)
- Modify: `__main__.py` (remove _run_wave, _run_one - now in classes)
- Create: `test_wave_runner.py`, `test_segment_executor.py`
- Final __main__.py size: ~400 lines (down from 1,285)

**Implementation approach:**
1. Create `segment_executor.py` with SegmentExecutor class:
   - `async def execute(segment, worktree_path)` → (status, summary)
   - Encapsulates retry loop with circuit breaker, retry policy
   - Calls run_segment() from runner.py

2. Create `wave_runner.py` with WaveRunner class:
   - `async def execute(wave_num, segments, shutting_down)` → list of (seg_num, status)
   - Manages semaphore for bounded parallelism
   - `_run_one_segment()` inner function checks dependencies, skips, executes
   - `async def _execute_with_worktree(seg)` acquires worktree, executes, merges
   - `_validate_dependencies()`, `_merge_worktree_changes()` helper methods

3. Update `orchestrator.py`:
   - Constructor creates WaveRunner instance
   - `run()` delegates to `wave_runner.execute()` instead of calling __main__._run_wave

4. Update `__main__.py`:
   - Remove `_run_wave()` (228 lines)
   - Remove `_run_one()` (142 lines)
   - Remove `_merge_worktree_changes()` (93 lines)
   - Keep: CLI parsing, _run_gate, _claude_summarise, heartbeat/notification workers
   - Final size: ~400 lines

5. Write tests: test_segment_executor.py (retry logic, circuit breaker), test_wave_runner.py (parallel execution, dependencies, worktree)

**Alternatives ruled out:**
- Keep _run_wave as-is: Rejected (doesn't solve testability)
- Merge WaveRunner into Orchestrator: Rejected (Orchestrator would still be too large)

**Pre-mortem risks:**
- Retry loop logic changed: Subtle bugs in retry/circuit breaker. Mitigation: Comprehensive tests, compare to original.
- Worktree merge breaks: Git operations tricky. Mitigation: Copy merge logic exactly, reuse tests.
- Parallel execution semantics changed: asyncio.gather might behave differently. Mitigation: Integration tests with real segments.
- __main__.py too empty: Thin CLI layer is acceptable. Mitigation: Good architecture pattern.

**Segment-specific commands:**
- Build: `python -m py_compile scripts/orchestrate_v3/wave_runner.py scripts/orchestrate_v3/segment_executor.py`
- Test (targeted): `pytest scripts/orchestrate_v3/test_wave_runner.py scripts/orchestrate_v3/test_segment_executor.py -v`
- Test (regression): `pytest scripts/orchestrate_v3/ -v` (ALL tests must pass)
- Test (full gate): `pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3 --cov-report=term`

**Exit criteria:**
1. Targeted tests: test_wave_runner.py and test_segment_executor.py pass (30+ combined)
2. Regression tests: **All existing tests still pass** (behavior unchanged)
3. Full build gate: No syntax errors
4. Full test gate: Full pytest suite passes, coverage ≥70%
5. Self-review gate: __main__.py reduced to ~400 lines, all long functions extracted
6. Scope verification gate: 2 new modules, orchestrator.py and __main__.py refactored, all tests pass

**Risk factor:** 8/10

**Estimated complexity:** High

**Commit message:** `refactor(orchestrate): Extract WaveRunner and SegmentExecutor classes`

---

## Execution Instructions

**To execute this plan, use the `/orchestrate` skill:**

```bash
# From project root
/orchestrate .claude/plans/orchestrate-v3-quality-2026-03-12/
```

The orchestrator will:
1. Execute each segment in dependency order (S1 → S2 → S3∥S4 → S5∥S6∥S7 → S8∥S9 → S10 → S11)
2. Launch each segment as an `iterative-builder` subagent with the full segment brief
3. Verify exit criteria (5 gates: targeted tests, regression tests, build, full test, self-review)
4. Automatically parallelize independent segments (S3∥S4, S5∥S6∥S7, S8∥S9)

**Do NOT implement segments directly — always delegate to iterative-builder subagents.**

After all segments complete, run:
```bash
/deep-verify
```

This will verify:
- Test coverage ≥70% achieved
- God module (__main__.py) reduced from 1,285 → ~400 lines
- All file operations use FileOps utility
- CSS extracted to external stylesheet
- All exit criteria satisfied

If verification finds gaps, re-enter `/deep-plan` on unresolved items.

---

## Test Commands Reference

```bash
# Targeted tests (per segment)
pytest scripts/orchestrate_v3/test_<module>.py -v

# Regression tests (all existing tests)
pytest scripts/orchestrate_v3/ -v

# Full build gate
python -m py_compile scripts/orchestrate_v3/*.py

# Full test gate with coverage
pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3 --cov-report=term-missing --cov-report=html

# Coverage threshold check (enforces 70%)
pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3 --cov-fail-under=70

# View detailed coverage report
open htmlcov/index.html
```

**Expected Final Coverage: 70-75%**
- state.py: 90%+
- runner.py: 85%+
- planner.py: 95%+
- monitor.py: 80%+
- config.py, notify.py, streamparse.py: 80%+ each
- orchestrator.py, wave_runner.py, segment_executor.py: 80%+ each
- fileops.py: 90%+
- __main__.py: 60% (CLI layer, hard to test - acceptable)

---

## Total Estimated Scope

- **Segment count:** 11
- **Complexity distribution:**
  - Low (3): S1, S4, S9
  - Medium (5): S2, S3, S5, S7, S9
  - High (3): S6, S10, S11
- **Risk distribution:**
  - Low 2-3/10 (5): S1, S3, S4, S7, S9
  - Medium 4-6/10 (4): S2, S5, S6, S8
  - High 8/10 (2): S10, S11
- **Estimated cycles:** 100-120 (with parallelization)
- **Risk mitigation:** High-risk segments (S10, S11) protected by 70% test coverage from S2-S9
- **Caveats:**
  - Segments S10-S11 are architectural refactoring - expect 2-3 iteration cycles for refinement
  - Test coverage might plateau at 65-70% (some code hard to test - acceptable)
  - Frontend CSS extraction (S4) is cosmetic - can be deferred if time-constrained

---

**Plan Status:** Ready for execution via `/orchestrate`
