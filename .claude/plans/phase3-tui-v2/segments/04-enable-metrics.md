---
segment: 04
title: Enable Metrics Overlay
depends: []
risk: 2
complexity: Low
cycle_budget: 3
estimated_lines: 150
---

# Segment 04: Enable Metrics Overlay

## Context

The `metrics_overlay` flag exists in app state but is never toggled. Need to enable this overlay to show performance metrics (p50/p95/p99, throughput, error rates, etc.).

## Current State

```rust
// In app.rs:
metrics_overlay: bool,
```

The flag exists but no keybinding toggles it, and rendering may be incomplete.

## Goal

Enable metrics overlay with keybinding, showing key performance statistics for captured traffic.

## Exit Criteria

1. [ ] Add keybinding (suggest `m` for "metrics") to toggle overlay
2. [ ] Overlay renders correctly over main view
3. [ ] Shows key metrics:
   - Event count total/filtered
   - Packets per second
   - Bytes per second
   - Protocol distribution
   - Error count/rate
   - Latency percentiles (if available)
4. [ ] Can close with same key or Esc
5. [ ] Metrics update in real-time during live capture
6. [ ] Manual test: toggle metrics overlay

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/app.rs` (~150 lines)
  - Add metrics toggle keybinding
  - Render metrics overlay
  - Calculate metrics from event store

### Metrics to Display

```
╭─ Metrics ─────────────────────────────────╮
│ Total Events:     5,234                   │
│ Filtered:         1,892  (36%)            │
│                                            │
│ Throughput:                                │
│   Events/sec:     245.3                   │
│   Bytes/sec:      1.2 MB/s                │
│                                            │
│ Protocols:                                 │
│   TCP:            3,421  (65%)            │
│   UDP:            1,723  (33%)            │
│   ICMP:           90     (2%)             │
│                                            │
│ Errors:           23     (0.4%)           │
│                                            │
│ Latency:                                   │
│   p50:            2.3 ms                  │
│   p95:            12.1 ms                 │
│   p99:            45.7 ms                 │
╰───────────────────────────────────────────╯
```

### Calculation

Most metrics can be derived from `EventStore`:
- Event count: `store.len()`
- Protocol dist: `store.protocol_counts()`
- Throughput: track over rolling time window

Latency requires conversation/request tracking (may be limited initially).

## Test Plan

1. Launch TUI with test pcap
2. Press `m` to show metrics overlay
3. Verify metrics are accurate
4. Press `m` again to close
5. Test with live capture (metrics should update)
6. Run test suite: `cargo nextest run -p prb-tui`

## Blocked By

None - metrics overlay is independent feature.

## Blocks

None - metrics is standalone feature (though S15 may enhance it).

## Rollback Plan

If metrics calculation is expensive or buggy, disable keybinding temporarily.

## Success Metrics

- Metrics display correctly
- Accurate calculations
- Good performance (no lag)
- Clean overlay rendering
- Zero regressions in existing tests

## Notes

- Latency percentiles may be limited without full conversation tracking
- Consider caching metrics calculations
- Update metrics on filter changes
- Could add export metrics to clipboard feature
