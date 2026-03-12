---
segment: 12
title: Complete Request Waterfall
depends_on: [01]
risk: 5
complexity: Medium
cycle_budget: 7
estimated_lines: 550
---

# Segment 12: Complete Request Waterfall

## Context

Waterfall pane exists but may be incomplete. Need timing bars, breakdown by phase, and color coding.

## Goal

Complete waterfall view showing request timing with phase breakdown (DNS, TLS handshake, request, response).

## Exit Criteria

1. [ ] Waterfall shows horizontal timing bars per request
2. [ ] Color-coded phases: DNS, TCP, TLS, wait, response
3. [ ] Hovering shows phase durations
4. [ ] Click request to select in event list
5. [ ] Sort by duration, start time, status
6. [ ] Filter waterfall same as event list
7. [ ] Export waterfall as HAR format
8. [ ] Manual test: view waterfall for HTTP/gRPC captures

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/panes/waterfall.rs` (~400 lines)
  - Complete timing bar rendering
  - Phase breakdown calculation
  - Click handling
  - Sorting
- `crates/prb-tui/src/app.rs` (~150 lines)
  - Wire waterfall interactions
  - Export functionality

### Phase Breakdown

```rust
struct RequestTiming {
    dns: Duration,
    tcp: Duration,
    tls: Duration,
    wait: Duration,
    response: Duration,
}

fn calculate_phases(conversation: &Conversation) -> RequestTiming {
    // Extract from conversation timestamps
}
```

### Rendering

```
Request 1: GET /api/users    [███▓▓▓░░░░░░░░░░░░] 234ms
Request 2: POST /api/data    [███████▓▓▓░░░░░░░] 189ms
                              DNS TCP TLS Wait Resp
```

## Test Plan

1. Load HTTP capture
2. Toggle to waterfall view
3. Verify phases display correctly
4. Click request and verify selection
5. Test sorting
6. Export as HAR
7. Run test suite

## Blocked By

- S01 (Enable Conversation) - waterfall needs conversation data

## Blocks

None - waterfall is additive feature.

## Rollback Plan

Disable waterfall view, keep event list.

## Success Metrics

- Timing bars accurate
- Phase breakdown helpful
- Click interaction works
- HAR export valid
- Zero regressions
