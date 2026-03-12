---
segment: 21
title: "Accessibility"
depends_on: [19]
risk: 2
complexity: Low
cycle_budget: 5
status: pending
commit_message: "feat(prb-tui): colorblind-safe palette, high contrast mode, shape+text indicators"
---

# Segment 21: Accessibility

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Ensure the TUI is usable by colorblind users and in low-visibility conditions with colorblind-safe palettes, high-contrast mode, and shape/text indicators alongside color.

**Depends on:** S19 (Theme System — theme infrastructure for palette swapping)

## Scope

- `crates/prb-tui/src/theme.rs` — Colorblind-safe palette, high-contrast theme preset
- `crates/prb-tui/src/panes/event_list.rs` — Shape indicators alongside color

## Implementation

### 21.1 Colorblind-Safe Palette

Replace red/green transport colors (indistinguishable to ~8% of men):

| Transport | Current | Colorblind-Safe |
|-----------|---------|-----------------|
| gRPC | Green | Blue (#0077BB) |
| ZMQ | Yellow | Orange (#EE7733) |
| DDS-RTPS | Magenta | Teal (#009988) |
| TCP | Blue | Yellow (#CCBB44) |
| UDP | Cyan | Purple (#AA3377) |

Add as a `ThemeConfig::colorblind_safe()` preset. Make it the default in the "accessible" theme.

### 21.2 Shape + Text Indicators

Never rely on color alone:
- Errors: `!` prefix (not just red text)
- Direction arrows already use text (good)
- Protocol label is textual (good)
- Add text prefix for warning events: `⚠` or `!`

### 21.3 High Contrast Mode

Ship a `high-contrast` theme preset with maximum luminance contrast:
- Background: pure black (#000000)
- Text: pure white (#FFFFFF)
- Selected: white on blue
- Borders: bright white
- Useful for presentations, poor lighting, low-quality displays

### 21.4 Reduced Motion

For the sparkline/timeline animations in live mode, add an option to disable animation and show static values only.

## Exit Criteria

1. **Colorblind palette:** New theme preset with non-red/green transport colors
2. **Shape indicators:** Errors have `!` prefix regardless of color
3. **High contrast:** Theme preset with maximum luminance contrast
4. **Tests pass:** `cargo nextest run -p prb-tui`
5. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
