---
segment: 09
title: Trace Correlation View
depends_on: [01, 05, 07]
risk: 5
complexity: Medium
cycle_budget: 7
estimated_lines: 500
---

# Segment 09: Trace Correlation View

## Context

Users need to correlate network traffic with distributed traces (OpenTelemetry). This requires trace ID extraction, tree view, and timing correlation.

## Goal

Add trace correlation view showing OTel traces as trees, correlated with packet timestamps.

## Exit Criteria

1. [ ] Extract trace IDs from gRPC metadata and HTTP headers
2. [ ] Build trace tree from trace ID → span ID relationships
3. [ ] New pane: TraceCorrelationPane
4. [ ] Keybinding `t` toggles trace view
5. [ ] Tree shows trace → spans → events hierarchy
6. [ ] Click span to jump to corresponding packet
7. [ ] Timing correlation with packet timestamps
8. [ ] Support OTel trace format (JSON, Protobuf)
9. [ ] Manual test: load trace-instrumented capture

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/panes/trace_correlation.rs` (~300 lines NEW)
  - Trace tree rendering
  - Span selection
  - Jump to packet
- `crates/prb-tui/src/app.rs` (~100 lines)
  - Wire trace pane
  - Toggle keybinding
- `crates/prb-tui/src/trace_extraction.rs` (~100 lines NEW)
  - Extract trace IDs from metadata
  - Build trace trees

### Trace Extraction

```rust
fn extract_trace_id(event: &DebugEvent) -> Option<String> {
    // Check gRPC metadata
    event.metadata.get("traceparent")?
    // Or HTTP header: X-Cloud-Trace-Context
}
```

## Test Plan

1. Load capture with OTel instrumentation
2. Press `t` to show trace view
3. Verify trace tree displays correctly
4. Click span and verify jump to packet
5. Run test suite

## Blocked By

- S01 (Enable Conversation) - trace correlation benefits from conversation context
- S05 (Schema Decode) - Protobuf traces need schema
- S07 (Column Layout) - layout improvements needed

## Blocks

None - trace correlation is additive.

## Rollback Plan

Disable keybinding, feature-gate behind `--traces`.

## Success Metrics

- Trace IDs extracted correctly
- Tree view is clear and navigable
- Timing correlation accurate
- Zero regressions
