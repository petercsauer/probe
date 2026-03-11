---
segment: 06
title: Error Intelligence
depends: []
risk: 2
complexity: Low
cycle_budget: 5
estimated_lines: 500
---

# Segment 06: Error Intelligence

## Context

Decode tree shows raw field values like `grpc.status: 4` with no explanation. Events with warnings are not visually distinguished. TCP RST, TLS alert codes show as raw numbers.

## Goal

Surface human-readable explanations for protocol error codes, TCP states, and TLS alerts directly in the decode tree and event list using static lookup tables.

## Exit Criteria

1. [ ] New module `error_intel.rs` with lookup functions for:
   - gRPC status codes (17 codes)
   - TLS alerts (15+ codes)
   - TCP flags
2. [ ] Unit tests for all lookup functions pass
3. [ ] Decode tree shows gRPC status codes with human name inline
4. [ ] Expandable explanation nodes in decode tree
5. [ ] Events with warnings show `!` prefix in event list
6. [ ] Warning indicator styled with theme warning color
7. [ ] Regression tests pass
8. [ ] Manual test: load gRPC capture with error status

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/error_intel.rs` (~300 lines NEW)
  - `grpc_status_name()` - status code to name
  - `grpc_status_explanation()` - detailed explanation
  - `tcp_flag_explanation()` - TCP flag meanings
  - `tls_alert_description()` - TLS alert descriptions
- `crates/prb-tui/src/panes/decode_tree.rs` (~100 lines)
  - Inline explanations next to known fields
  - Child nodes for detailed explanations
- `crates/prb-tui/src/panes/event_list.rs` (~50 lines)
  - Warning badge `!` prefix
  - Warning styling
- `crates/prb-tui/src/lib.rs` (~1 line)
  - Register module

### Lookup Tables

```rust
pub fn grpc_status_name(code: u32) -> Option<&'static str> {
    match code {
        0 => Some("OK"),
        1 => Some("CANCELLED"),
        2 => Some("UNKNOWN"),
        3 => Some("INVALID_ARGUMENT"),
        4 => Some("DEADLINE_EXCEEDED"),
        5 => Some("NOT_FOUND"),
        6 => Some("ALREADY_EXISTS"),
        7 => Some("PERMISSION_DENIED"),
        8 => Some("RESOURCE_EXHAUSTED"),
        9 => Some("FAILED_PRECONDITION"),
        10 => Some("ABORTED"),
        11 => Some("OUT_OF_RANGE"),
        12 => Some("UNIMPLEMENTED"),
        13 => Some("INTERNAL"),
        14 => Some("UNAVAILABLE"),
        15 => Some("DATA_LOSS"),
        16 => Some("UNAUTHENTICATED"),
        _ => None,
    }
}
```

### Decode Tree Enhancement

```rust
// When rendering metadata field like "grpc.status"
if key == "grpc.status" {
    if let Ok(code) = value.parse::<u32>() {
        if let Some(name) = error_intel::grpc_status_name(code) {
            label = format!("{}: {} ({})", key, value, name);
            // Add explanation as expandable child node
            if let Some(explanation) = error_intel::grpc_status_explanation(code) {
                add_child_node(explanation);
            }
        }
    }
}
```

### Event List Warning Badge

```rust
let warning_indicator = if !event.warnings.is_empty() { "!" } else { " " };
let warning_style = if !event.warnings.is_empty() {
    theme.warning()
} else {
    row_style
};
```

## Test Plan

1. Create unit tests for all lookup functions
2. Load gRPC capture with error status codes
3. Verify decode tree shows human names
4. Verify expandable explanations work
5. Test warning badges in event list
6. Run full test suite
7. Run clippy

## Blocked By

None - foundational feature for Wave 2.

## Blocks

None - error intelligence is standalone.

## Rollback Plan

Remove error_intel module, revert decode tree and event list changes.

## Success Metrics

- All status codes have human-readable names
- Explanations are helpful and accurate
- Warning badges visible in event list
- Zero performance impact
- Zero regressions in existing tests
