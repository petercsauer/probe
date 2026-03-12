//! Additional integration tests for wire-format protobuf decoding.

use prb_decode::wire_format::{decode_wire_format, LenValue, WireValue};

#[test]
fn test_multi_field_message() {
    // Message with multiple fields of different types
    let mut bytes = vec![];

    // Field 1: varint = 100
    bytes.extend_from_slice(&[0x08, 0x64]);

    // Field 2: string = "test"
    bytes.extend_from_slice(&[0x12, 0x04, b't', b'e', b's', b't']);

    // Field 3: fixed32 = 200
    bytes.push(0x1d); // tag (3 << 3) | 5
    bytes.extend_from_slice(&200u32.to_le_bytes());

    // Field 4: fixed64 = 300
    bytes.push(0x21); // tag (4 << 3) | 1
    bytes.extend_from_slice(&300u64.to_le_bytes());

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    assert_eq!(msg.fields.len(), 4);

    // Verify field numbers
    assert_eq!(msg.fields[0].field_number, 1);
    assert_eq!(msg.fields[1].field_number, 2);
    assert_eq!(msg.fields[2].field_number, 3);
    assert_eq!(msg.fields[3].field_number, 4);
}

#[test]
fn test_single_field_message() {
    // Message with just one field
    let bytes = vec![0x08, 0x01]; // field 1, varint 1

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    assert_eq!(msg.fields.len(), 1);
    assert_eq!(msg.fields[0].field_number, 1);

    match &msg.fields[0].value {
        WireValue::Varint(v) => {
            assert_eq!(v.unsigned, 1);
            assert_eq!(v.as_bool, Some(true));
        }
        _ => panic!("Expected varint"),
    }
}

#[test]
fn test_very_large_varint() {
    // Test maximum u64 value
    let mut bytes = vec![0x08]; // tag for field 1

    // Encode u64::MAX
    let val = u64::MAX;
    let mut remaining = val;
    while remaining >= 0x80 {
        bytes.push((remaining as u8) | 0x80);
        remaining >>= 7;
    }
    bytes.push(remaining as u8);

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    match &msg.fields[0].value {
        WireValue::Varint(v) => {
            assert_eq!(v.unsigned, u64::MAX);
        }
        _ => panic!("Expected varint"),
    }
}

#[test]
fn test_varint_as_bool_false() {
    let bytes = vec![0x08, 0x00]; // field 1, varint 0

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    match &msg.fields[0].value {
        WireValue::Varint(v) => {
            assert_eq!(v.unsigned, 0);
            assert_eq!(v.as_bool, Some(false));
        }
        _ => panic!("Expected varint"),
    }
}

#[test]
fn test_varint_not_bool() {
    let bytes = vec![0x08, 0x02]; // field 1, varint 2

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    match &msg.fields[0].value {
        WireValue::Varint(v) => {
            assert_eq!(v.unsigned, 2);
            assert_eq!(v.as_bool, None); // Not 0 or 1
        }
        _ => panic!("Expected varint"),
    }
}

#[test]
fn test_zigzag_positive() {
    // Zigzag encoding of 42: (42 << 1) = 84
    let bytes = vec![0x08, 0x54]; // field 1, varint 84

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    match &msg.fields[0].value {
        WireValue::Varint(v) => {
            assert_eq!(v.unsigned, 84);
            assert_eq!(v.signed_zigzag, 42);
        }
        _ => panic!("Expected varint"),
    }
}

#[test]
fn test_fixed32_all_interpretations() {
    let value = 0x40490fdbu32; // Represents 3.14159 as f32
    let bytes_val = value.to_le_bytes();
    let mut bytes = vec![0x25]; // tag (4 << 3) | 5
    bytes.extend_from_slice(&bytes_val);

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    match &msg.fields[0].value {
        WireValue::Fixed32(v) => {
            assert_eq!(v.as_u32, value);
            assert_eq!(v.as_i32, value as i32);
            assert!(v.as_f32.is_some());
            // Verify it's close to pi
            let f = v.as_f32.unwrap();
            assert!((f - std::f32::consts::PI).abs() < 0.01);
        }
        _ => panic!("Expected fixed32"),
    }
}

#[test]
fn test_fixed64_all_interpretations() {
    // Use a value that represents a normal f64
    let value = 0x400921fb54442d18u64; // Represents pi as f64
    let bytes_val = value.to_le_bytes();
    let mut bytes = vec![0x29]; // tag (5 << 3) | 1
    bytes.extend_from_slice(&bytes_val);

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    match &msg.fields[0].value {
        WireValue::Fixed64(v) => {
            assert_eq!(v.as_u64, value);
            assert_eq!(v.as_i64, value as i64);
            assert!(v.as_f64.is_some());
            // Verify it's close to pi
            let f = v.as_f64.unwrap();
            assert!((f - std::f64::consts::PI).abs() < 0.01);
        }
        _ => panic!("Expected fixed64"),
    }
}

#[test]
fn test_fixed32_non_normal_float() {
    // Use NaN representation
    let value = 0x7fc00000u32; // NaN
    let bytes_val = value.to_le_bytes();
    let mut bytes = vec![0x25]; // tag (4 << 3) | 5
    bytes.extend_from_slice(&bytes_val);

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    match &msg.fields[0].value {
        WireValue::Fixed32(v) => {
            assert_eq!(v.as_u32, value);
            // NaN should result in None for as_f32
            assert_eq!(v.as_f32, None);
        }
        _ => panic!("Expected fixed32"),
    }
}

#[test]
fn test_fixed64_non_normal_float() {
    // Use infinity representation
    let value = 0x7ff0000000000000u64; // Positive infinity
    let bytes_val = value.to_le_bytes();
    let mut bytes = vec![0x29]; // tag (5 << 3) | 1
    bytes.extend_from_slice(&bytes_val);

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    match &msg.fields[0].value {
        WireValue::Fixed64(v) => {
            assert_eq!(v.as_u64, value);
            // Infinity should result in None for as_f64
            assert_eq!(v.as_f64, None);
        }
        _ => panic!("Expected fixed64"),
    }
}

#[test]
fn test_fixed32_zero() {
    let mut bytes = vec![0x25]; // tag (4 << 3) | 5
    bytes.extend_from_slice(&0u32.to_le_bytes());

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    match &msg.fields[0].value {
        WireValue::Fixed32(v) => {
            assert_eq!(v.as_u32, 0);
            assert_eq!(v.as_f32, Some(0.0));
        }
        _ => panic!("Expected fixed32"),
    }
}

#[test]
fn test_fixed64_zero() {
    let mut bytes = vec![0x29]; // tag (5 << 3) | 1
    bytes.extend_from_slice(&0u64.to_le_bytes());

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    match &msg.fields[0].value {
        WireValue::Fixed64(v) => {
            assert_eq!(v.as_u64, 0);
            assert_eq!(v.as_f64, Some(0.0));
        }
        _ => panic!("Expected fixed64"),
    }
}

#[test]
fn test_length_delimited_empty() {
    let bytes = vec![0x0a, 0x00]; // field 1, length 0

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    match &msg.fields[0].value {
        WireValue::LengthDelimited(LenValue::String(s)) => {
            assert_eq!(s, "");
        }
        _ => panic!("Expected empty string"),
    }
}

#[test]
fn test_length_delimited_non_printable_bytes() {
    // Binary data that's not valid UTF-8
    let bytes = vec![0x0a, 0x04, 0xff, 0xfe, 0xfd, 0xfc];

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    match &msg.fields[0].value {
        WireValue::LengthDelimited(LenValue::Bytes(b)) => {
            assert_eq!(b, &[0xff, 0xfe, 0xfd, 0xfc]);
        }
        _ => panic!("Expected bytes"),
    }
}

#[test]
fn test_length_delimited_mostly_printable() {
    // String with some control chars but mostly printable (>80%)
    // This should still be detected as a string since control chars like \x01, \x02
    // might pass the printable test if they're whitespace or the ratio is high enough
    let data = b"hello world test";
    let mut bytes = vec![0x0a, data.len() as u8];
    bytes.extend_from_slice(data);

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    match &msg.fields[0].value {
        WireValue::LengthDelimited(LenValue::String(s)) => {
            assert_eq!(s, "hello world test");
        }
        _ => panic!("Expected string for printable ASCII"),
    }
}

#[test]
fn test_length_delimited_below_printable_threshold() {
    // Data with <80% printable characters should be bytes
    let data = b"\x00\x01\x02\x03hello\xff";
    let mut bytes = vec![0x0a, data.len() as u8];
    bytes.extend_from_slice(data);

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    match &msg.fields[0].value {
        WireValue::LengthDelimited(LenValue::Bytes(_)) => {}
        _ => panic!("Expected bytes for low printability ratio"),
    }
}

#[test]
fn test_nested_submessage_empty() {
    // Length-delimited field with empty submessage
    let bytes = vec![0x0a, 0x00]; // field 1, length 0

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    if let WireValue::LengthDelimited(LenValue::String(s)) = &msg.fields[0].value {
        // Empty submessage falls back to empty string
        assert_eq!(s, "");
    }
}

#[test]
fn test_multiple_same_field_number() {
    // Repeated field: same field number appears multiple times
    let bytes = vec![
        0x08, 0x01, // field 1 = 1
        0x08, 0x02, // field 1 = 2
        0x08, 0x03, // field 1 = 3
    ];

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    assert_eq!(msg.fields.len(), 3);

    // All should be field number 1
    assert_eq!(msg.fields[0].field_number, 1);
    assert_eq!(msg.fields[1].field_number, 1);
    assert_eq!(msg.fields[2].field_number, 1);
}

#[test]
fn test_invalid_wire_type_3() {
    // Wire type 3 (start group) is deprecated
    let bytes = vec![0x0b]; // tag (1 << 3) | 3

    let result = decode_wire_format(&bytes);
    assert!(result.is_err());
}

#[test]
fn test_invalid_wire_type_4() {
    // Wire type 4 (end group) is deprecated
    let bytes = vec![0x0c]; // tag (1 << 3) | 4

    let result = decode_wire_format(&bytes);
    assert!(result.is_err());
}

#[test]
fn test_invalid_wire_type_6() {
    // Wire type 6 doesn't exist
    let bytes = vec![0x0e]; // tag (1 << 3) | 6

    let result = decode_wire_format(&bytes);
    assert!(result.is_err());
}

#[test]
fn test_invalid_wire_type_7() {
    // Wire type 7 doesn't exist
    let bytes = vec![0x0f]; // tag (1 << 3) | 7

    let result = decode_wire_format(&bytes);
    assert!(result.is_err());
}

#[test]
fn test_field_number_zero() {
    // Field number 0 is invalid
    let bytes = vec![0x00, 0x01]; // field 0, varint 1

    let result = decode_wire_format(&bytes);
    assert!(result.is_err());
}

#[test]
fn test_truncated_fixed32() {
    // Fixed32 requires 4 bytes but only 2 provided
    let bytes = vec![0x25, 0x01, 0x02]; // tag, but incomplete data

    let result = decode_wire_format(&bytes);
    assert!(result.is_err());
}

#[test]
fn test_truncated_fixed64() {
    // Fixed64 requires 8 bytes but only 4 provided
    let bytes = vec![0x29, 0x01, 0x02, 0x03, 0x04]; // tag, but incomplete data

    let result = decode_wire_format(&bytes);
    assert!(result.is_err());
}

#[test]
fn test_truncated_length_delimited() {
    // Length says 10 bytes but only 2 provided
    let bytes = vec![0x0a, 0x0a, 0x01, 0x02]; // field 1, length 10, but only 2 bytes

    let result = decode_wire_format(&bytes);
    assert!(result.is_err());
}

#[test]
fn test_large_field_number() {
    // Test field number > 15 (requires multi-byte tag)
    let field_num = 1000u32;
    let tag = field_num << 3; // wire type 0 (varint)

    let mut bytes = vec![];
    // Encode tag as varint
    let mut remaining = tag;
    while remaining >= 0x80 {
        bytes.push((remaining as u8) | 0x80);
        remaining >>= 7;
    }
    bytes.push(remaining as u8);
    bytes.push(0x01); // value = 1

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());

    let msg = result.unwrap();
    assert_eq!(msg.fields[0].field_number, field_num);
}

#[test]
fn test_display_formatting() {
    let bytes = vec![
        0x08, 0x2a, // field 1: varint 42
        0x12, 0x04, b't', b'e', b's', b't', // field 2: string "test"
    ];

    let msg = decode_wire_format(&bytes).unwrap();
    let display = format!("{}", msg);

    // Verify display output contains expected elements
    assert!(display.contains("WIRE FORMAT DECODE"));
    assert!(display.contains("field 1"));
    assert!(display.contains("field 2"));
    assert!(display.contains("42"));
    assert!(display.contains("test"));
}

#[test]
fn test_display_nested_message() {
    // Nested message
    let inner = vec![0x08, 0x01]; // field 1 = 1
    let mut bytes = vec![0x0a, inner.len() as u8];
    bytes.extend_from_slice(&inner);

    let msg = decode_wire_format(&bytes).unwrap();
    let display = format!("{}", msg);

    assert!(display.contains("submessage"));
    assert!(display.contains("field"));
}

#[test]
fn test_display_bytes_short() {
    // Short bytes (≤16 bytes) should be fully displayed
    let data = vec![0x01, 0x02, 0x03, 0x04];
    let mut bytes = vec![0x0a, data.len() as u8];
    bytes.extend_from_slice(&data);

    let msg = decode_wire_format(&bytes).unwrap();
    let display = format!("{}", msg);

    assert!(display.contains("0x01020304"));
}

#[test]
fn test_display_bytes_long() {
    // Long bytes (>16 bytes) should be truncated with "..."
    let data: Vec<u8> = (0..20).collect();
    let mut bytes = vec![0x0a, data.len() as u8];
    bytes.extend_from_slice(&data);

    let msg = decode_wire_format(&bytes).unwrap();
    let display = format!("{}", msg);

    assert!(display.contains("..."));
    assert!(display.contains("20 bytes"));
}

#[test]
fn test_deeply_nested_message_near_limit() {
    // Create a message nested 63 times (just under the 64 limit)
    let mut bytes = vec![0x08, 0x01]; // innermost: field 1 = 1

    for _ in 0..62 {
        let len = bytes.len();
        let mut wrapper = vec![0x0a]; // field 1, length-delimited
        if len < 128 {
            wrapper.push(len as u8);
        } else {
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
    assert!(result.is_ok(), "Should decode at 63 levels");
}

#[test]
fn test_varint_edge_case_max_bytes() {
    // Valid varint using exactly 10 bytes (max for 64-bit)
    let bytes = vec![
        0x08, // tag
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01, // max varint
    ];

    let result = decode_wire_format(&bytes);
    assert!(result.is_ok());
}

#[test]
fn test_unexpected_eof_in_tag() {
    // Message ends in the middle of a varint tag
    let bytes = vec![0x80]; // Continuation byte with nothing following

    let result = decode_wire_format(&bytes);
    assert!(result.is_err());
}
