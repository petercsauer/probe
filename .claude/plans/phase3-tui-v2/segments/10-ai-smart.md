---
segment: 10
title: AI Smart Features
depends: [03]
risk: 5
complexity: Medium
cycle_budget: 7
estimated_lines: 450
---

# Segment 10: AI Smart Features

## Context

AI panel provides explanations. Extend with smart features: natural language filter generation, anomaly detection, and protocol identification.

## Goal

Add AI-powered smart features for filter generation, anomaly scanning, and protocol hints.

## Exit Criteria

1. [ ] Natural language filter: `/ai <query>` generates filter
2. [ ] Example: "/ai show all failed requests" → `grpc.status != 0`
3. [ ] Anomaly detection scan: identify unusual patterns
4. [ ] Protocol identification hints for unknown payloads
5. [ ] Smart suggestions based on capture content
6. [ ] Error recovery with fallback to manual filter
7. [ ] Rate limiting to prevent API abuse
8. [ ] Manual test: generate filters with NL queries

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/ai_smart.rs` (~250 lines NEW)
  - NL filter generation
  - Anomaly detection
  - Protocol hints
- `crates/prb-tui/src/app.rs` (~100 lines)
  - Wire AI smart commands
  - Handle `/ai` command
- `crates/prb-tui/src/panes/ai_panel.rs` (~100 lines)
  - Display smart suggestions

### NL Filter Generation

```rust
async fn generate_filter(nl_query: &str, context: &CaptureContext) -> Result<Filter> {
    let prompt = format!(
        "Convert this natural language query to a prb filter expression: {}\n\
        Available fields: {}\n\
        Filter syntax: field op value (e.g., grpc.status != 0)",
        nl_query, context.available_fields()
    );
    let response = ai_call(prompt).await?;
    Filter::parse(&response)
}
```

## Test Plan

1. Load test capture
2. Try `/ai show errors`
3. Verify filter generated correctly
4. Test anomaly detection
5. Test protocol hints
6. Run test suite

## Blocked By

- S03 (Wire AI Panel) - needs working AI integration

## Blocks

None - smart features are additive.

## Rollback Plan

Disable `/ai` command, feature-gate behind config.

## Success Metrics

- NL filter generation works 80%+ of time
- Anomaly detection finds real issues
- Protocol hints are helpful
- Good error handling
- Zero regressions
