---
segment: 8
title: "Event List Pane"
depends_on: [1, 2, 6, 7]
risk: 4
complexity: Medium
cycle_budget: 3
status: pending
commit_message: "feat(prb-tui): add virtual-scroll event list pane with filter bar"
---

# Subsection 3: Event List Pane

## Purpose

The primary navigation pane. Displays DebugEvents in a scrollable table with
virtual rendering for O(1) per-frame cost regardless of event count.

## Virtual Scrolling Strategy

ratatui's built-in `Table` widget is O(n) — it iterates all rows even when only
~30 are visible (GitHub issue #1004). For 100k+ events this causes multi-second
frame times.

**Solution**: Manual windowed rendering. The pane maintains:
- `scroll_offset: usize` — index of the first visible row
- `selected: usize` — index of the selected row (within filtered set)
- `visible_height: u16` — calculated from the render area

On render, only slice `filtered_indices[scroll_offset..scroll_offset+visible_height]`
and build `Row` widgets for that window. The `Scrollbar` widget reflects the full
dataset size via `ScrollbarState`.

## Columns

| Column | Width | Source |
|--------|-------|--------|
| # | 6 | `event.id` |
| Time | 12 | `event.timestamp` (HH:MM:SS.mmm) |
| Source | 18 | `event.source.network.src` or adapter |
| Destination | 18 | `event.source.network.dst` or "—" |
| Protocol | 10 | `event.transport` display |
| Dir | 3 | ← / → / ? |
| Summary | Fill | First metadata value or payload preview |

---

## Segment S3.1: Virtual-Scroll Table

Implement `EventListPane` with:
- `render()` that slices the visible window and renders rows
- `handle_key()` for j/k/↑/↓/g/G/Home/End navigation
- Automatic scroll adjustment to keep selected row visible
- Scrollbar on right edge

## Segment S3.2: Filter Bar Integration

The filter bar is an overlay at the top. When `/` is pressed:
- Focus moves to filter bar (tui-input widget)
- User types a prb-query expression
- On Enter: parse with `prb_query::parse_filter()`, recompute `filtered_indices`
- On Esc: cancel, restore previous filter
- Show parse errors inline in red

`filtered_indices` is a `Vec<usize>` of indices into the full event store that
match the filter. Updated on filter change, not on every frame.

## Segment S3.3: Column Sort + Summary

- `s` key cycles sort column
- `S` reverses sort direction
- Sort operates on `filtered_indices` via `sort_by` with the chosen comparator
- Summary column: truncated first metadata value, or hex preview of payload
