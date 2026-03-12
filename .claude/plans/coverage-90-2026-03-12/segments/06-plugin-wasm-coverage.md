---
segment: 06
title: prb-plugin-wasm to 85%
depends_on: []
risk: 4
complexity: High
cycle_budget: 10
estimated_lines: ~250 test lines
---

# Segment 06: prb-plugin-wasm Coverage to 85%

## Context

**Current:** 66.44%
**Target:** 85%
**Gap:** +18.56 percentage points

**Gaps:**
- `src/loader.rs` - 33.06% (48 lines uncovered)
- `src/adapter.rs` - 78.09% (96 lines uncovered)
- `src/runtime.rs` - 100% ✅

## Goal

Test WASM module loading, instantiation, and error paths.

## Implementation Plan

Create test WASM modules, test loading/validation/execution errors.

~40 new tests targeting loader and adapter edge cases.
