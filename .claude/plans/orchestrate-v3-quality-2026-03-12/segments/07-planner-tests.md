---
segment: 7
title: "Planner.py comprehensive tests (Priority 1)"
depends_on: [2]
cycle_budget: 12
risk: 3
complexity: "Medium"
commit_message: "test(orchestrate): Add comprehensive planner.py tests (95% coverage)"
---

# Segment 7: Planner.py comprehensive tests

## Goal

Achieve 95%+ test coverage for planner.py (frontmatter parsing, wave assignment, dependency resolution).

## Context

planner.py (163 lines, pure logic) is completely untested. Contains critical algorithms: topological sort (Kahn's algorithm), frontmatter parsing, transitive dependents calculation.

## Scope

- **Create:** `test_planner.py` (~200 lines)
- **Target coverage:** 95%+ (straightforward logic)

## Implementation Approach

1. **Test _parse_frontmatter:**
   ```python
   def test_parse_frontmatter_valid_yaml(tmp_path):
       md = tmp_path / "test.md"
       md.write_text("""---
   segment: 5
   title: "Test Segment"
   depends_on: [1, 2, 3]
   cycle_budget: 20
   ---
   # Content
   """)
       fm = _parse_frontmatter(md)
       assert fm["segment"] == 5
       assert fm["depends_on"] == [1, 2, 3]
   ```
   - Test valid frontmatter, missing frontmatter, malformed YAML
   - Test quoted strings, empty lists, comments, integers

2. **Test _compute_transitive_dependents:**
   - Linear chain: S1 → S2 → S3
   - Diamond: S1 → S2, S1 → S3, S2 → S4, S3 → S4
   - No dependencies (all wave 1)

3. **Test _assign_waves (topological sort):**
   ```python
   @pytest.mark.parametrize("segments,expected_waves", [
       # Linear: S1 → S2 → S3
       ([Segment(1, "s1", "S1"),
         Segment(2, "s2", "S2", depends_on=[1]),
         Segment(3, "s3", "S3", depends_on=[2])],
        {1: 1, 2: 2, 3: 3}),
       # Parallel: S1, S2 independent, S3 depends on both
       ([Segment(1, "s1", "S1"),
         Segment(2, "s2", "S2"),
         Segment(3, "s3", "S3", depends_on=[1, 2])],
        {1: 1, 2: 1, 3: 2}),
   ])
   def test_assign_waves_correct_topology(segments, expected_waves):
       _assign_waves(segments)
       for seg in segments:
           assert seg.wave == expected_waves[seg.num]
   ```
   - Test circular dependency detection (should raise ValueError)
   - Test missing dependency filtering

4. **Test load_plan:**
   - Valid plan directory
   - Missing manifest.md (FileNotFoundError)
   - Missing segments/ (FileNotFoundError)
   - No segments (ValueError)

5. **Edge cases:**
   - Duplicate segment numbers
   - Non-sequential numbering
   - Gaps in numbering

## Pre-Mortem Risks

- **Frontmatter parser divergence:** Hand-rolled parser might miss edge cases
  - Mitigation: Test thoroughly, consider PyYAML if bugs found
- **Circular dependency detection incomplete:** Might miss complex cycles
  - Mitigation: Test multiple cycle patterns

## Exit Criteria

1. **Targeted tests:** test_planner.py passes (25+ tests)
2. **Regression tests:** All tests pass
3. **Full build gate:** No syntax errors
4. **Full test gate:** All tests pass
5. **Self-review gate:** All edge cases tested
6. **Scope verification gate:** Only test_planner.py created

## Commands

```bash
# Build
python -m py_compile scripts/orchestrate_v3/test_planner.py

# Test (targeted)
pytest scripts/orchestrate_v3/test_planner.py -v

# Test (regression)
pytest scripts/orchestrate_v3/ -v

# Test (full gate)
pytest scripts/orchestrate_v3/ --cov=scripts/orchestrate_v3/planner.py --cov-report=term
```
