---
segment: 04
title: prb-decode to 90%
depends_on: []
risk: 2
complexity: Medium
cycle_budget: 5
estimated_lines: ~100 test lines
---

# Segment 04: prb-decode Coverage to 90%

## Context

**Current:** 86.31%
**Target:** 90%
**Gap:** +3.69 percentage points

**Gaps:**
- `src/schema_backed.rs` - 69.55% (41 lines uncovered) - schema decoding edge cases
- `src/wire_format.rs` - 86.44% (28 lines uncovered) - protobuf wire format variants

## Goal

Test schema-backed decoding with various protobuf types and wire format edge cases.

## Exit Criteria

1. [ ] prb-decode ≥90%
2. [ ] schema_backed.rs ≥85%
3. [ ] All tests pass

## Implementation Plan

### Priority 1: Schema Decoding Variants (~60 lines)

```rust
// crates/prb-decode/tests/schema_decode_tests.rs

#[test]
fn test_decode_with_missing_field() {
    let schema = create_schema_with_optional_fields();
    let bytes = encode_message_without_field("optional_field");
    let result = decode_with_schema(&bytes, &schema);
    assert!(result.is_ok());
}

#[test]
fn test_decode_repeated_fields() {
    // Test protobuf repeated fields
}

#[test]
fn test_decode_nested_messages() {
    // Test nested message decoding
}

#[test]
fn test_decode_with_unknown_fields() {
    // Test forward compatibility
}
```

### Priority 2: Wire Format Edge Cases (~40 lines)

Test varint edge cases, fixed32/64, zigzag encoding.

## Test Plan

1. `cargo llvm-cov -p prb-decode --html`
2. Add schema edge case tests
3. Verify: `cargo test -p prb-decode`

## Success Metrics

- prb-decode: 86.31% → 90%+
- ~20 new tests
