---
segment: 09
title: prb-capture to 75%
depends_on: [2]
risk: 4
complexity: High
cycle_budget: 10
estimated_lines: ~220 test lines
---

# Segment 09: prb-capture Coverage to 75%

## Context

**Current:** 35.42%
**Target:** 75%
**Gap:** +39.58 percentage points

**CRITICAL GAPS:**
- `src/capture.rs` - 8.59% (103 lines uncovered)
- `src/adapter.rs` - 39.39% (62 lines uncovered)
- `src/privileges.rs` - 0% (6 lines uncovered)

## Goal

Mock-based tests for live capture without requiring root privileges.

## Implementation Plan

### Priority 1: Mock Adapter Tests (~120 lines)

```rust
// crates/prb-capture/tests/mock_capture_tests.rs

#[test]
fn test_capture_adapter_initialization() {
    let adapter = MockCaptureAdapter::new("lo");
    assert!(adapter.is_ok());
}

#[test]
fn test_capture_with_filter() {
    let mut adapter = MockCaptureAdapter::new("lo").unwrap();
    adapter.set_filter("tcp port 80").unwrap();
    let packets = adapter.capture_packets(10);
    assert!(packets.len() <= 10);
}
```

### Priority 2: Privilege Checking (~50 lines)

Test privilege detection logic without actually requiring privileges.

### Priority 3: Interface Enumeration (~50 lines)

Test network interface listing and validation.

## Success Metrics

- prb-capture: 35.42% → 75%+
- ~35 new tests (many with #[ignore] for privileged operations)
