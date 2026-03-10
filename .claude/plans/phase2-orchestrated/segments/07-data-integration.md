---
segment: 7
title: "Data Layer & CLI Integration"
depends_on: []
risk: 4
complexity: Medium
cycle_budget: 3
status: pending
commit_message: "feat(prb-tui): add EventStore, file loaders, and prb tui CLI subcommand"
---

# Subsection 7: Data Layer & CLI Integration

## Purpose

Bridge between the existing Probe crates and the TUI. Loads events from all
supported formats into an in-memory `EventStore`, and exposes the TUI as a
CLI subcommand.

## EventStore

```rust
pub struct EventStore {
    events: Vec<DebugEvent>,
    by_timestamp: Vec<usize>,   // indices sorted by timestamp
    time_range: Option<(Timestamp, Timestamp)>,
}

impl EventStore {
    pub fn len(&self) -> usize;
    pub fn get(&self, index: usize) -> Option<&DebugEvent>;
    pub fn iter(&self) -> impl Iterator<Item = &DebugEvent>;
    pub fn time_range(&self) -> Option<(Timestamp, Timestamp)>;
    pub fn filter(&self, f: &Filter) -> Vec<usize>;
}
```

---

## Segment S7.1: EventStore

In-memory store with timestamp-sorted index. `filter()` returns indices of
matching events. For large captures (>100k events), filtering is O(n) but runs
on tick (4 Hz), not on every frame.

## Segment S7.2: File Loaders

**JSON fixtures**: Use `prb-fixture` → `FixtureCaptureAdapter::ingest()` → collect
into `EventStore`.

**MCAP files**: Use `prb-storage` → `SessionReader::read_events()` → collect.

**PCAP files**: Use `prb-pcap` pipeline → decode with `prb-grpc`, `prb-zmq`,
`prb-dds` → collect. Reuse existing `ingest` command's pipeline logic.

**Format detection**: Reuse magic-bytes detection from prb-cli or implement
equivalent in prb-tui.

## Segment S7.3: CLI `prb tui` Subcommand

Add to `prb-cli`:
```
prb tui <file>              Open file in TUI
prb tui <file> --where "x"  Open with pre-applied filter
```

Also add `--where` flag to `prb inspect` for CLI-only filtering.
