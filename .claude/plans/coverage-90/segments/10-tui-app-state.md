---
segment: 10
title: TUI app.rs state to 45%
depends_on: [3, 4]
risk: 5
complexity: High
cycle_budget: 15
estimated_lines: ~400 test lines
---

# Segment 10: TUI app.rs State Management to 45%

## Context

**Current:** 19.94% (app.rs: 4,339 regions, 3,474 uncovered!)
**Target:** 45%
**Gap:** +25.06 percentage points (~1,100 lines)

**Challenge:** app.rs is 4,339 lines - MASSIVE state machine for TUI

## Goal

Test state management, mode switching, event handling - NOT rendering.

## Implementation Plan

### Priority 1: State Machine Tests (~200 lines)

```rust
// crates/prb-tui/tests/app_state_tests.rs

#[test]
fn test_app_mode_transitions() {
    let mut app = App::new();
    assert!(matches!(app.mode, AppMode::Normal));
    
    app.enter_filter_mode();
    assert!(matches!(app.mode, AppMode::Filter));
    
    app.cancel();
    assert!(matches!(app.mode, AppMode::Normal));
}

#[test]
fn test_event_selection() {
    let mut app = create_app_with_events(100);
    app.select_next();
    assert_eq!(app.selected_index(), Some(1));
    
    app.select_previous();
    assert_eq!(app.selected_index(), Some(0));
}

#[test]
fn test_filter_application() {
    let mut app = create_app_with_events(100);
    app.apply_filter("tcp.port == 80");
    assert!(app.filtered_events().len() < 100);
}
```

### Priority 2: Command Handling (~100 lines)

Test key commands, input validation, undo/redo logic.

### Priority 3: Error State Handling (~100 lines)

Test error overlay display, recovery paths.

## Success Metrics

- app.rs: 19.94% → 45%+
- Focus on state management, accept <30% rendering coverage
- ~60 new tests
