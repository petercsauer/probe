---
segment: 15
title: Large File Performance
depends_on: []
risk: 6
complexity: High
cycle_budget: 10
estimated_lines: 550
---

# Segment 15: Large File Performance

## Context

TUI loads entire file into memory. Large files (>100MB) cause slowness and high memory usage. Need streaming and virtual scrolling.

## Goal

Optimize for large file handling with streaming load, virtual scrolling, and incremental filtering.

## Exit Criteria

1. [ ] Streaming load for files >10MB
2. [ ] Virtual scrolling in event list (render only visible rows)
3. [ ] Incremental filter application (don't re-filter everything)
4. [ ] Background indexing for faster search
5. [ ] Progress indicator during load
6. [ ] Handle files up to 1GB without crash
7. [ ] Filter on 100K+ events in <500ms
8. [ ] Manual test: open 500MB pcap file

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/loader.rs` (~250 lines)
  - Streaming load implementation
  - Background indexing
  - Progress tracking
- `crates/prb-tui/src/panes/event_list.rs` (~200 lines)
  - Virtual scrolling
  - Lazy rendering
- `crates/prb-tui/src/event_store.rs` (~100 lines)
  - Incremental filtering
  - Index management

### Streaming Load

```rust
fn load_events_streaming(path: &Path, tx: Sender<LoadEvent>) {
    // Stream packets in batches
    for batch in read_pcap_batches(path, 1000) {
        tx.send(LoadEvent::Batch(batch))?;
    }
}
```

### Virtual Scrolling

Render only visible rows, compute positions dynamically:

```rust
let visible_start = scroll_offset;
let visible_end = scroll_offset + visible_rows;
for i in visible_start..visible_end {
    render_row(i);
}
```

### Incremental Filtering

Track which events have been checked, only filter new ones:

```rust
struct FilterCache {
    last_checked: usize,
    matches: Vec<usize>,
}
```

## Test Plan

1. Generate large pcap (500MB+)
2. Open in TUI
3. Verify streaming load with progress
4. Scroll through events (check smoothness)
5. Apply filter (check speed)
6. Memory usage should be reasonable
7. Run test suite

## Blocked By

None - performance optimization is independent.

## Blocks

None - perf improvements are additive.

## Rollback Plan

Keep full-load for small files, disable streaming for now.

## Success Metrics

- 500MB file loads in <30 seconds
- Memory usage proportional to visible events
- Smooth scrolling
- Fast filtering
- Zero regressions
