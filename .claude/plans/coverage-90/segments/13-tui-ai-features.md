---
segment: 13
title: TUI AI features to 55%
depends_on: [3]
risk: 3
complexity: Medium
cycle_budget: 10
estimated_lines: ~280 test lines
---

# Segment 13: TUI AI Features Coverage to 55%

## Context

**Target modules:**
- `ai_smart.rs` - 20.70% → 50% (403 lines uncovered)
- `ai_features.rs` - 38.02% → 60% (205 lines uncovered)

## Goal

Test AI integration logic, prompt assembly, response handling.

## Implementation Plan

### Priority 1: Prompt Building (~150 lines)

```rust
#[test]
fn test_build_explanation_prompt() {
    let events = create_test_events(10);
    let prompt = build_explanation_prompt(&events, target_idx: 5);
    assert!(prompt.contains("Event #5"));
    assert!(prompt.contains("context"));
}
```

### Priority 2: Response Parsing (~80 lines)

Test streaming response handling, error recovery.

### Priority 3: Smart Suggestions (~50 lines)

Test AI suggestion logic, relevance scoring.

## Success Metrics

- ai_smart: 20.70% → 50%+
- ai_features: 38.02% → 60%+
- ~40 new tests
