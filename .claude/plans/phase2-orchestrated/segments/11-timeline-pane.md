---
segment: 11
title: "Timeline Pane"
depends_on: [6, 7]
risk: 2
complexity: Low
cycle_budget: 2
status: pending
commit_message: "feat(prb-tui): add timeline sparkline pane with protocol distribution"
---

# Subsection 6: Timeline Pane

## Purpose

A sparkline minimap showing event density over time. Provides temporal context
for the capture and helps users identify bursts, gaps, and patterns.

## Layout

```
▁▂▃▅▇█▇▅▃▂▁▁▂▃▅▇█▇▅▃▂▁▂▃▅▇  14:00:01.000 ─── 14:05:32.456
                                gRPC: ██ ZMQ: ██ DDS: ██
```

3 lines total: sparkline (1 line), time range (1 line), legend (1 line).

---

## Segment S6.1: Sparkline Widget

- Divide the time range into N buckets (N = terminal width of the pane)
- Count events per bucket from `filtered_indices`
- Render using ratatui's built-in `Sparkline` widget
- Current selected event's bucket gets a marker/highlight

## Segment S6.2: Protocol Distribution + Time Range

- Below the sparkline: formatted time range (start ─── end)
- Protocol legend with color-coded counts
- If a filter is active, show "(filtered)" indicator
