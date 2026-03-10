---
segment: 20
title: "Large File Performance"
depends_on: [11]
risk: 6
complexity: High
cycle_budget: 10
status: pending
commit_message: "feat(prb-tui): streaming file loading, background indexing, virtual scroll optimization for 1M+ events"
---

# Segment 20: Large File Performance

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Make the TUI handle files with 100K-1M+ events smoothly: streaming load with progress, background indexing, and optimized virtual scroll.

**Depends on:** S11 (Live Capture — async patterns for background processing)

## Scope

- `crates/prb-tui/src/loader.rs` — Streaming file parser with progress callback
- `crates/prb-tui/src/event_store.rs` — Indexed event store, lazy sorted indices
- `crates/prb-tui/src/app.rs` — Background loading, progress display

## Implementation

### 20.1 Streaming File Loading

Instead of loading all events into memory before showing TUI, stream-parse and show events as they load:

```rust
pub fn load_events_streaming(
    path: &Path,
    sender: mpsc::Sender<LoadEvent>,
) -> Result<()> {
    // For JSON: parse events one-by-one, send each
    // For PCAP: parse packets in chunks of 1000, send batch
    // For MCAP: read messages, send batch
    // Send LoadEvent::Progress(current, total) periodically
    // Send LoadEvent::Done when complete
}

pub enum LoadEvent {
    Batch(Vec<DebugEvent>),
    Progress { loaded: usize, total: Option<usize> },
    Done,
    Error(String),
}
```

Show loading progress in the status bar:
```
Loading capture... [████████░░░░░░░░] 52% (520K / 1M events)
```

User can start browsing loaded events immediately while loading continues.

### 20.2 Background Indexing

After initial load, build indexes in a background task:

```rust
struct EventIndex {
    by_protocol: HashMap<TransportKind, Vec<usize>>,
    by_source: HashMap<String, Vec<usize>>,
    by_dest: HashMap<String, Vec<usize>>,
    time_sorted: Vec<usize>,
}
```

Filter operations use indexes when available (O(1) lookup for protocol filter vs O(n) scan).

### 20.3 Virtual Scroll Optimization

Current issues at scale:
- `filtered_indices` is cloned on every sort — cache it, invalidate on filter/sort change
- Sort operates on full `filtered_indices` — use partial sort for visible window
- `protocol_counts()` scans all filtered events — cache and incrementally update

```rust
struct CachedView {
    filter_hash: u64,
    sort_key: SortColumn,
    sort_reversed: bool,
    sorted_indices: Vec<usize>,
    protocol_counts: Vec<(TransportKind, usize)>,
}
```

Only recompute when filter or sort changes, not on every frame.

### 20.4 Memory-Mapped Large Files

For PCAP files >100MB, use memory-mapped I/O to avoid loading entire file into RAM. Parse packets on-demand from the mmap when accessed.

### 20.5 Benchmarks

Add a benchmark test that loads a 100K event dataset and measures:
- Time to first render
- Frame render time at various scroll positions
- Filter application time
- Sort time

## Key Files and Context

- `crates/prb-tui/src/loader.rs` — Current synchronous `load_events()`
- `crates/prb-tui/src/event_store.rs` — `EventStore`, `filter_indices()`, `protocol_counts()`
- `crates/prb-tui/src/panes/event_list.rs` — Virtual scroll rendering

## Pre-Mortem Risks

- Streaming JSON parsing requires careful handling of partial reads
- Background indexing thread must not block the render loop
- Memory-mapped PCAP may not work on all platforms

## Exit Criteria

1. **Streaming load:** TUI shows events as they load, progress bar in status
2. **Background indexing:** Indexes built after load, filter uses indexes when available
3. **Cached view:** Sort/filter results cached, not recomputed per frame
4. **Performance:** 100K events: <100ms filter, <16ms render, <500ms sort
5. **Tests pass:** `cargo nextest run -p prb-tui`
6. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
