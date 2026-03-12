---
segment: 06
title: Coverage Analysis
depends_on: [3]
risk: 2
complexity: Low
cycle_budget: 3
estimated_lines: Analysis output only
---

# Segment 06: Coverage Analysis

## Context

Analyze current test coverage to identify gaps and prioritize coverage improvement work. This segment generates reports and identifies which crates/modules need additional tests to reach the 80% target.

## Current State

- 904 test functions across 78 test files
- No coverage tracking or analysis
- Unknown actual coverage percentage
- Unknown coverage gaps

## Goal

Generate comprehensive coverage analysis to guide Segment 07's test writing efforts.

## Exit Criteria

1. [ ] cargo-llvm-cov installed
2. [ ] Coverage report generated (HTML + LCOV)
3. [ ] Per-crate coverage percentages documented
4. [ ] Modules below 80% identified and prioritized
5. [ ] Coverage gaps documented in segment handoff
6. [ ] HTML report reviewed manually
7. [ ] Analysis results committed to plan directory

## Implementation Plan

### Step 1: Install Coverage Tool

```bash
cargo install cargo-llvm-cov --locked
```

### Step 2: Generate Coverage Report

```bash
# Generate HTML report
cargo llvm-cov --workspace --html

# Generate LCOV for analysis
cargo llvm-cov --workspace --lcov --output-path lcov.info

# Open HTML report
open target/llvm-cov/html/index.html
```

### Step 3: Analyze Per-Crate Coverage

```bash
# Get summary by crate
cargo llvm-cov --workspace --summary-only
```

Expected output format:
```
prb-core: XX.X%
prb-cli: XX.X%
prb-pcap: XX.X%
...
TOTAL: XX.X%
```

### Step 4: Identify Priority Gaps

Review HTML report and identify:

1. **Critical crates** (must be 90%+):
   - prb-core - foundation types
   - prb-pcap - core pipeline

2. **High priority** (must be 85%+):
   - prb-grpc - gRPC decoder
   - prb-zmq - ZMTP decoder
   - prb-dds - DDS decoder
   - prb-storage - data persistence

3. **Standard priority** (must be 80%+):
   - All other crates

### Step 5: Document Findings

Create handoff document: `handoff/S06-coverage-analysis.md`

Template:
```markdown
# Coverage Analysis Results

## Overall Coverage

Current: XX.X%
Target: 80.0%
Gap: XX.X%

## Per-Crate Breakdown

### Below Target (<80%)

| Crate | Current | Target | Gap | Priority |
|-------|---------|--------|-----|----------|
| prb-xxx | XX% | 80% | XX% | High |
...

### Meeting Target (≥80%)

| Crate | Current | Notes |
|-------|---------|-------|
| prb-yyy | XX% | ✅ |
...

## Specific Gaps by Module

### prb-core
- [ ] src/engine.rs - Lines 45-78 uncovered (error handling)
- [ ] src/conversation.rs - Lines 120-145 (edge cases)
...

### prb-pcap
- [ ] src/tls/decrypt.rs - Lines 200-250 (cipher suites)
...

## Recommendations for S07

1. Start with prb-core (foundation)
2. Then prb-pcap (critical path)
3. Then protocol decoders (prb-grpc, prb-zmq, prb-dds)
4. Finally remaining crates

Estimated effort: XX tests needed across XX files
```

## Files to Create

- `handoff/S06-coverage-analysis.md` (analysis results)
- `target/llvm-cov/html/` (generated report, not committed)
- `lcov.info` (coverage data, not committed)

## Test Plan

1. Install cargo-llvm-cov
2. Generate coverage report:
   ```bash
   cargo llvm-cov --workspace --html
   ```
3. Review HTML report in browser
4. Identify crates below 80%
5. Document specific uncovered lines/functions
6. Create prioritized list for S07
7. Commit handoff document to plan

## Blocked By

- Segment 03 (Main CI Workflow) - need CI coverage job as reference

## Blocks

- Segment 07 (Fill Coverage Gaps) - this analysis guides that work

## Success Metrics

- Coverage report generated successfully
- All crates analyzed
- Gaps identified and documented
- Priority list created for S07
- Handoff document complete

## Notes

- This is purely analysis - no code changes
- Focus on identifying easy wins (uncovered error paths, edge cases)
- Some code may be legitimately untestable (unsafe blocks, platform-specific)
- Consider excluding benches from coverage (they're perf tests, not unit tests)
- Large files with low coverage are high-value targets
