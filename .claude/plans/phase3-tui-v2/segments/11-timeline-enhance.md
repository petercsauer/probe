---
segment: 11
title: Timeline Enhancements
depends_on: [01]
risk: 4
complexity: Medium
cycle_budget: 5
estimated_lines: 400
---

# Segment 11: Timeline Enhancements

## Context

Timeline pane shows basic sparklines. Need interactive features: click to jump, zoom, latency heatmap.

## Goal

Enhance timeline with interactivity, zoom, and latency visualization.

## Exit Criteria

1. [ ] Click timeline to jump to that time range in event list
2. [ ] Zoom timeline with +/- keys
3. [ ] Latency heatmap mode (when conversations available)
4. [ ] Toggle between sparkline and heatmap with `h` key
5. [ ] Show time range selection
6. [ ] Cursor shows timestamp on hover
7. [ ] Time bucket size adapts to zoom level
8. [ ] Manual test: click, zoom, heatmap mode

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/panes/timeline.rs` (~300 lines)
  - Interactive click handling
  - Zoom level management
  - Latency heatmap rendering
- `crates/prb-tui/src/app.rs` (~100 lines)
  - Wire timeline click events
  - Handle time range selection

### Interactive Click

```rust
fn handle_click(&mut self, x: u16, area: Rect) -> Action {
    let bucket = (x - area.x) as usize;
    let (start_time, end_time) = self.bucket_time_range(bucket);
    Action::JumpToTimeRange(start_time, end_time)
}
```

### Latency Heatmap

Use conversation data to compute request/response latencies, visualize as color-coded heatmap.

## Test Plan

1. Click timeline
2. Verify jump to correct time
3. Zoom in/out with +/-
4. Toggle heatmap mode
5. Run test suite

## Blocked By

- S01 (Enable Conversation) - heatmap needs conversation data

## Blocks

None - timeline enhancements are additive.

## Rollback Plan

Disable interactive features, keep basic sparkline.

## Success Metrics

- Click accurately jumps to time
- Zoom works smoothly
- Heatmap is visually clear
- Zero regressions
