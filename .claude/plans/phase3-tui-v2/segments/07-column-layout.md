---
segment: 07
title: Column Layout Improvements
depends: []
risk: 3
complexity: Medium
cycle_budget: 5
estimated_lines: 400
---

# Segment 07: Column Layout Improvements

## Context

Event list columns have fixed widths and don't adapt to content. We recently improved this somewhat, but need smarter fallback display and better adaptive sizing.

## Goal

Make event list show useful source/destination info in all cases, with adaptive column sizing based on terminal width and content.

## Exit Criteria

1. [ ] Smart fallback: when network info absent, show origin instead of "-"
2. [ ] Adaptive column widths based on terminal size
3. [ ] Detect when network info is absent and collapse columns
4. [ ] Header adapts to show "Origin" vs "Source/Destination"
5. [ ] Summary column uses remaining space effectively
6. [ ] Columns scale smoothly from narrow to wide terminals
7. [ ] Update test fixtures to have network addresses
8. [ ] Manual test: verify layout with various terminal widths

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/panes/event_list.rs` (~300 lines)
  - Smart fallback display functions
  - Adaptive column width calculation
  - Header adaptation
- `fixtures/*.json` (~100 lines)
  - Add network addresses to test fixtures

### Smart Fallback

```rust
fn format_source(event: &DebugEvent) -> String {
    event.source.network.as_ref()
        .map(|n| n.src.to_string())
        .unwrap_or_else(|| event.source.origin.clone())
}
```

### Adaptive Widths

Already partially implemented, but enhance with:
- Better detection of network info presence
- Smoother scaling across terminal sizes
- Header column name adaptation

## Test Plan

1. Test with fixtures (no network info)
2. Test with real pcap (has network info)
3. Resize terminal and verify columns adapt
4. Run test suite
5. Run clippy

## Blocked By

None - layout improvements are independent.

## Blocks

- S09 (Trace Correlation) - benefits from better layout

## Rollback Plan

Revert column width calculations to fixed values.

## Success Metrics

- Columns adapt smoothly to terminal size
- No wasted space
- Smart fallback shows useful info
- Zero performance impact
- Zero regressions
