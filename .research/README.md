# Test Utilities Research - March 13, 2026

## Overview

This directory contains comprehensive research on Rust test utilities and best practices for eliminating 680 LOC of duplicated test event builder code across 42 files in the probe project.

## Documents

### 1. test-utilities-research-2026-03-13.md
**Main research deliverable** - Comprehensive analysis of:
- **Source 3**: 8 existing Rust crates (rstest, proptest, derive_builder, etc.)
- **Source 4**: 6 external best practices from official Rust docs, popular projects, and authoritative sources

**Key sections**:
- Crate-by-crate analysis with maintenance status, license, dependencies, stack fit
- Recommendations: Adopt / Adapt / Reject for each solution
- Best practices from Rust Book, API Guidelines, tokio, serde, tracing
- Anti-patterns from "Rust for Rustaceans" (Jon Gjengset)
- 14 authoritative references

**Read this first** for comprehensive context.

### 2. DECISION_MATRIX.md
**Quick reference guide** - Implementation roadmap:
- Priority matrix: High/Medium/Low impact vs effort
- 5-phase implementation plan with timelines and success metrics
- Cost-benefit analysis (80/20 rule: prb-test-utils solves 90% of problem)
- Visual impact vs effort chart
- Key insights summary

**Read this second** for actionable decisions.

### 3. BEFORE_AFTER_EXAMPLES.md
**Concrete migration guide** - Shows:
- 4 real examples of current duplication (29, 19, 20, 20 lines each)
- Proposed prb-test-utils crate structure (full implementation code)
- After-migration examples (95% LOC reduction in some cases)
- Migration checklist for 42 files
- Semi-automated migration script
- ROI calculation: 200%+ over project lifetime

**Read this third** for implementation details.

## Executive Summary

### The Problem
- **42 test files** with duplicated event builders
- **680 LOC** of copy-pasted code
- **3 naming conventions** (make_test_event, sample_event, create_test_event)
- **2 implementation patterns** (builder vs struct literal)
- Maintenance burden: changes require updating 42 files

### The Solution (High Priority)
Create **`prb-test-utils`** crate with:
1. `event()` - minimal default event
2. `grpc_event()`, `zmq_event()`, etc. - protocol-specific presets
3. `event_builder()` - pre-configured DebugEventBuilder with test defaults
4. Standardized naming and consistent implementation

**Effort**: 6-10 hours (1-2 days)  
**Impact**: Remove 600+ LOC, single source of truth  
**ROI**: Break-even after 2-3 test changes, 200%+ over lifetime

### Recommended Adoptions

| Solution | Priority | Use Case | When |
|----------|----------|----------|------|
| **prb-test-utils crate** | HIGH | Eliminate duplication | Phase 1 (immediate) |
| **rstest OR test-case** | MEDIUM | Parameterized tests | Phase 2 (after dedupe) |
| **proptest strategies** | MEDIUM | Edge case coverage | Phase 3 (expand testing) |
| **fake** | LOW | Realistic test data | Phase 4 (polish) |
| **arbitrary** | DEFER | Fuzzing | Phase 5 (future) |

### Rejected Solutions
- **derive_builder**: Manual builder is better for domain logic
- **typed-builder**: Overkill for test utilities
- **quickcheck**: Use proptest instead (already in project)

## Key Findings

1. **The problem is NOT the builder quality** - DebugEventBuilder in prb-core is well-designed. The issue is inconsistent use across tests.

2. **Simple solution, big impact** - A 100-line crate eliminates 600+ LOC. Classic 80/20 rule.

3. **Learn from the ecosystem** - tokio, serde, tracing all have separate test utility crates. This is the proven pattern.

4. **Avoid over-engineering** - Don't replace the manual builder with macros. Domain-specific methods matter.

5. **Property-based testing is underutilized** - proptest is already in the project but not used extensively.

## Implementation Plan

### Phase 1: Eliminate Duplication (HIGH ROI)
**Timeline**: 1-2 days | **Effort**: 6-10 hours

1. Create `crates/prb-test-utils/` with 3 modules:
   - `fixtures.rs` - Protocol-specific presets
   - `builders.rs` - Pre-configured builder helpers
   - `lib.rs` - Public API
2. Add to workspace `[dev-dependencies]`
3. Migrate 42 test files
4. Remove 600+ LOC of duplication

**Success**: All tests pass, single source of truth

### Phase 2: Improve Organization (MEDIUM ROI)
**Timeline**: 1 day | **Effort**: 2-3 hours

- Add rstest OR test-case for parameterized tests
- Migrate table-driven tests

**Success**: Cleaner test files, easier to add cases

### Phase 3: Expand Coverage (MEDIUM-HIGH ROI)
**Timeline**: 2-3 days | **Effort**: 4-8 hours

- Add proptest strategies to prb-test-utils
- Property-based tests for serialization, query eval, rendering

**Success**: Edge cases discovered and fixed

### Phase 4: Polish (LOW ROI)
**Timeline**: 1 day | **Effort**: 1-2 hours

- Add fake crate for realistic IPs/ports

**Success**: Integration tests more readable

### Phase 5: Fuzzing (FUTURE)
**Timeline**: TBD | **Effort**: 8+ hours

- Add arbitrary + cargo-fuzz
- Fuzz parsers and decoders

**Success**: Fuzzing infrastructure in place

## Quick Start

To implement Phase 1 immediately:

1. **Read BEFORE_AFTER_EXAMPLES.md** for full code
2. **Copy the prb-test-utils structure** (fixtures.rs, builders.rs, lib.rs)
3. **Add to workspace** Cargo.toml:
   ```toml
   members = [
       # ... existing members
       "crates/prb-test-utils",
   ]
   ```
4. **Add dev-dependency** to each crate's Cargo.toml:
   ```toml
   [dev-dependencies]
   prb-test-utils = { path = "../prb-test-utils" }
   ```
5. **Migrate one test file** as proof-of-concept
6. **Run tests**: `cargo test`
7. **Repeat for remaining 41 files**

## References

See test-utilities-research-2026-03-13.md for full bibliography (14 sources):
- Official Rust docs (Book, API Guidelines, Patterns)
- Popular crates (rstest, proptest, derive_builder, etc.)
- Authoritative books ("Rust for Rustaceans" - Jon Gjengset)
- Major projects (tokio-test, serde_test)
- Expert blogs (Luca Palmieri, Hypothesis)

## Metrics

| Metric | Current | After Phase 1 | Improvement |
|--------|---------|---------------|-------------|
| LOC in tests | 680 | 80 | -88% |
| Implementations | 42 | 1 | -98% |
| Naming conventions | 3 | 1 | -67% |
| Maintenance burden | High | Low | -90% |

## Questions?

For implementation questions:
1. Check BEFORE_AFTER_EXAMPLES.md for code samples
2. Check DECISION_MATRIX.md for prioritization
3. Check test-utilities-research-2026-03-13.md for rationale

---

**Generated**: 2026-03-13  
**Research Context**: 680 LOC duplication across 42 test files  
**Recommendation**: Create prb-test-utils crate (6-10h effort, 90% impact)
