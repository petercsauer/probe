---
segment: 4
title: "Protobuf Testing Suite"
depends_on: []
risk: 6/10
complexity: High
cycle_budget: 18
status: merged
commit_message: "test(decode): Add comprehensive protobuf type coverage with fuzzing and property tests"
---

# Segment 4: Protobuf Testing Suite

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Increase protobuf coverage from 32-69% to 85%+ with systematic type testing and edge cases.

**Depends on:** None (independent)

## Context: Issues Addressed

**Core Problem:** Protobuf decoding has unbounded recursion risk in schema_backed.rs (lines 192-195 in `format_value()`) - stack overflow possible. Current coverage is 32.55-69.74% with no property testing, no fuzzing, and limited malformed input testing. Uses prost-reflect for dynamic decoding without generated code.

**Proposed Fix:** Add recursion depth limit to formatter (MAX_DEPTH = 64), create descriptor builder utility to eliminate test boilerplate, add parameterized tests with rstest for all 18 protobuf types, create malformed input corpus (50+ cases), add property tests for round-trip validation, establish fuzzing target.

**Pre-Mortem Risks:**
- Recursion limit could break legitimate deeply nested messages - make configurable via env var, default 64 like wire parser
- Proptest round-trips might fail on unknown fields - preserve unknown fields during encoding
- rstest adds compilation time for 90 test instances - acceptable tradeoff, tests still fast at runtime

## Scope

- `crates/prb-decode/src/schema_backed.rs` (356 lines)
- `crates/prb-decode/src/wire_format.rs` (513 lines)
- `crates/prb-decode/tests/helpers/descriptor_builder.rs` - New utility
- `crates/prb-decode/tests/protobuf_type_matrix_test.rs` - New parameterized tests
- `crates/prb-decode/tests/corpus/protobuf_malformed.rs` - New malformed tests
- `crates/prb-decode/tests/protobuf_property_test.rs` - New property tests
- `fuzz/fuzz_targets/protobuf_decoder.rs` - New fuzzing target

## Key Files and Context

**`crates/prb-decode/src/schema_backed.rs`** (356 lines):
- Lines 192-195: Unbounded recursion in `format_value()` for nested messages - stack overflow risk
- Uses `prost-reflect` for dynamic protobuf decoding without generated code
- Current coverage: 32.55% (69.55% line coverage in some areas)

**`crates/prb-decode/src/wire_format.rs`** (513 lines):
- Lines 138-180: Recursive descent parser with MAX_RECURSION_DEPTH = 64
- Current coverage: 69.74% (missing edge cases)

**Existing tests:**
- Use manual descriptor building with `prost_types::FileDescriptorProto`
- Current gaps: No property testing, no fuzzing, limited malformed input testing

## Implementation Approach

1. **Add recursion depth limit** to schema_backed formatter at lines 192-195:
   ```rust
   fn format_value(f: &mut fmt::Formatter, value: &Value, indent: usize, depth: usize) -> fmt::Result {
       const MAX_DEPTH: usize = 64;
       if depth >= MAX_DEPTH {
           return write!(f, "<max recursion depth reached>");
       }
       match value {
           Value::Message(msg) => {
               writeln!(f, "{{")?;
               format_message_fields(f, msg, indent + 1, depth + 1)?; // Pass depth
               write!(f, "{}}}", "  ".repeat(indent))
           }
           // ... other cases
       }
   }
   ```

2. **Create descriptor builder utility** in `tests/helpers/descriptor_builder.rs`:
   ```rust
   pub struct DescriptorBuilder {
       name: String,
       package: String,
       fields: Vec<(String, i32, FieldType)>,
   }

   impl DescriptorBuilder {
       pub fn message(name: &str) -> Self { /* ... */ }
       pub fn field(mut self, name: &str, num: i32, field_type: FieldType) -> Self { /* ... */ }
       pub fn build(self) -> MessageDescriptor { /* ... */ }
   }

   // Usage:
   let desc = DescriptorBuilder::message("TestMsg")
       .field("id", 1, FieldType::Int32)
       .field("name", 2, FieldType::String)
       .build();
   ```

3. **Add parameterized tests** with rstest for all 18 protobuf types in `tests/protobuf_type_matrix_test.rs`:
   - Scalar: int32, int64, uint32, uint64, sint32, sint64, bool, string, bytes (9 types)
   - Fixed: fixed32, fixed64, sfixed32, sfixed64, float, double (6 types)
   - Complex: message, enum, repeated (3 types)
   - Test matrix: Each type × [zero, min, max, normal, negative] values = 90 test cases

4. **Create malformed input corpus** in `tests/corpus/protobuf_malformed.rs` (50+ cases):
   - Invalid varint: no terminator byte (10 bytes all with high bit set), >10 bytes
   - Truncated messages: truncated at tag byte, in middle of varint, in length-delimited field, in fixed32/64
   - Invalid UTF-8 in string fields: `vec![0x12, 0x04, 0xff, 0xfe, 0xfd, 0xfc]`
   - Field number 0 (invalid per spec): `vec![0x00, 0x01]`
   - Recursion bombs: deeply nested messages at depths 100, 1000
   - Reserved wire types 3, 4, 6, 7: `vec![0x1b, 0x01]` (field 3, wire type 3)
   - Zero-length strings/bytes, length overflow (length > remaining bytes)

5. **Add proptest strategies** for round-trip testing in `tests/protobuf_property_test.rs`:
   ```rust
   fn arb_protobuf_message() -> impl Strategy<Value = (MessageDescriptor, Vec<u8>)> {
       // Generate arbitrary valid protobuf messages
       // Return descriptor + encoded bytes
   }

   proptest! {
       #[test]
       fn roundtrip_encode_decode((desc, encoded) in arb_protobuf_message()) {
           let decoded = decode_with_schema(&encoded, &desc).unwrap();
           let re_encoded = encode_message(&decoded);
           assert_eq!(re_encoded, encoded);
       }
   }
   ```

6. **Add fuzzing target** in `fuzz/fuzz_targets/protobuf_decoder.rs`:
   ```rust
   #![no_main]
   use libfuzzer_sys::fuzz_target;
   use prb_decode::decode_wire_format;

   fuzz_target!(|data: &[u8]| {
       let _ = decode_wire_format(data); // Should never panic
   });
   ```

## Alternatives Ruled Out

- **Manual enumeration of 90 type combinations:** Rejected - rstest cleaner, auto-generates test names, easier to review
- **Testing only common types (int32, string, message):** Rejected - production uses all types including fixed32/64, sfixed, etc.

## Pre-Mortem Risks

- Recursion limit on formatter could break legitimate deeply nested messages: Make configurable via env var, default 64 like wire parser
- Proptest round-trips might fail on unknown fields: Preserve unknown fields during encoding
- rstest adds compilation time for 90 test instances: Acceptable tradeoff, tests still fast at runtime

## Build and Test Commands

- Build: `cargo build -p prb-decode`
- Test (targeted): `cargo test -p prb-decode protobuf_type_matrix malformed_corpus protobuf_property`
- Test (regression): `cargo test -p prb-decode`
- Test (full gate): `cargo nextest run -p prb-decode`
- Fuzz (optional): `cargo fuzz run protobuf_decoder -- -max_total_time=60`

## Exit Criteria

1. **Targeted tests:**
   - `protobuf_type_matrix` - 90 tests pass (18 types × 5 value ranges)
   - `malformed_corpus` - 50+ malformed input cases pass (all return errors, no panics)
   - `protobuf_property` - proptest passes (100+ generated round-trips)

2. **Regression tests:** All existing decode tests in `tests/schema_backed_tests.rs`, `tests/wire_format_tests.rs` pass

3. **Full build gate:** `cargo build -p prb-decode` succeeds with zero warnings

4. **Full test gate:** `cargo nextest run -p prb-decode` passes

5. **Self-review gate:**
   - Recursion limit added with configurable depth
   - Descriptor builder eliminates test boilerplate
   - Fuzzing target works (doesn't panic on arbitrary input)

6. **Scope verification gate:** Only modified:
   - `schema_backed.rs` - recursion limit addition
   - `wire_format.rs` - if needed for edge case handling
   - Test files in `tests/` directory
   - Added rstest dependency to Cargo.toml
