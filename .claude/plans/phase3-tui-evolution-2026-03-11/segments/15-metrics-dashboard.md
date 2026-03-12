---
segment: 15
title: "Metrics Dashboard"
depends_on: [9]
risk: 4
complexity: Medium
cycle_budget: 5
status: pending
commit_message: "feat(prb-tui): metrics dashboard pane — latency percentiles, error rate, throughput, per-protocol breakdown"
---

# Segment 15: Metrics Dashboard

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Add a metrics dashboard overlay showing aggregate conversation metrics: latency percentiles, error rates, throughput, and per-protocol breakdown.

**Depends on:** S09 (Conversation View — ConversationSet is required for metrics computation)

## Current State

- `prb-core/src/metrics.rs` has `compute_aggregate_metrics(conversations) -> AggregateMetrics`
- `AggregateMetrics` contains: total_conversations, error_rate, latency_p50/p95/p99_ns, total_bytes, conversations_per_second
- Conversation metrics (per-conversation) also available
- None shown in TUI

## Scope

- `crates/prb-tui/src/overlays/metrics_dashboard.rs` — **New file.** Metrics overlay
- `crates/prb-tui/src/app.rs` — `m` key toggle

## Implementation

### 15.1 Metrics Dashboard Overlay

Press `m` to toggle metrics dashboard:

```
Metrics ────────────────────────────────────────
 Conversations: 47    Error rate: 4.3%
 Latency:  p50=23ms  p95=89ms  p99=142ms
 Throughput: 12.3 conv/s  45.6 KB/s
 ──────────────────────────────────────────────
 By Protocol:
   gRPC:  32 conv  p50=19ms  2 errors
   ZMQ:   12 conv  p50=31ms  0 errors
   DDS:    3 conv  p50=45ms  0 errors
```

Use `compute_aggregate_metrics()` with conversations from `ConversationSet`. Format nanosecond latencies as human-readable (ms/us). Use bar charts (ratatui `BarChart` widget) for visual latency distribution.

### 15.2 Per-Protocol Breakdown

Group conversations by protocol, compute metrics per group:

```rust
let by_protocol = conversation_set.by_protocol(TransportKind::Grpc);
let grpc_metrics = compute_aggregate_metrics(&by_protocol.iter().collect::<Vec<_>>());
```

### 15.3 Live Metrics

During live capture (S11), update metrics in real-time. Show trend indicators (arrow up/down for increasing/decreasing latency).

### 15.4 Latency Histogram

If space permits, show a mini latency histogram using ratatui's block characters:

```
Latency Distribution:
 0-10ms  ████████████████  32
10-50ms  ████████  16
50-100ms ███  6
 >100ms  █  2
```

## Key Files and Context

- `crates/prb-core/src/metrics.rs` — `compute_aggregate_metrics()`, `AggregateMetrics`
- `crates/prb-core/src/engine.rs` — `ConversationSet::by_protocol()`

## Exit Criteria

1. **Dashboard:** `m` toggles metrics overlay
2. **Metrics shown:** Latency p50/p95/p99, error rate, throughput, conversation count
3. **Per-protocol:** Breakdown by gRPC, ZMQ, DDS, etc.
4. **Formatting:** Nanoseconds formatted as human-readable ms/us
5. **Tests pass:** `cargo nextest run -p prb-tui`
6. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
