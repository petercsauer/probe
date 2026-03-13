//! Property-based tests for protobuf round-trip encoding/decoding.
//!
//! Uses proptest to generate arbitrary protobuf messages and verify that
//! encoding followed by decoding produces the same data.

mod helpers;

use helpers::descriptor_builder::{DescriptorBuilder, FieldType};
use prb_decode::schema_backed::decode_with_schema;
use proptest::prelude::*;
use prost::Message;
use prost_reflect::{DynamicMessage, Value};

// Helper to encode a message
fn encode_message(msg: &DynamicMessage) -> Vec<u8> {
    let mut buf = Vec::new();
    msg.encode(&mut buf).expect("Failed to encode");
    buf
}

// Strategy for generating arbitrary i32 values
fn arb_i32() -> impl Strategy<Value = i32> {
    any::<i32>()
}

// Strategy for generating arbitrary i64 values
fn arb_i64() -> impl Strategy<Value = i64> {
    any::<i64>()
}

// Strategy for generating arbitrary u32 values
fn arb_u32() -> impl Strategy<Value = u32> {
    any::<u32>()
}

// Strategy for generating arbitrary u64 values
fn arb_u64() -> impl Strategy<Value = u64> {
    any::<u64>()
}

// Strategy for generating arbitrary bool values
fn arb_bool() -> impl Strategy<Value = bool> {
    any::<bool>()
}

// Strategy for generating arbitrary strings
fn arb_string() -> impl Strategy<Value = String> {
    ".*".prop_filter("valid UTF-8", |s| s.len() <= 1000)
}

// Strategy for generating arbitrary bytes
fn arb_bytes() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..1000)
}

// Strategy for generating arbitrary f32 values (excluding NaN/Inf)
fn arb_f32() -> impl Strategy<Value = f32> {
    any::<f32>().prop_filter("not NaN or Inf", |f| f.is_finite())
}

// Strategy for generating arbitrary f64 values (excluding NaN/Inf)
fn arb_f64() -> impl Strategy<Value = f64> {
    any::<f64>().prop_filter("not NaN or Inf", |f| f.is_finite())
}

// ============================================================================
// Round-trip property tests for each type
// ============================================================================

proptest! {
    #[test]
    fn roundtrip_int32(value in arb_i32()) {
        let desc = DescriptorBuilder::message("TestMsg")
            .field("value", 1, FieldType::Int32)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        msg.set_field_by_name("value", Value::I32(value));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        prop_assert_eq!(&json["value"], value);
    }

    #[test]
    fn roundtrip_int64(value in arb_i64()) {
        let desc = DescriptorBuilder::message("TestMsg")
            .field("value", 1, FieldType::Int64)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        msg.set_field_by_name("value", Value::I64(value));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        prop_assert_eq!(&json["value"], value);
    }

    #[test]
    fn roundtrip_uint32(value in arb_u32()) {
        let desc = DescriptorBuilder::message("TestMsg")
            .field("value", 1, FieldType::Uint32)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        msg.set_field_by_name("value", Value::U32(value));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        prop_assert_eq!(&json["value"], value);
    }

    #[test]
    fn roundtrip_uint64(value in arb_u64()) {
        let desc = DescriptorBuilder::message("TestMsg")
            .field("value", 1, FieldType::Uint64)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        msg.set_field_by_name("value", Value::U64(value));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        prop_assert_eq!(&json["value"], value);
    }

    #[test]
    fn roundtrip_sint32(value in arb_i32()) {
        let desc = DescriptorBuilder::message("TestMsg")
            .field("value", 1, FieldType::Sint32)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        msg.set_field_by_name("value", Value::I32(value));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        prop_assert_eq!(&json["value"], value);
    }

    #[test]
    fn roundtrip_sint64(value in arb_i64()) {
        let desc = DescriptorBuilder::message("TestMsg")
            .field("value", 1, FieldType::Sint64)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        msg.set_field_by_name("value", Value::I64(value));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        prop_assert_eq!(&json["value"], value);
    }

    #[test]
    fn roundtrip_bool(value in arb_bool()) {
        let desc = DescriptorBuilder::message("TestMsg")
            .field("value", 1, FieldType::Bool)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        msg.set_field_by_name("value", Value::Bool(value));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        prop_assert_eq!(&json["value"], value);
    }

    #[test]
    fn roundtrip_string(value in arb_string()) {
        let desc = DescriptorBuilder::message("TestMsg")
            .field("value", 1, FieldType::String)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        msg.set_field_by_name("value", Value::String(value.clone()));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        prop_assert_eq!(json["value"].as_str().expect("Expected string"), value.as_str());
    }

    #[test]
    fn roundtrip_bytes(value in arb_bytes()) {
        let desc = DescriptorBuilder::message("TestMsg")
            .field("value", 1, FieldType::Bytes)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        msg.set_field_by_name("value", Value::Bytes(value.clone().into()));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        // Bytes are base64-encoded in JSON
        let decoded_base64 = json["value"].as_str().expect("Expected base64 string");
        let decoded_bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            decoded_base64,
        )
        .expect("Failed to decode base64");
        prop_assert_eq!(decoded_bytes, value);
    }

    #[test]
    fn roundtrip_fixed32(value in arb_u32()) {
        let desc = DescriptorBuilder::message("TestMsg")
            .field("value", 1, FieldType::Fixed32)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        msg.set_field_by_name("value", Value::U32(value));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        prop_assert_eq!(&json["value"], value);
    }

    #[test]
    fn roundtrip_fixed64(value in arb_u64()) {
        let desc = DescriptorBuilder::message("TestMsg")
            .field("value", 1, FieldType::Fixed64)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        msg.set_field_by_name("value", Value::U64(value));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        prop_assert_eq!(&json["value"], value);
    }

    #[test]
    fn roundtrip_sfixed32(value in arb_i32()) {
        let desc = DescriptorBuilder::message("TestMsg")
            .field("value", 1, FieldType::Sfixed32)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        msg.set_field_by_name("value", Value::I32(value));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        prop_assert_eq!(&json["value"], value);
    }

    #[test]
    fn roundtrip_sfixed64(value in arb_i64()) {
        let desc = DescriptorBuilder::message("TestMsg")
            .field("value", 1, FieldType::Sfixed64)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        msg.set_field_by_name("value", Value::I64(value));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        prop_assert_eq!(&json["value"], value);
    }

    #[test]
    fn roundtrip_float(value in arb_f32()) {
        let desc = DescriptorBuilder::message("TestMsg")
            .field("value", 1, FieldType::Float)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        msg.set_field_by_name("value", Value::F32(value));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        let decoded_f32 = json["value"].as_f64().expect("Expected float") as f32;
        // Floating point round-trip may have slight precision loss
        prop_assert!((decoded_f32 - value).abs() < 1e-6 * value.abs().max(1.0));
    }

    #[test]
    fn roundtrip_double(value in arb_f64()) {
        let desc = DescriptorBuilder::message("TestMsg")
            .field("value", 1, FieldType::Double)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        msg.set_field_by_name("value", Value::F64(value));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        let decoded_f64 = json["value"].as_f64().expect("Expected double");
        // Floating point round-trip may have slight precision loss
        prop_assert!((decoded_f64 - value).abs() < 1e-12 * value.abs().max(1.0));
    }

    // ========================================================================
    // Multi-field messages
    // ========================================================================

    #[test]
    fn roundtrip_multi_field(
        int_val in arb_i32(),
        str_val in arb_string(),
        bool_val in arb_bool()
    ) {
        let desc = DescriptorBuilder::message("TestMsg")
            .field("int_field", 1, FieldType::Int32)
            .field("str_field", 2, FieldType::String)
            .field("bool_field", 3, FieldType::Bool)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        msg.set_field_by_name("int_field", Value::I32(int_val));
        msg.set_field_by_name("str_field", Value::String(str_val.clone()));
        msg.set_field_by_name("bool_field", Value::Bool(bool_val));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        prop_assert_eq!(&json["int_field"], &int_val);
        prop_assert_eq!(json["str_field"].as_str().expect("Expected string"), str_val.as_str());
        prop_assert_eq!(&json["bool_field"], &bool_val);
    }

    // ========================================================================
    // Repeated fields
    // ========================================================================

    #[test]
    fn roundtrip_repeated_int32(values in prop::collection::vec(arb_i32(), 0..20)) {
        let desc = DescriptorBuilder::message("TestMsg")
            .repeated_field("values", 1, FieldType::Int32)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        let list_values: Vec<Value> = values.iter().map(|&i| Value::I32(i)).collect();
        msg.set_field_by_name("values", Value::List(list_values));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        let decoded_array = json["values"].as_array().expect("Expected array");
        prop_assert_eq!(decoded_array.len(), values.len());

        for (i, &val) in values.iter().enumerate() {
            prop_assert_eq!(
                decoded_array[i].as_i64().expect("Expected int"),
                i64::from(val)
            );
        }
    }

    #[test]
    fn roundtrip_repeated_string(values in prop::collection::vec(arb_string(), 0..20)) {
        let desc = DescriptorBuilder::message("TestMsg")
            .repeated_field("values", 1, FieldType::String)
            .build();

        let mut msg = DynamicMessage::new(desc.clone());
        let list_values: Vec<Value> = values
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        msg.set_field_by_name("values", Value::List(list_values));

        let encoded = encode_message(&msg);
        let decoded = decode_with_schema(&encoded, &desc).expect("Decode failed");

        let json = decoded.to_json();
        let decoded_array = json["values"].as_array().expect("Expected array");
        prop_assert_eq!(decoded_array.len(), values.len());

        for (i, val) in values.iter().enumerate() {
            prop_assert_eq!(
                decoded_array[i].as_str().expect("Expected string"),
                val
            );
        }
    }
}
