---
segment: 17
title: Final push to 90%
depends_on: [10, 11, 12, 13, 14, 15, 16]
risk: 4
complexity: Medium
cycle_budget: 10
estimated_lines: varies
---

# Segment 17: Final Targeted Push to 90%

## Context

**After S01-S16:** Expected ~88-89% workspace coverage

**This segment:** Targeted gap-filling based on actual results

## Goal

Analyze actual coverage after S01-S16 and fill remaining gaps to reach 90%.

## Implementation Plan

1. Run full workspace coverage:
   ```bash
   cargo llvm-cov --workspace --summary-only
   ```

2. Generate HTML reports for each crate:
   ```bash
   cargo llvm-cov --workspace --html
   ```

3. Identify crates still below targets:
   - Check if any crate fell short of its target
   - Prioritize crates with largest workspace impact

4. Create targeted tests for identified gaps

5. Verify final workspace coverage ≥90%

## Exit Criteria

1. [ ] Workspace coverage ≥90%
2. [ ] All library crates ≥85%
3. [ ] prb-tui ≥65%
4. [ ] Critical crates ≥95%
5. [ ] All tests pass
6. [ ] Coverage report generated
7. [ ] PR created with summary

## Success Metrics

- Workspace: 88% → 90%+
- All segment targets met or justified
- Comprehensive coverage report documenting acceptable gaps
