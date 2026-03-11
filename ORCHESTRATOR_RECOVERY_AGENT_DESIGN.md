# Workspace Recovery Agent Design

## Problem Statement

When segments end with PARTIAL or BLOCKED status, they often leave workspace in a broken state that cascades to dependent segments. Current orchestrator has no automated recovery mechanism.

**Evidence from phase3-tui-evolution:**
- S10 left incomplete stubs → S12/S18 both correctly implemented features but couldn't compile
- S12 builder report: "All S12 exit criteria met, blocked by 14 pre-existing S10 errors"
- S18 builder report: "My code has ZERO compilation errors, blocked by S10 stubs"

## Proposed Solution: Workspace Health Agent

### When to Trigger

Run after each wave IF any segments in wave ended as PARTIAL/BLOCKED:

```python
# In __main__.py, after wave completes:
if any(status in ('partial', 'blocked') for _, status in results):
    health_report = await run_workspace_health_agent(wave_num, results, log_dir)

    if health_report.can_auto_fix:
        await auto_fix_workspace(health_report)
        await retry_affected_segments(health_report.retryable_segments)
```

### Agent Capabilities

**1. Diagnostic Phase**
```python
async def diagnose_workspace(wave_results):
    """Run comprehensive workspace health check."""

    checks = []

    # Build check
    build_result = await run_command("cargo check --workspace --message-format=json")
    checks.append(parse_cargo_errors(build_result))

    # Clippy check
    clippy_result = await run_command("cargo clippy --workspace --message-format=json")
    checks.append(parse_clippy_warnings(clippy_result))

    # Test check
    test_result = await run_command("cargo test --no-run")
    checks.append(parse_test_errors(test_result))

    # Git status check
    git_status = await run_command("git status --porcelain")
    checks.append(parse_unstaged_changes(git_status))

    return WorkspaceHealthReport(checks)
```

**2. Root Cause Analysis**
```python
class WorkspaceHealthReport:
    def analyze_cascade_failures(self, segment_reports):
        """Determine if failures are cascading from one segment."""

        error_patterns = defaultdict(list)

        for seg_num, report in segment_reports.items():
            if seg.status in ('partial', 'blocked'):
                # Extract compiler errors
                errors = parse_builder_report_errors(report)

                for error in errors:
                    # Group by file and error type
                    key = (error.file, error.error_code)
                    error_patterns[key].append(seg_num)

        # Find common errors affecting multiple segments
        cascading_errors = {
            error: segs for error, segs in error_patterns.items()
            if len(segs) > 1
        }

        if cascading_errors:
            # Find root segment (earliest in dependency chain)
            root_segment = min(
                seg for segs in cascading_errors.values() for seg in segs
            )
            return CascadeFailure(root_segment, cascading_errors)

        return None
```

**3. Auto-Fix Strategies**

```python
class AutoFixStrategy:
    """Automated fixes for common issues."""

    async def fix_incomplete_stubs(self, cascade):
        """Detect and remove incomplete function stubs."""

        root_seg_files = get_segment_modified_files(cascade.root_segment)

        for file in root_seg_files:
            # Find functions with todo!() or unimplemented!()
            stub_functions = find_stub_functions(file)

            if stub_functions:
                strategy = "revert"  # or "complete"

                if strategy == "revert":
                    # Revert file to pre-segment state
                    await run_command(f"git checkout HEAD~1 -- {file}")

                elif strategy == "complete":
                    # Use LLM to complete stubs
                    completed = await llm_complete_stubs(file, stub_functions)
                    write_file(file, completed)

        return FixResult(strategy, files_fixed=root_seg_files)

    async def fix_missing_fields(self, error):
        """Add missing struct fields across test files."""

        if "missing field" in error.message:
            field_name = extract_field_name(error.message)
            struct_name = extract_struct_name(error.message)

            # Find all instances of struct initialization
            files = grep_recursive(f"{struct_name} {{")

            for file in files:
                await add_field_to_initializers(
                    file, struct_name, field_name, default_value="None"
                )

            return FixResult("add_field", files_fixed=files)

    async def fix_import_errors(self, errors):
        """Remove unused imports, add missing imports."""

        for error in errors:
            if "unused import" in error.message:
                await remove_import(error.file, error.import_name)

            elif "unresolved import" in error.message:
                # Suggest adding dependency to Cargo.toml
                return FixResult("needs_dependency", dep=error.crate_name)

        return FixResult("fixed_imports")
```

**4. Segment Retry Logic**

```python
async def retry_affected_segments(health_report, max_retry_rounds=2):
    """Retry segments that were blocked by fixed issues."""

    retryable = []

    for seg_num in health_report.cascade_victims:
        seg = await state.get_segment(seg_num)

        # Only retry if segment code was correct
        if "My code has ZERO compilation errors" in seg.log:
            retryable.append(seg_num)

        elif "exit criteria met" in seg.log.lower():
            retryable.append(seg_num)

    if retryable:
        log.info("Retrying %d segments after workspace fix: %s",
                 len(retryable), retryable)

        for seg_num in retryable:
            await state.reset_for_retry(seg_num)

        # Re-run gate check
        gate_ok, _ = await _run_gate(config, log_dir, wave_num)

        if gate_ok:
            # Retry segments in parallel
            retry_results = await _run_wave(
                wave_num,
                [s for s in segments if s.num in retryable],
                config, state, notifier, log_dir, shutting_down, pool
            )

            return retry_results

    return []
```

## Integration Points

### 1. After Wave Completion

```python
# In __main__.py _run_wave(), after results = await asyncio.gather(*tasks):

# Check for problematic statuses
problematic = [
    (num, status) for num, status in results
    if status in ('partial', 'blocked')
]

if problematic and config.enable_recovery_agent:
    log.info("Wave %d: %d segments need recovery, launching health agent",
             wave_num, len(problematic))

    health_report = await run_workspace_health_agent(
        wave_num, problematic, segments, state, log_dir
    )

    if health_report.auto_fixable:
        log.info("Applying automatic fixes: %s", health_report.fixes)
        fix_results = await apply_auto_fixes(health_report)

        # Retry affected segments
        retry_results = await retry_affected_segments(
            health_report.retryable_segments,
            config, state, log_dir, pool
        )

        # Update results with retry outcomes
        results = merge_results(results, retry_results)
```

### 2. Before Gate (Optional Preflight)

```python
# In __main__.py before _run_gate():

if config.preflight_check_enabled:
    preflight_ok, errors = await preflight_workspace_check(log_dir)

    if not preflight_ok:
        log.warning("Preflight check failed: %d errors", len(errors))

        # Don't run segments if workspace is already broken
        await notifier.error(f"Preflight failed: {errors[:3]}")
        return []  # Skip wave execution
```

## Configuration

Add to `orchestrate.toml`:

```toml
[recovery]
enabled = true
auto_fix = true  # Attempt automatic fixes
max_retry_rounds = 2
preflight_check = true  # Check workspace health before wave

# What to auto-fix
fix_incomplete_stubs = true
fix_missing_fields = true
fix_unused_imports = true
revert_on_cascade = true  # Revert root segment if it cascades failures
```

## Detailed Workflow

```
Wave N completes with PARTIAL/BLOCKED segments
         ↓
Launch Workspace Health Agent
         ↓
┌─────────────────────────────────────┐
│ 1. Diagnose                         │
│    - cargo check (find errors)      │
│    - cargo clippy (find warnings)   │
│    - cargo test --no-run           │
│    - git status (uncommitted work) │
└────────────┬────────────────────────┘
             ↓
┌─────────────────────────────────────┐
│ 2. Analyze                          │
│    - Group errors by file/type      │
│    - Find cascade root segment      │
│    - Identify victim segments       │
│    - Classify errors (fixable?)     │
└────────────┬────────────────────────┘
             ↓
┌─────────────────────────────────────┐
│ 3. Auto-fix (if enabled)            │
│    - Revert incomplete stubs        │
│    - Add missing struct fields      │
│    - Remove unused imports          │
│    - Fix type mismatches            │
└────────────┬────────────────────────┘
             ↓
┌─────────────────────────────────────┐
│ 4. Verify Fix                       │
│    - Re-run cargo check              │
│    - Confirm errors cleared         │
│    - Run gate if configured         │
└────────────┬────────────────────────┘
             ↓
┌─────────────────────────────────────┐
│ 5. Retry Affected Segments          │
│    - Reset victim segments to pending│
│    - Re-run in parallel             │
│    - Update wave results            │
└─────────────────────────────────────┘
```

## Benefits

1. **Automatic Recovery**: Fixes common cascade failures without manual intervention
2. **Smart Retry**: Only retries segments whose code was correct
3. **Prevents Wasted Work**: Stops cascading failures before they affect more segments
4. **Faster Iteration**: No need to restart orchestrator for fixable issues
5. **Better Observability**: Health reports show root cause of failures

## Limitations

1. **Complex Fixes**: Can't auto-fix architecture issues or logic errors
2. **Timing**: Adds overhead between waves (but only when needed)
3. **False Positives**: Might revert code that was actually correct
4. **Git Conflicts**: Auto-fixes might conflict with manual changes

## Fallback Strategy

If auto-fix fails or is disabled:

```python
if not health_report.auto_fixable or not config.auto_fix:
    # Generate human-readable recovery instructions
    recovery_doc = generate_recovery_guide(health_report)

    # Write to file
    write_file(
        log_dir / f"wave-{wave_num}-recovery-guide.md",
        recovery_doc
    )

    # Send notification
    await notifier.manual_intervention_required(
        wave_num,
        health_report.summary,
        recovery_doc_path
    )

    # Pause orchestration
    log.warning("Manual intervention required, pausing")
    shutting_down.set()
```

## Example Recovery Guide

```markdown
# Wave 3 Recovery Guide

## Summary
3 segments ended as PARTIAL/BLOCKED due to cascading failures from S10.

## Root Cause
S10 (Export & Clipboard) left incomplete function stubs in `app.rs`:
- `copy_hex_dump()` - calls `unimplemented!()`
- `copy_decoded_tree()` - calls `todo!()`
- Missing `Payload::as_bytes()` method

## Affected Segments
- S12 (AI Explain Panel): **Code correct**, blocked by S10 errors
- S18 (Live Capture UI): **Code correct**, blocked by S10 errors

## Recommended Action
**Option A (Quick)**: Revert S10 changes
```bash
git revert <S10-commit-hash>
python -m scripts.orchestrate_v2 retry 12
python -m scripts.orchestrate_v2 retry 18
```

**Option B (Complete)**: Fix S10 stubs
```bash
# Edit app.rs lines 1149-1188
# Implement copy_hex_dump, copy_decoded_tree
# Add Payload::as_bytes() method to prb-core
cargo check --workspace  # verify
python -m scripts.orchestrate_v2 retry 12
python -m scripts.orchestrate_v2 retry 18
```

## Prevention
- Add preflight_check to catch workspace errors before wave starts
- Set fix_incomplete_stubs = true to auto-remove stubs
```

## Implementation Priority

**Phase 1 (Immediate):**
- [x] CLAUDECODE env var fix (DONE)
- [ ] Basic workspace health check (cargo check + error parsing)
- [ ] Manual recovery guide generation

**Phase 2 (Short-term):**
- [ ] Auto-fix for common patterns (unused imports, missing fields)
- [ ] Cascade detection algorithm
- [ ] Smart retry logic

**Phase 3 (Long-term):**
- [ ] LLM-powered stub completion
- [ ] Preflight checks before waves
- [ ] Integration with monitor dashboard
