//! Malformed protobuf input corpus for robustness testing.
//!
//! Each test case represents a specific malformation that decoders must handle gracefully
//! without panicking. Tests verify that errors are returned, not panics.

mod helpers;

use helpers::descriptor_builder::{DescriptorBuilder, FieldType};
use prb_decode::schema_backed::decode_with_schema;
use prb_decode::wire_format::decode_wire_format;
use rstest::rstest;

// ============================================================================
// Invalid Varint Tests
// ============================================================================

#[test]
fn test_varint_no_terminator_10_bytes() {
    // 10 bytes all with high bit set - exceeds max varint length
    let bytes = vec![0x80; 11]; // tag + 10 continuation bytes
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on varint with no terminator");
}

#[test]
fn test_varint_incomplete_in_tag() {
    // Tag starts but message ends
    let bytes = vec![0x80]; // Continuation byte with nothing following
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on incomplete tag varint");
}

#[test]
fn test_varint_incomplete_in_value() {
    // Tag is complete but value varint is incomplete
    let bytes = vec![0x08, 0x80]; // field 1, incomplete varint value
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on incomplete value varint");
}

#[test]
fn test_varint_excessive_length() {
    // More than 10 bytes for a varint
    let bytes = vec![
        0x08, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01,
    ];
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on >10 byte varint");
}

// ============================================================================
// Truncated Message Tests
// ============================================================================

#[test]
fn test_truncated_at_tag() {
    // Message ends immediately after partial tag
    let bytes = vec![0x80, 0x80]; // Multi-byte tag incomplete
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on truncated tag");
}

#[test]
fn test_truncated_fixed32_0_bytes() {
    // Fixed32 field with no data
    let bytes = vec![0x25]; // tag (4 << 3) | 5, but no data
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on truncated fixed32");
}

#[test]
fn test_truncated_fixed32_2_bytes() {
    // Fixed32 field with only 2 bytes
    let bytes = vec![0x25, 0x01, 0x02];
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on truncated fixed32");
}

#[test]
fn test_truncated_fixed64_4_bytes() {
    // Fixed64 field with only 4 bytes
    let bytes = vec![0x29, 0x01, 0x02, 0x03, 0x04];
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on truncated fixed64");
}

#[test]
fn test_truncated_length_delimited_no_data() {
    // Length says 10 but no data follows
    let bytes = vec![0x0a, 0x0a]; // field 1, length 10, but no data
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on truncated length-delimited");
}

#[test]
fn test_truncated_length_delimited_partial_data() {
    // Length says 10 but only 5 bytes follow
    let bytes = vec![0x0a, 0x0a, 0x01, 0x02, 0x03, 0x04, 0x05];
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on truncated length-delimited");
}

#[test]
fn test_truncated_in_middle_of_length_varint() {
    // Length varint is incomplete
    let bytes = vec![0x0a, 0x80]; // field 1, incomplete length varint
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on incomplete length varint");
}

// ============================================================================
// Invalid UTF-8 in String Fields
// ============================================================================

#[test]
fn test_invalid_utf8_string() {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("text", 1, FieldType::String)
        .build();

    // Invalid UTF-8 sequence
    let bytes = vec![0x0a, 0x04, 0xff, 0xfe, 0xfd, 0xfc];
    let result = decode_with_schema(&bytes, &desc);
    // prost-reflect may handle this differently - check it doesn't panic
    // String fields with invalid UTF-8 should either error or be handled gracefully
    let _ = result; // May succeed or fail depending on implementation
}

#[test]
fn test_invalid_utf8_continuation_byte() {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("text", 1, FieldType::String)
        .build();

    // UTF-8 continuation byte without starter
    let bytes = vec![0x0a, 0x02, 0x80, 0x80];
    let _ = decode_with_schema(&bytes, &desc);
}

#[test]
fn test_invalid_utf8_overlong_encoding() {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("text", 1, FieldType::String)
        .build();

    // Overlong encoding of '/' (should be 0x2F, not 0xC0 0xAF)
    let bytes = vec![0x0a, 0x02, 0xc0, 0xaf];
    let _ = decode_with_schema(&bytes, &desc);
}

// ============================================================================
// Field Number Zero (Invalid)
// ============================================================================

#[test]
fn test_field_number_zero() {
    // Field number 0 is invalid per protobuf spec
    let bytes = vec![0x00, 0x01]; // field 0, value 1
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on field number 0");
}

#[test]
fn test_field_number_zero_in_length_delimited() {
    // Field number 0 in length-delimited
    let bytes = vec![0x02, 0x01]; // field 0, wire type 2, length 1
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on field number 0");
}

// ============================================================================
// Recursion Bomb Tests
// ============================================================================

#[test]
fn test_deeply_nested_100_levels() {
    // Create 100 levels of nesting (exceeds 64 limit)
    let mut bytes = vec![0x08, 0x01]; // innermost: field 1 = 1

    for _ in 0..99 {
        let len = bytes.len();
        let mut wrapper = vec![0x0a]; // field 1, length-delimited
        if len < 128 {
            wrapper.push(len as u8);
        } else {
            // Encode length as varint
            let mut remaining = len;
            while remaining >= 0x80 {
                wrapper.push((remaining as u8) | 0x80);
                remaining >>= 7;
            }
            wrapper.push(remaining as u8);
        }
        wrapper.extend_from_slice(&bytes);
        bytes = wrapper;
    }

    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail at recursion limit");
}

#[test]
fn test_deeply_nested_1000_levels() {
    // Create 1000 levels of nesting
    let mut bytes = vec![0x08, 0x01];

    for _ in 0..999 {
        let len = bytes.len();
        let mut wrapper = vec![0x0a];

        // Encode length as varint
        let mut remaining = len;
        while remaining >= 0x80 {
            wrapper.push((remaining as u8) | 0x80);
            remaining >>= 7;
        }
        wrapper.push(remaining as u8);

        wrapper.extend_from_slice(&bytes);
        bytes = wrapper;
    }

    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail at recursion limit");
}

// ============================================================================
// Reserved Wire Type Tests
// ============================================================================

#[test]
fn test_wire_type_3_start_group() {
    // Wire type 3 (start group) is deprecated/invalid
    let bytes = vec![0x0b]; // tag (1 << 3) | 3
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on wire type 3");
}

#[test]
fn test_wire_type_4_end_group() {
    // Wire type 4 (end group) is deprecated/invalid
    let bytes = vec![0x0c]; // tag (1 << 3) | 4
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on wire type 4");
}

#[test]
fn test_wire_type_6_reserved() {
    // Wire type 6 doesn't exist
    let bytes = vec![0x0e]; // tag (1 << 3) | 6
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on wire type 6");
}

#[test]
fn test_wire_type_7_reserved() {
    // Wire type 7 doesn't exist
    let bytes = vec![0x0f]; // tag (1 << 3) | 7
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on wire type 7");
}

// ============================================================================
// Zero-Length Fields
// ============================================================================

#[test]
fn test_zero_length_string() {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("text", 1, FieldType::String)
        .build();

    let bytes = vec![0x0a, 0x00]; // field 1, length 0
    let result = decode_with_schema(&bytes, &desc);
    assert!(result.is_ok(), "Should handle zero-length string");

    let decoded = result.unwrap();
    let json = decoded.to_json();
    assert_eq!(json["text"], "");
}

#[test]
fn test_zero_length_bytes() {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("data", 1, FieldType::Bytes)
        .build();

    let bytes = vec![0x0a, 0x00]; // field 1, length 0
    let result = decode_with_schema(&bytes, &desc);
    assert!(result.is_ok(), "Should handle zero-length bytes");
}

// ============================================================================
// Length Overflow Tests
// ============================================================================

#[test]
fn test_length_overflow_claims_more_than_available() {
    // Length claims 1000 bytes but only 5 follow
    let bytes = vec![0x0a, 0xe8, 0x07, 0x01, 0x02, 0x03, 0x04, 0x05];
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on length overflow");
}

#[test]
fn test_length_near_max_varint() {
    // Length value near u64::MAX (unrealistic)
    let mut bytes = vec![0x0a]; // field 1
    // Encode a very large length (will fail when trying to read)
    bytes.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01]);

    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on excessive length");
}

// ============================================================================
// Mixed Malformation Tests
// ============================================================================

#[test]
fn test_valid_field_then_truncated() {
    // Valid field followed by truncated field
    let mut bytes = vec![];
    bytes.extend_from_slice(&[0x08, 0x2a]); // field 1 = 42 (valid)
    bytes.extend_from_slice(&[0x25, 0x01]); // field 4, fixed32 but only 1 byte

    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on second truncated field");
}

#[test]
fn test_multiple_field_zero() {
    // Multiple fields with number 0
    let bytes = vec![0x00, 0x01, 0x00, 0x02];
    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on field number 0");
}

#[test]
fn test_truncated_after_valid_varint() {
    let mut bytes = vec![];
    bytes.extend_from_slice(&[0x08, 0x64]); // field 1 = 100 (valid)
    bytes.extend_from_slice(&[0x29]); // field 5, fixed64 tag but no data

    let result = decode_wire_format(&bytes);
    assert!(result.is_err(), "Should fail on truncated fixed64");
}

// ============================================================================
// Edge Case: Empty Message
// ============================================================================

#[test]
fn test_completely_empty_message() {
    let bytes = vec![];
    let result = decode_wire_format(&bytes);
    assert!(result.is_ok(), "Should handle empty message");
    assert_eq!(result.unwrap().fields.len(), 0);
}

// ============================================================================
// Parameterized Truncation Tests
// ============================================================================

#[rstest]
#[case(vec![0x08])] // tag only
#[case(vec![0x25])] // fixed32 tag only
#[case(vec![0x25, 0x01])] // fixed32 with 1 byte
#[case(vec![0x25, 0x01, 0x02])] // fixed32 with 2 bytes
#[case(vec![0x25, 0x01, 0x02, 0x03])] // fixed32 with 3 bytes
#[case(vec![0x29])] // fixed64 tag only
#[case(vec![0x29, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07])] // fixed64 with 7 bytes
#[case(vec![0x0a])] // length-delimited tag only
#[case(vec![0x0a, 0x05])] // length 5 but no data
#[case(vec![0x0a, 0x05, 0x01, 0x02])] // length 5 but only 2 bytes
fn test_truncated_messages(#[case] bytes: Vec<u8>) {
    let result = decode_wire_format(&bytes);
    assert!(
        result.is_err(),
        "Should fail on truncated message: {:?}",
        bytes
    );
}

// ============================================================================
// Invalid Wire Type Combinations
// ============================================================================

#[rstest]
#[case(vec![0x0b])] // wire type 3
#[case(vec![0x0c])] // wire type 4
#[case(vec![0x0e])] // wire type 6
#[case(vec![0x0f])] // wire type 7
#[case(vec![0x13])] // field 2, wire type 3
#[case(vec![0x1c])] // field 3, wire type 4
fn test_invalid_wire_types(#[case] bytes: Vec<u8>) {
    let result = decode_wire_format(&bytes);
    assert!(
        result.is_err(),
        "Should fail on invalid wire type: {:?}",
        bytes
    );
}

// ============================================================================
// Large Field Numbers
// ============================================================================

#[test]
fn test_field_number_max_valid() {
    // Field number 536870911 (2^29 - 1, max valid field number)
    let field_num = 536870911u32;
    let tag = field_num << 3; // wire type 0

    let mut bytes = vec![];
    let mut remaining = tag;
    while remaining >= 0x80 {
        bytes.push((remaining as u8) | 0x80);
        remaining >>= 7;
    }
    bytes.push(remaining as u8);
    bytes.push(0x01); // value = 1

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok(), "Should handle max valid field number");
}

// ============================================================================
// Nested Message Malformations
// ============================================================================

#[test]
fn test_nested_message_with_invalid_content() {
    // Outer message contains length-delimited field with malformed inner message
    let mut inner = vec![0x00, 0x01]; // Invalid: field number 0
    let mut bytes = vec![0x0a]; // field 1, length-delimited
    bytes.push(inner.len() as u8);
    bytes.append(&mut inner);

    let result = decode_wire_format(&bytes);
    // May succeed at outer level but inner message is malformed
    // Wire format decoder attempts to decode submessages
    let _ = result;
}

#[test]
fn test_nested_message_truncated_inner() {
    // Outer message with length-delimited field containing truncated inner message
    let mut inner = vec![0x25]; // fixed32 tag but no data
    let mut bytes = vec![0x0a]; // field 1
    bytes.push(inner.len() as u8);
    bytes.append(&mut inner);

    let result = decode_wire_format(&bytes);
    // May parse as bytes if submessage decode fails
    let _ = result;
}

// ============================================================================
// Boundary Conditions
// ============================================================================

#[test]
fn test_exactly_max_varint_bytes() {
    // Exactly 10 bytes for a varint (max allowed)
    let bytes = vec![
        0x08, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01,
    ];
    let result = decode_wire_format(&bytes);
    assert!(result.is_ok(), "Should handle 10-byte varint");
}

#[test]
fn test_fixed32_exactly_4_bytes() {
    let bytes = vec![0x25, 0x01, 0x02, 0x03, 0x04];
    let result = decode_wire_format(&bytes);
    assert!(result.is_ok(), "Should handle fixed32 with exactly 4 bytes");
}

#[test]
fn test_fixed64_exactly_8_bytes() {
    let bytes = vec![0x29, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
    let result = decode_wire_format(&bytes);
    assert!(result.is_ok(), "Should handle fixed64 with exactly 8 bytes");
}
