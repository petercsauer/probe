//! Parameterized tests for all protobuf scalar types.
//!
//! Tests each of the 18 protobuf scalar types across 5 value ranges:
//! - Zero
//! - Minimum (for signed types)
//! - Maximum
//! - Normal positive/negative
//!
//! Uses rstest for parameterized testing with auto-generated test names.

mod helpers;

use helpers::descriptor_builder::{DescriptorBuilder, FieldType};
use prb_decode::schema_backed::decode_with_schema;
use prost::Message;
use prost_reflect::DynamicMessage;
use rstest::rstest;

// Helper to encode a message with prost-reflect
fn encode_message(msg: &DynamicMessage) -> Vec<u8> {
    let mut buf = Vec::new();
    msg.encode(&mut buf).expect("Failed to encode message");
    buf
}

// ============================================================================
// INT32 Tests
// ============================================================================

#[rstest]
#[case::zero(0i32)]
#[case::negative(-42i32)]
#[case::positive(42i32)]
#[case::min(i32::MIN)]
#[case::max(i32::MAX)]
fn test_int32_values(#[case] value: i32) {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("value", 1, FieldType::Int32)
        .build();

    let mut msg = DynamicMessage::new(desc.clone());
    msg.set_field_by_name("value", prost_reflect::Value::I32(value));

    let encoded = encode_message(&msg);
    let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

    let json = decoded.to_json();
    assert_eq!(json["value"], value);
}

// ============================================================================
// INT64 Tests
// ============================================================================

#[rstest]
#[case::zero(0i64)]
#[case::negative(-1000i64)]
#[case::positive(1000i64)]
#[case::min(i64::MIN)]
#[case::max(i64::MAX)]
fn test_int64_values(#[case] value: i64) {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("value", 1, FieldType::Int64)
        .build();

    let mut msg = DynamicMessage::new(desc.clone());
    msg.set_field_by_name("value", prost_reflect::Value::I64(value));

    let encoded = encode_message(&msg);
    let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

    let json = decoded.to_json();
    assert_eq!(json["value"], value);
}

// ============================================================================
// UINT32 Tests
// ============================================================================

#[rstest]
#[case::zero(0u32)]
#[case::normal(12345u32)]
#[case::large(999999u32)]
#[case::max(u32::MAX)]
fn test_uint32_values(#[case] value: u32) {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("value", 1, FieldType::Uint32)
        .build();

    let mut msg = DynamicMessage::new(desc.clone());
    msg.set_field_by_name("value", prost_reflect::Value::U32(value));

    let encoded = encode_message(&msg);
    let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

    let json = decoded.to_json();
    assert_eq!(json["value"], value);
}

// ============================================================================
// UINT64 Tests
// ============================================================================

#[rstest]
#[case::zero(0u64)]
#[case::normal(999999u64)]
#[case::large(999999999999u64)]
#[case::max(u64::MAX)]
fn test_uint64_values(#[case] value: u64) {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("value", 1, FieldType::Uint64)
        .build();

    let mut msg = DynamicMessage::new(desc.clone());
    msg.set_field_by_name("value", prost_reflect::Value::U64(value));

    let encoded = encode_message(&msg);
    let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

    let json = decoded.to_json();
    assert_eq!(json["value"], value);
}

// ============================================================================
// SINT32 Tests (ZigZag encoded)
// ============================================================================

#[rstest]
#[case::zero(0i32)]
#[case::negative(-42i32)]
#[case::positive(42i32)]
#[case::min(i32::MIN)]
#[case::max(i32::MAX)]
fn test_sint32_values(#[case] value: i32) {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("value", 1, FieldType::Sint32)
        .build();

    let mut msg = DynamicMessage::new(desc.clone());
    msg.set_field_by_name("value", prost_reflect::Value::I32(value));

    let encoded = encode_message(&msg);
    let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

    let json = decoded.to_json();
    assert_eq!(json["value"], value);
}

// ============================================================================
// SINT64 Tests (ZigZag encoded)
// ============================================================================

#[rstest]
#[case::zero(0i64)]
#[case::negative(-1000i64)]
#[case::positive(1000i64)]
#[case::min(i64::MIN)]
#[case::max(i64::MAX)]
fn test_sint64_values(#[case] value: i64) {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("value", 1, FieldType::Sint64)
        .build();

    let mut msg = DynamicMessage::new(desc.clone());
    msg.set_field_by_name("value", prost_reflect::Value::I64(value));

    let encoded = encode_message(&msg);
    let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

    let json = decoded.to_json();
    assert_eq!(json["value"], value);
}

// ============================================================================
// BOOL Tests
// ============================================================================

#[rstest]
#[case::false_value(false)]
#[case::true_value(true)]
fn test_bool_values(#[case] value: bool) {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("value", 1, FieldType::Bool)
        .build();

    let mut msg = DynamicMessage::new(desc.clone());
    msg.set_field_by_name("value", prost_reflect::Value::Bool(value));

    let encoded = encode_message(&msg);
    let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

    let json = decoded.to_json();
    assert_eq!(json["value"], value);
}

// ============================================================================
// STRING Tests
// ============================================================================

#[rstest]
#[case::empty("")]
#[case::simple("hello")]
#[case::unicode("Hello 世界 🌍")]
#[case::long(&"a".repeat(1000))]
fn test_string_values(#[case] value: &str) {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("value", 1, FieldType::String)
        .build();

    let mut msg = DynamicMessage::new(desc.clone());
    msg.set_field_by_name("value", prost_reflect::Value::String(value.to_string()));

    let encoded = encode_message(&msg);
    let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

    let json = decoded.to_json();
    assert_eq!(json["value"], value);
}

// ============================================================================
// BYTES Tests
// ============================================================================

#[rstest]
#[case::empty(vec![])]
#[case::simple(vec![0x01, 0x02, 0x03, 0x04])]
#[case::binary(vec![0xff, 0xfe, 0xfd, 0xfc])]
#[case::large((0..=255u8).cycle().take(1000).collect())]
fn test_bytes_values(#[case] value: Vec<u8>) {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("value", 1, FieldType::Bytes)
        .build();

    let mut msg = DynamicMessage::new(desc.clone());
    msg.set_field_by_name("value", prost_reflect::Value::Bytes(value.clone().into()));

    let encoded = encode_message(&msg);
    let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

    let json = decoded.to_json();
    // Bytes are base64-encoded in JSON
    let decoded_base64 = json["value"].as_str().expect("Expected base64 string");
    let decoded_bytes =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, decoded_base64)
            .expect("Failed to decode base64");
    assert_eq!(decoded_bytes, value);
}

// ============================================================================
// FIXED32 Tests
// ============================================================================

#[rstest]
#[case::zero(0u32)]
#[case::normal(12345u32)]
#[case::large(4000000000u32)]
#[case::max(u32::MAX)]
fn test_fixed32_values(#[case] value: u32) {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("value", 1, FieldType::Fixed32)
        .build();

    let mut msg = DynamicMessage::new(desc.clone());
    msg.set_field_by_name("value", prost_reflect::Value::U32(value));

    let encoded = encode_message(&msg);
    let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

    let json = decoded.to_json();
    assert_eq!(json["value"], value);
}

// ============================================================================
// FIXED64 Tests
// ============================================================================

#[rstest]
#[case::zero(0u64)]
#[case::normal(999999u64)]
#[case::large(18000000000000000000u64)]
#[case::max(u64::MAX)]
fn test_fixed64_values(#[case] value: u64) {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("value", 1, FieldType::Fixed64)
        .build();

    let mut msg = DynamicMessage::new(desc.clone());
    msg.set_field_by_name("value", prost_reflect::Value::U64(value));

    let encoded = encode_message(&msg);
    let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

    let json = decoded.to_json();
    assert_eq!(json["value"], value);
}

// ============================================================================
// SFIXED32 Tests
// ============================================================================

#[rstest]
#[case::zero(0i32)]
#[case::negative(-42i32)]
#[case::positive(42i32)]
#[case::min(i32::MIN)]
#[case::max(i32::MAX)]
fn test_sfixed32_values(#[case] value: i32) {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("value", 1, FieldType::Sfixed32)
        .build();

    let mut msg = DynamicMessage::new(desc.clone());
    msg.set_field_by_name("value", prost_reflect::Value::I32(value));

    let encoded = encode_message(&msg);
    let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

    let json = decoded.to_json();
    assert_eq!(json["value"], value);
}

// ============================================================================
// SFIXED64 Tests
// ============================================================================

#[rstest]
#[case::zero(0i64)]
#[case::negative(-1000i64)]
#[case::positive(1000i64)]
#[case::min(i64::MIN)]
#[case::max(i64::MAX)]
fn test_sfixed64_values(#[case] value: i64) {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("value", 1, FieldType::Sfixed64)
        .build();

    let mut msg = DynamicMessage::new(desc.clone());
    msg.set_field_by_name("value", prost_reflect::Value::I64(value));

    let encoded = encode_message(&msg);
    let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

    let json = decoded.to_json();
    assert_eq!(json["value"], value);
}

// ============================================================================
// FLOAT Tests
// ============================================================================

#[rstest]
#[case::zero(0.0f32)]
#[case::negative(-3.14f32)]
#[case::positive(3.14f32)]
#[case::large(1e20f32)]
#[case::small(1e-20f32)]
fn test_float_values(#[case] value: f32) {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("value", 1, FieldType::Float)
        .build();

    let mut msg = DynamicMessage::new(desc.clone());
    msg.set_field_by_name("value", prost_reflect::Value::F32(value));

    let encoded = encode_message(&msg);
    let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

    let json = decoded.to_json();
    // Floating point comparison with tolerance
    let decoded_f32 = json["value"].as_f64().expect("Expected float") as f32;
    assert!((decoded_f32 - value).abs() < 1e-6 * value.abs().max(1.0));
}

// ============================================================================
// DOUBLE Tests
// ============================================================================

#[rstest]
#[case::zero(0.0f64)]
#[case::negative(-2.718f64)]
#[case::positive(2.718f64)]
#[case::large(1e100f64)]
#[case::small(1e-100f64)]
fn test_double_values(#[case] value: f64) {
    let desc = DescriptorBuilder::message("TestMsg")
        .field("value", 1, FieldType::Double)
        .build();

    let mut msg = DynamicMessage::new(desc.clone());
    msg.set_field_by_name("value", prost_reflect::Value::F64(value));

    let encoded = encode_message(&msg);
    let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

    let json = decoded.to_json();
    let decoded_f64 = json["value"].as_f64().expect("Expected double");
    assert!((decoded_f64 - value).abs() < 1e-12 * value.abs().max(1.0));
}

// ============================================================================
// REPEATED Tests
// ============================================================================

#[rstest]
#[case::empty(vec![])]
#[case::single(vec!["one".to_string()])]
#[case::multiple(vec!["one".to_string(), "two".to_string(), "three".to_string()])]
#[case::many((0..100).map(|i| i.to_string()).collect())]
fn test_repeated_string(#[case] values: Vec<String>) {
    let desc = DescriptorBuilder::message("TestMsg")
        .repeated_field("values", 1, FieldType::String)
        .build();

    let mut msg = DynamicMessage::new(desc.clone());
    let list_values: Vec<prost_reflect::Value> = values
        .iter()
        .map(|s| prost_reflect::Value::String(s.clone()))
        .collect();
    msg.set_field_by_name("values", prost_reflect::Value::List(list_values));

    let encoded = encode_message(&msg);
    let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

    let json = decoded.to_json();
    let decoded_array = json["values"].as_array().expect("Expected array");
    assert_eq!(decoded_array.len(), values.len());
    for (i, val) in values.iter().enumerate() {
        assert_eq!(decoded_array[i].as_str().expect("Expected string"), val);
    }
}

#[rstest]
#[case::empty(vec![])]
#[case::single(vec![42])]
#[case::multiple(vec![1, 2, 3, 4, 5])]
#[case::many((0..100).collect())]
fn test_repeated_int32(#[case] values: Vec<i32>) {
    let desc = DescriptorBuilder::message("TestMsg")
        .repeated_field("values", 1, FieldType::Int32)
        .build();

    let mut msg = DynamicMessage::new(desc.clone());
    let list_values: Vec<prost_reflect::Value> = values
        .iter()
        .map(|&i| prost_reflect::Value::I32(i))
        .collect();
    msg.set_field_by_name("values", prost_reflect::Value::List(list_values));

    let encoded = encode_message(&msg);
    let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

    let json = decoded.to_json();
    let decoded_array = json["values"].as_array().expect("Expected array");
    assert_eq!(decoded_array.len(), values.len());
    for (i, &val) in values.iter().enumerate() {
        assert_eq!(
            decoded_array[i].as_i64().expect("Expected int"),
            i64::from(val)
        );
    }
}
