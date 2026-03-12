---
segment: 16
title: Error modules to 100%
depends_on: [8, 9]
risk: 1
complexity: Low
cycle_budget: 3
estimated_lines: ~100 test lines
---

# Segment 16: Error Modules Coverage to 100%

## Context

**All error modules currently at 0%:**
- `prb-pcap/src/error.rs`
- `prb-schema/src/error.rs`
- `prb-plugin-api/src/types.rs`
- `prb-fixture/src/format.rs`

**Reason:** thiserror-derived types (no manual code)

## Goal

Add construction and formatting tests for all error types.

## Implementation Plan

```rust
// Quick wins - one test per error type

#[test]
fn test_pcap_error_construction() {
    let err = PcapError::InvalidFormat("bad magic");
    assert!(err.to_string().contains("bad magic"));
}

#[test]
fn test_schema_error_construction() {
    let err = SchemaError::NotFound("service.Method");
    assert!(err.to_string().contains("service.Method"));
}
```

## Success Metrics

- All error modules: 0% → 100%
- ~20 simple tests
