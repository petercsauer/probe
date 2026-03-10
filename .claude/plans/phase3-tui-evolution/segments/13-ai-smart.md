---
segment: 13
title: "AI Smart Features"
depends_on: [12, 6]
risk: 5
complexity: Medium
cycle_budget: 7
status: pending
commit_message: "feat(prb-tui): natural language filter, AI capture summary, anomaly scan"
---

# Segment 13: AI Smart Features

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Build on the AI explain panel to add natural language filtering, capture summary/analysis, and anomaly detection.

**Depends on:** S12 (AI Explain — provider integration), S06 (Filter — filter bar for NL filter)

## Scope

- `crates/prb-tui/src/app.rs` — NL filter mode, capture summary trigger
- `crates/prb-tui/src/ai_features.rs` — **New file.** NL-to-filter conversion, summary generation

## Implementation

### 13.1 Natural Language Filter

Press `@` to enter AI filter mode. Type natural language:

```
@: show me failed gRPC calls with latency > 100ms
→ transport == "gRPC" && grpc.status != "0"  [Apply? Enter/Esc]
```

The LLM converts to a `prb-query` expression. Show generated expression for approval before applying. Include `prb-query` field names and operators in the prompt context.

### 13.2 Capture Summary ("What's Wrong?")

Press `A` (shift-a) to ask the AI to analyze the filtered capture:
- Summarize traffic patterns
- Identify anomalies
- Suggest investigation filters
- Surface error patterns

Send a batch of events (sampled if >100) to the AI with a structured prompt.

### 13.3 Anomaly Indicators

After AI analysis, mark events with anomaly tags in the event list (subtle icon or color). Cache anomaly results.

## Key Files and Context

- `crates/prb-ai/src/explain.rs` — Streaming explain functions
- `crates/prb-query/src/lib.rs` — `parse_filter()` for validating generated expressions
- `crates/prb-tui/src/panes/ai_panel.rs` — AI panel from S12

## Exit Criteria

1. **NL filter:** `@` opens AI filter mode, generated expression shown for approval
2. **Capture summary:** `A` produces traffic summary in AI panel
3. **Validation:** Generated filter expression is validated by `parse_filter()` before apply
4. **Graceful errors:** Invalid AI output shows "Could not generate filter" message
5. **Tests pass:** `cargo nextest run -p prb-tui`
6. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
