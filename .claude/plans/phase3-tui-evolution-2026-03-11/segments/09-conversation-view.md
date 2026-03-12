---
segment: 9
title: "Conversation View & Follow Stream"
depends_on: [5, 6]
risk: 6
complexity: High
cycle_budget: 10
status: pending
commit_message: "feat(prb-tui): conversation list view, follow stream overlay, conversation metrics"
---

# Segment 9: Conversation View & Follow Stream

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Wire the existing `ConversationEngine` from `prb-core` into the TUI, adding a conversation list view, follow-stream overlay, and per-conversation metrics.

**Depends on:** S05 (Zoom/Mouse — overlay patterns), S06 (Filter — filter by conversation)

## Current State

- `prb-core` has `ConversationEngine` with `build_conversations()` → `ConversationSet`
- `ConversationSet` has `sorted_by_time()`, `by_protocol()`, `stats()`, `for_event()`
- Correlation strategies exist for gRPC (`GrpcCorrelationStrategy`), ZMQ, and DDS
- `compute_aggregate_metrics()` returns p50/p95/p99 latency, error rate, throughput
- None of this is wired into the TUI — users see individual events with no conversation grouping

## Scope

- `crates/prb-tui/src/panes/conversation_list.rs` — **New file.** Conversation list pane
- `crates/prb-tui/src/overlays/follow_stream.rs` — **New file.** Follow stream overlay
- `crates/prb-tui/src/app.rs` — Conversation engine integration, `C` key toggle, `F` key overlay

## Implementation

### 9.1 Build Conversations on Load

After loading events, build conversations:

```rust
fn build_conversations(store: &EventStore) -> Option<ConversationSet> {
    let mut engine = ConversationEngine::new();
    engine.register(Box::new(GrpcCorrelationStrategy::new()));
    engine.register(Box::new(ZmqCorrelationStrategy::new()));
    engine.register(Box::new(DdsCorrelationStrategy::new()));
    engine.build_conversations(store.events()).ok()
}
```

Store `Option<ConversationSet>` in `AppState`.

### 9.2 Conversation List Pane

Press `C` to toggle between event list and conversation list as the primary pane:

```rust
pub struct ConversationListPane {
    selected: usize,
    scroll_offset: usize,
    sort_column: ConvSortColumn,
}

pub enum ConvSortColumn { Id, Protocol, Source, Dest, Requests, Duration, Error }
```

Columns:
| Column | Width | Source |
|--------|-------|--------|
| # | 6 | conversation index |
| Protocol | 10 | conversation.protocol |
| Source | 18 | first event source |
| Dest | 18 | first event dest |
| Reqs | 5 | request count |
| Duration | 10 | formatted duration |
| Status | 8 | OK/ERROR |
| Method | fill | gRPC method or topic |

Selecting a conversation filters the event list to show only that conversation's events.

### 9.3 Follow Stream Overlay

Press `F` on a selected event to open a full-screen follow-stream overlay:

```
┌─ Follow Stream: /api.v1.Users/Get ──────────────────────┐
│                                                          │
│  → 10.0.0.1:50051 → 10.0.0.2:8080                      │
│    gRPC Request: GetUserRequest                          │
│    { name: "Alice" }                                     │
│                                                          │
│  ← 10.0.0.2:8080 → 10.0.0.1:50051  (+45ms)             │
│    gRPC Response: GetUserResponse (OK)                   │
│    { user: { id: 1, name: "Alice", role: "admin" } }    │
│                                                          │
│  Duration: 45ms  │  2 events  │  Status: OK              │
└──────────────────────────────────────────────────────────┘
```

Color-code: client→server in green, server→client in blue. Show decoded payloads if schemas loaded (from S04). Scrollable for long conversations.

### 9.4 Conversation Metrics Overlay

Press `M` to show aggregate metrics:

```
Conversations: 47    Error rate: 4.3%
Latency:  p50=23ms  p95=89ms  p99=142ms
Throughput: 12.3 conv/s  45.6 KB/s
─────────────────────────────────────
By Protocol:
  gRPC:  32 conv  p50=19ms  2 errors
  ZMQ:   12 conv  p50=31ms  0 errors
  DDS:    3 conv  p50=45ms  0 errors
```

Use `compute_aggregate_metrics()` and per-protocol breakdown.

### 9.5 Status Bar Integration

When conversations are available, show conversation count in status bar:
```
4 events │ 2 conversations │ gRPC: 1 ZMQ: 1 │ ...
```

## Key Files and Context

- `crates/prb-core/src/engine.rs` — `ConversationEngine`, `ConversationSet`
- `crates/prb-core/src/conversation.rs` — `Conversation`, `ConversationMetrics`
- `crates/prb-core/src/metrics.rs` — `compute_aggregate_metrics()`, `AggregateMetrics`
- `crates/prb-grpc/src/correlation.rs` — `GrpcCorrelationStrategy`
- `crates/prb-zmq/src/correlation.rs` — `ZmqCorrelationStrategy`
- `crates/prb-dds/src/correlation.rs` — `DdsCorrelationStrategy`
- `crates/prb-tui/src/app.rs` — App state, overlay rendering

## Pre-Mortem Risks

- Conversation engine may need all events upfront — verify it works with partial data
- Correlation strategies might not handle fixture data well — use demo events if needed
- Follow stream overlay needs to handle conversations with many events — add scrolling

## Build and Test Commands

- Build: `cargo check -p prb-tui`
- Test (targeted): `cargo nextest run -p prb-tui -p prb-core`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Conversations built:** After loading events, ConversationSet is computed and stored
2. **Conversation list:** `C` toggles conversation list pane with sortable columns
3. **Selection filters:** Selecting a conversation filters event list to its events
4. **Follow stream:** `F` opens scrollable overlay showing req/resp flow with colors
5. **Metrics overlay:** `M` shows aggregate metrics with per-protocol breakdown
6. **Status bar:** Shows conversation count when available
7. **Tests:** Conversation list rendering and metrics computation tests pass
8. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
