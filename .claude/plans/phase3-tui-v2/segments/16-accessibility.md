---
segment: 16
title: Accessibility
depends: [14]
risk: 2
complexity: Low
cycle_budget: 5
estimated_lines: 300
---

# Segment 16: Accessibility

## Context

TUI uses colors that may be difficult for colorblind users or high-contrast needs. Need accessibility improvements.

## Goal

Add colorblind-safe themes, high contrast mode, and screen reader hints.

## Exit Criteria

1. [ ] Colorblind-safe themes: deuteranopia, protanopia, tritanopia
2. [ ] High contrast theme option
3. [ ] Semantic markup for screen readers (where applicable)
4. [ ] Keyboard-only navigation verification
5. [ ] No color-only information (use symbols too)
6. [ ] Text size doesn't break layout
7. [ ] Configuration flag for accessibility mode
8. [ ] Manual test: verify with color vision simulator

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/theme.rs` (~150 lines)
  - Colorblind-safe color palettes
  - High contrast theme
- `crates/prb-tui/src/panes/*.rs` (~150 lines)
  - Add symbols alongside colors
  - Semantic markup

### Colorblind-Safe Palettes

Use palettes from ColorBrewer or similar:
- Deuteranopia: blues/oranges
- Protanopia: blues/yellows
- Tritanopia: reds/greens

### Symbol + Color

Don't rely on color alone:
```
✓ Success (green)
✗ Error (red)
⚠ Warning (yellow)
→ Outbound (blue)
← Inbound (cyan)
```

## Test Plan

1. Enable colorblind theme
2. Verify all info is distinguishable
3. Test high contrast
4. Verify keyboard navigation works without mouse
5. Run test suite

## Blocked By

- S14 (Theme System) - needs theme switching

## Blocks

None - accessibility is additive.

## Rollback Plan

Remove accessibility themes, keep default.

## Success Metrics

- All colorblind modes tested
- No information lost without color
- Keyboard navigation complete
- Zero regressions
