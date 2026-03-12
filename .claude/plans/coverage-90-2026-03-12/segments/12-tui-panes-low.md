---
segment: 12
title: TUI panes (low-coverage) to 40%
depends_on: [10]
risk: 3
complexity: Medium
cycle_budget: 10
estimated_lines: ~300 test lines
---

# Segment 12: TUI Panes Low-Coverage to 40%

## Context

**Target panes (currently near-zero):**
- `waterfall.rs` - 0.45% → 40% (452 lines uncovered)
- `conversation_list.rs` - 0.61% → 40% (321 lines uncovered)
- `ai_panel.rs` - 37.94% → 55% (107 lines uncovered)
- `trace_correlation.rs` - 35.92% → 50% (149 lines uncovered)

## Goal

Extract and test business logic from rendering-heavy panes.

## Implementation Plan

Focus on data structures, filtering, sorting - NOT pixel-level rendering.

~40 new tests targeting calculable logic.
