---
segment: 24
title: "Session Comparison"
depends_on: [9, 15]
risk: 6
complexity: High
cycle_budget: 10
status: pending
commit_message: "feat(prb-tui): session comparison — diff two captures, regression detection"
---

# Segment 24: Session Comparison

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Enable comparing two capture files side-by-side to identify differences, new errors, and performance regressions.

**Depends on:** S09 (Conversations — for conversation-level comparison), S15 (Metrics — for regression detection)

## Scope

- `crates/prb-tui/src/overlays/diff_view.rs` — **New file.** Side-by-side diff overlay
- `crates/prb-tui/src/app.rs` — `--diff` flag, diff mode
- `crates/prb-cli/src/commands/tui.rs` — `--diff` CLI flag

## Implementation

### 24.1 Diff Two Captures

```bash
prb tui --diff file1.json file2.json
```

Or from within TUI: open second file for comparison via command palette.

Show side-by-side view:
- Events unique to file A (green left column)
- Events unique to file B (red right column)
- Events present in both (white, centered)

Match by: timestamp proximity, request method, source/dest pair.

### 24.2 Conversation-Level Diff

Compare conversations between the two files:
- New conversations in B
- Missing conversations from A
- Changed conversations (different status, duration, etc.)

### 24.3 Regression Detection

Compare aggregate metrics between the two captures:

```
Regression Report ──────────────────────────
 Latency p95: 45ms → 120ms  (+167%) ▲
 Error rate:  2.1% → 8.5%  (+6.4%) ▲
 New errors: 3 conversations
 New endpoint: /api.v2.Items/Get
 Missing endpoint: /api.v1.Items/Get (renamed?)
```

Highlight regressions in red, improvements in green.

### 24.4 Navigation

Navigate between diff entries. Select an entry to see the full event in the corresponding event store.

## Key Files and Context

- `crates/prb-core/src/metrics.rs` — `compute_aggregate_metrics()`
- `crates/prb-core/src/engine.rs` — `ConversationSet` for both files

## Exit Criteria

1. **Diff view:** `--diff` opens side-by-side comparison
2. **Event matching:** Events matched by method/timestamp proximity
3. **Conversation diff:** Shows new/missing/changed conversations
4. **Regression report:** Compares latency, error rate, identifies regressions
5. **Tests pass:** `cargo nextest run -p prb-tui`
6. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
