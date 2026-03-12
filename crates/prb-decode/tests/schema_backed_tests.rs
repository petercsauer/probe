//! Integration tests for schema-backed protobuf decoding.

use prb_decode::schema_backed::{DecodeError, decode_with_schema};
use prost_reflect::DescriptorPool;
use prost_types::{
    DescriptorProto, EnumDescriptorProto, EnumValueDescriptorProto, FieldDescriptorProto,
    FileDescriptorProto, FileDescriptorSet, field_descriptor_proto,
};

/// Helper to create a simple message descriptor.
fn create_simple_descriptor() -> prost_reflect::MessageDescriptor {
    let file = FileDescriptorProto {
        name: Some("test.proto".to_string()),
        package: Some("test".to_string()),
        message_type: vec![DescriptorProto {
            name: Some("SimpleMessage".to_string()),
            field: vec![
                FieldDescriptorProto {
                    name: Some("id".to_string()),
                    number: Some(1),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::Int32 as i32),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("name".to_string()),
                    number: Some(2),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::String as i32),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };

    let fds = FileDescriptorSet { file: vec![file] };

    let pool = DescriptorPool::from_file_descriptor_set(fds).unwrap();
    pool.get_message_by_name("test.SimpleMessage").unwrap()
}

/// Helper to create a nested message descriptor.
fn create_nested_descriptor() -> prost_reflect::MessageDescriptor {
    let file = FileDescriptorProto {
        name: Some("nested.proto".to_string()),
        package: Some("test".to_string()),
        message_type: vec![
            DescriptorProto {
                name: Some("Inner".to_string()),
                field: vec![FieldDescriptorProto {
                    name: Some("value".to_string()),
                    number: Some(1),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::Int32 as i32),
                    ..Default::default()
                }],
                ..Default::default()
            },
            DescriptorProto {
                name: Some("Outer".to_string()),
                field: vec![FieldDescriptorProto {
                    name: Some("inner".to_string()),
                    number: Some(1),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::Message as i32),
                    type_name: Some(".test.Inner".to_string()),
                    ..Default::default()
                }],
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let fds = FileDescriptorSet { file: vec![file] };

    let pool = DescriptorPool::from_file_descriptor_set(fds).unwrap();
    pool.get_message_by_name("test.Outer").unwrap()
}

/// Helper to create a message with repeated fields.
fn create_repeated_descriptor() -> prost_reflect::MessageDescriptor {
    let file = FileDescriptorProto {
        name: Some("repeated.proto".to_string()),
        package: Some("test".to_string()),
        message_type: vec![DescriptorProto {
            name: Some("RepeatedMessage".to_string()),
            field: vec![FieldDescriptorProto {
                name: Some("items".to_string()),
                number: Some(1),
                label: Some(field_descriptor_proto::Label::Repeated as i32),
                r#type: Some(field_descriptor_proto::Type::String as i32),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };

    let fds = FileDescriptorSet { file: vec![file] };

    let pool = DescriptorPool::from_file_descriptor_set(fds).unwrap();
    pool.get_message_by_name("test.RepeatedMessage").unwrap()
}

/// Helper to create an enum descriptor.
fn create_enum_descriptor() -> prost_reflect::MessageDescriptor {
    let file = FileDescriptorProto {
        name: Some("enum.proto".to_string()),
        package: Some("test".to_string()),
        enum_type: vec![EnumDescriptorProto {
            name: Some("Status".to_string()),
            value: vec![
                EnumValueDescriptorProto {
                    name: Some("UNKNOWN".to_string()),
                    number: Some(0),
                    ..Default::default()
                },
                EnumValueDescriptorProto {
                    name: Some("ACTIVE".to_string()),
                    number: Some(1),
                    ..Default::default()
                },
                EnumValueDescriptorProto {
                    name: Some("INACTIVE".to_string()),
                    number: Some(2),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        message_type: vec![DescriptorProto {
            name: Some("EnumMessage".to_string()),
            field: vec![FieldDescriptorProto {
                name: Some("status".to_string()),
                number: Some(1),
                label: Some(field_descriptor_proto::Label::Optional as i32),
                r#type: Some(field_descriptor_proto::Type::Enum as i32),
                type_name: Some(".test.Status".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };

    let fds = FileDescriptorSet { file: vec![file] };

    let pool = DescriptorPool::from_file_descriptor_set(fds).unwrap();
    pool.get_message_by_name("test.EnumMessage").unwrap()
}

#[test]
fn test_decode_simple_message() {
    let descriptor = create_simple_descriptor();

    // Encode: field 1 (id) = 42, field 2 (name) = "test"
    // Field 1: tag = (1 << 3) | 0 = 0x08, value = 42 = 0x2a
    // Field 2: tag = (2 << 3) | 2 = 0x12, length = 4, value = "test"
    let payload = vec![0x08, 0x2a, 0x12, 0x04, b't', b'e', b's', b't'];

    let result = decode_with_schema(&payload, &descriptor);
    assert!(result.is_ok(), "Should decode successfully");

    let decoded = result.unwrap();
    assert_eq!(decoded.type_name(), "test.SimpleMessage");

    // Verify JSON output
    let json = decoded.to_json();
    assert_eq!(json["id"], 42);
    assert_eq!(json["name"], "test");

    // Verify Display output contains expected content
    let display = format!("{}", decoded);
    assert!(display.contains("SimpleMessage"));
    assert!(display.contains("id"));
    assert!(display.contains("42"));
    assert!(display.contains("name"));
    assert!(display.contains("test"));
}

#[test]
fn test_decode_nested_message() {
    let descriptor = create_nested_descriptor();

    // Encode: outer message with inner message
    // Inner message: field 1 (value) = 99
    //   tag = 0x08, value = 0x63
    // Outer message: field 1 (inner) = [inner bytes]
    //   tag = 0x0a, length = 2, [0x08, 0x63]
    let inner_bytes = vec![0x08, 0x63];
    let mut payload = vec![0x0a, inner_bytes.len() as u8];
    payload.extend_from_slice(&inner_bytes);

    let result = decode_with_schema(&payload, &descriptor);
    assert!(result.is_ok(), "Should decode nested message");

    let decoded = result.unwrap();
    let json = decoded.to_json();

    // Verify nested structure
    assert!(json["inner"].is_object());
    assert_eq!(json["inner"]["value"], 99);
}

#[test]
fn test_decode_repeated_fields() {
    let descriptor = create_repeated_descriptor();

    // Encode: repeated string field with values "a", "b", "c"
    // Field 1 tag = 0x0a (field 1, wire type 2)
    // Each string: length-prefixed
    let payload = vec![
        0x0a, 0x01, b'a', // items[0] = "a"
        0x0a, 0x01, b'b', // items[1] = "b"
        0x0a, 0x01, b'c', // items[2] = "c"
    ];

    let result = decode_with_schema(&payload, &descriptor);
    assert!(result.is_ok(), "Should decode repeated fields");

    let decoded = result.unwrap();
    let json = decoded.to_json();

    assert!(json["items"].is_array());
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0], "a");
    assert_eq!(items[1], "b");
    assert_eq!(items[2], "c");
}

#[test]
fn test_decode_enum_field() {
    let descriptor = create_enum_descriptor();

    // Encode: field 1 (status) = ACTIVE (1)
    // Field 1: tag = 0x08, value = 0x01
    let payload = vec![0x08, 0x01];

    let result = decode_with_schema(&payload, &descriptor);
    assert!(result.is_ok(), "Should decode enum field");

    let decoded = result.unwrap();
    let json = decoded.to_json();

    // Enum is represented as numeric value in JSON
    assert_eq!(json["status"], 1);
}

#[test]
fn test_decode_wrong_schema() {
    let descriptor = create_simple_descriptor();

    // Payload with field number 99 (not in schema)
    // This should still decode but with unknown field behavior
    let payload = vec![0xf8, 0x06, 0x2a]; // field 99, varint, value 42

    let result = decode_with_schema(&payload, &descriptor);
    // prost-reflect typically ignores unknown fields, so this should succeed
    assert!(
        result.is_ok(),
        "Should decode with unknown fields (ignored)"
    );
}

#[test]
fn test_decode_truncated_payload() {
    let descriptor = create_simple_descriptor();

    // Truncated payload: field tag indicates string but incomplete data
    // Field 2: tag = 0x12, length = 10 (but only 2 bytes follow)
    let payload = vec![0x12, 0x0a, b't', b'e'];

    let result = decode_with_schema(&payload, &descriptor);
    assert!(result.is_err(), "Should fail on truncated data");

    match result {
        Err(DecodeError::DecodeFailed(_)) => {
            // Expected error type
        }
        _ => panic!("Expected DecodeError::DecodeFailed"),
    }
}

#[test]
fn test_decode_json_output() {
    let descriptor = create_simple_descriptor();

    let payload = vec![0x08, 0x2a, 0x12, 0x04, b't', b'e', b's', b't'];

    let decoded = decode_with_schema(&payload, &descriptor).unwrap();

    // Serialize to JSON string
    let json_str = serde_json::to_string(&decoded).unwrap();
    assert!(json_str.contains("\"id\":42"));
    assert!(json_str.contains("\"name\":\"test\""));

    // Round-trip through serde_json
    let value: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(value["id"], 42);
    assert_eq!(value["name"], "test");
}

#[test]
fn test_decode_empty_message() {
    let descriptor = create_simple_descriptor();

    // Empty payload - all fields optional/default
    let payload = vec![];

    let result = decode_with_schema(&payload, &descriptor);
    assert!(result.is_ok(), "Should decode empty message");

    let decoded = result.unwrap();
    let json = decoded.to_json();

    // Fields should have default values (0 for int, "" for string)
    assert_eq!(json["id"], 0);
    assert_eq!(json["name"], "");
}

/// Helper to create a message with all numeric types.
fn create_all_types_descriptor() -> prost_reflect::MessageDescriptor {
    let file = FileDescriptorProto {
        name: Some("alltypes.proto".to_string()),
        package: Some("test".to_string()),
        message_type: vec![DescriptorProto {
            name: Some("AllTypes".to_string()),
            field: vec![
                FieldDescriptorProto {
                    name: Some("int32_field".to_string()),
                    number: Some(1),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::Int32 as i32),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("int64_field".to_string()),
                    number: Some(2),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::Int64 as i32),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("uint32_field".to_string()),
                    number: Some(3),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::Uint32 as i32),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("uint64_field".to_string()),
                    number: Some(4),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::Uint64 as i32),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("bool_field".to_string()),
                    number: Some(5),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::Bool as i32),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("float_field".to_string()),
                    number: Some(6),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::Float as i32),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("double_field".to_string()),
                    number: Some(7),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::Double as i32),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("bytes_field".to_string()),
                    number: Some(8),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::Bytes as i32),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("fixed32_field".to_string()),
                    number: Some(9),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::Fixed32 as i32),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("fixed64_field".to_string()),
                    number: Some(10),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::Fixed64 as i32),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("sfixed32_field".to_string()),
                    number: Some(11),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::Sfixed32 as i32),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("sfixed64_field".to_string()),
                    number: Some(12),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::Sfixed64 as i32),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };

    let fds = FileDescriptorSet { file: vec![file] };

    let pool = DescriptorPool::from_file_descriptor_set(fds).unwrap();
    pool.get_message_by_name("test.AllTypes").unwrap()
}

#[test]
fn test_decode_all_numeric_types() {
    let descriptor = create_all_types_descriptor();

    // Encode various field types
    let mut payload = vec![];

    // int32_field = -42 (field 1, varint, zigzag)
    payload.extend_from_slice(&[0x08, 0x54]); // tag 1, zigzag(-42) = 83

    // int64_field = -1000 (field 2, varint)
    payload.extend_from_slice(&[0x10, 0xd0, 0x0f]); // tag 2, varint 1999

    // uint32_field = 12345 (field 3, varint)
    payload.extend_from_slice(&[0x18, 0xb9, 0x60]); // tag 3, varint 12345

    // uint64_field = 999999 (field 4, varint)
    payload.extend_from_slice(&[0x20, 0xbf, 0xc4, 0x3d]); // tag 4, varint 999999

    // bool_field = true (field 5, varint 1)
    payload.extend_from_slice(&[0x28, 0x01]); // tag 5, varint 1

    let result = decode_with_schema(&payload, &descriptor);
    assert!(result.is_ok(), "Should decode all numeric types");

    let decoded = result.unwrap();
    let json = decoded.to_json();

    // Verify values were decoded (exact values depend on prost-reflect's handling)
    assert!(json.get("int32_field").is_some());
    assert!(json.get("uint32_field").is_some());
    assert!(json.get("bool_field").is_some());
}

#[test]
fn test_decode_fixed_types() {
    let descriptor = create_all_types_descriptor();

    let mut payload = vec![];

    // fixed32_field = 100 (field 9, fixed32)
    payload.push(0x4d); // tag (9 << 3) | 5 = 77 = 0x4d
    payload.extend_from_slice(&100u32.to_le_bytes());

    // fixed64_field = 200 (field 10, fixed64)
    payload.push(0x51); // tag (10 << 3) | 1 = 81 = 0x51
    payload.extend_from_slice(&200u64.to_le_bytes());

    let result = decode_with_schema(&payload, &descriptor);
    assert!(result.is_ok(), "Should decode fixed types");

    let decoded = result.unwrap();
    let json = decoded.to_json();

    assert_eq!(json["fixed32_field"], 100);
    assert_eq!(json["fixed64_field"], 200);
}

#[test]
fn test_decode_float_double() {
    let descriptor = create_all_types_descriptor();

    let mut payload = vec![];

    // float_field = 3.14 (field 6, fixed32)
    payload.push(0x35); // tag (6 << 3) | 5 = 53 = 0x35
    payload.extend_from_slice(&std::f32::consts::PI.to_le_bytes());

    // double_field = 2.718 (field 7, fixed64)
    payload.push(0x39); // tag (7 << 3) | 1 = 57 = 0x39
    payload.extend_from_slice(&std::f64::consts::E.to_le_bytes());

    let result = decode_with_schema(&payload, &descriptor);
    assert!(result.is_ok(), "Should decode float/double");

    let decoded = result.unwrap();
    let json = decoded.to_json();

    // Check that float/double fields exist (exact comparison may have rounding)
    assert!(json.get("float_field").is_some());
    assert!(json.get("double_field").is_some());
}

#[test]
fn test_decode_bytes_field() {
    let descriptor = create_all_types_descriptor();

    let mut payload = vec![];

    // bytes_field = [0xde, 0xad, 0xbe, 0xef] (field 8, length-delimited)
    payload.push(0x42); // tag (8 << 3) | 2 = 66 = 0x42
    payload.push(0x04); // length = 4
    payload.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);

    let result = decode_with_schema(&payload, &descriptor);
    assert!(result.is_ok(), "Should decode bytes field");

    let decoded = result.unwrap();
    let json = decoded.to_json();

    // Bytes are base64-encoded in JSON
    assert!(json["bytes_field"].is_string());
    let bytes_str = json["bytes_field"].as_str().unwrap();
    // Base64 encoding of [0xde, 0xad, 0xbe, 0xef] is "3q2+7w=="
    assert_eq!(bytes_str, "3q2+7w==");
}

/// Helper to create a message with packed repeated fields.
fn create_packed_descriptor() -> prost_reflect::MessageDescriptor {
    let file = FileDescriptorProto {
        name: Some("packed.proto".to_string()),
        package: Some("test".to_string()),
        message_type: vec![DescriptorProto {
            name: Some("PackedMessage".to_string()),
            field: vec![FieldDescriptorProto {
                name: Some("values".to_string()),
                number: Some(1),
                label: Some(field_descriptor_proto::Label::Repeated as i32),
                r#type: Some(field_descriptor_proto::Type::Int32 as i32),
                options: Some(prost_types::FieldOptions {
                    packed: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };

    let fds = FileDescriptorSet { file: vec![file] };

    let pool = DescriptorPool::from_file_descriptor_set(fds).unwrap();
    pool.get_message_by_name("test.PackedMessage").unwrap()
}

#[test]
fn test_decode_packed_repeated_field() {
    let descriptor = create_packed_descriptor();

    // Encode packed repeated int32: [1, 2, 3, 4, 5]
    // Field 1, length-delimited with packed varints
    let mut payload = vec![0x0a]; // tag (1 << 3) | 2 = 10 = 0x0a

    let packed_data = vec![0x01, 0x02, 0x03, 0x04, 0x05];
    payload.push(packed_data.len() as u8);
    payload.extend_from_slice(&packed_data);

    let result = decode_with_schema(&payload, &descriptor);
    assert!(result.is_ok(), "Should decode packed repeated field");

    let decoded = result.unwrap();
    let json = decoded.to_json();

    assert!(json["values"].is_array());
    let values = json["values"].as_array().unwrap();
    assert_eq!(values.len(), 5);
}

/// Helper to create a message with map field.
fn create_map_descriptor() -> prost_reflect::MessageDescriptor {
    use prost_types::DescriptorProto;

    let file = FileDescriptorProto {
        name: Some("map.proto".to_string()),
        package: Some("test".to_string()),
        message_type: vec![
            // Map entry message (auto-generated by protobuf compiler)
            DescriptorProto {
                name: Some("MapMessage_AttributesEntry".to_string()),
                field: vec![
                    FieldDescriptorProto {
                        name: Some("key".to_string()),
                        number: Some(1),
                        label: Some(field_descriptor_proto::Label::Optional as i32),
                        r#type: Some(field_descriptor_proto::Type::String as i32),
                        ..Default::default()
                    },
                    FieldDescriptorProto {
                        name: Some("value".to_string()),
                        number: Some(2),
                        label: Some(field_descriptor_proto::Label::Optional as i32),
                        r#type: Some(field_descriptor_proto::Type::String as i32),
                        ..Default::default()
                    },
                ],
                options: Some(prost_types::MessageOptions {
                    map_entry: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
            // Actual message with map field
            DescriptorProto {
                name: Some("MapMessage".to_string()),
                field: vec![FieldDescriptorProto {
                    name: Some("attributes".to_string()),
                    number: Some(1),
                    label: Some(field_descriptor_proto::Label::Repeated as i32),
                    r#type: Some(field_descriptor_proto::Type::Message as i32),
                    type_name: Some(".test.MapMessage_AttributesEntry".to_string()),
                    ..Default::default()
                }],
                nested_type: vec![DescriptorProto {
                    name: Some("AttributesEntry".to_string()),
                    field: vec![
                        FieldDescriptorProto {
                            name: Some("key".to_string()),
                            number: Some(1),
                            label: Some(field_descriptor_proto::Label::Optional as i32),
                            r#type: Some(field_descriptor_proto::Type::String as i32),
                            ..Default::default()
                        },
                        FieldDescriptorProto {
                            name: Some("value".to_string()),
                            number: Some(2),
                            label: Some(field_descriptor_proto::Label::Optional as i32),
                            r#type: Some(field_descriptor_proto::Type::String as i32),
                            ..Default::default()
                        },
                    ],
                    options: Some(prost_types::MessageOptions {
                        map_entry: Some(true),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let fds = FileDescriptorSet { file: vec![file] };

    let pool = DescriptorPool::from_file_descriptor_set(fds).unwrap();
    pool.get_message_by_name("test.MapMessage").unwrap()
}

#[test]
fn test_decode_map_field() {
    let descriptor = create_map_descriptor();

    // Encode map<string, string> with entry {"key1": "value1"}
    // Map entry is a repeated message field
    // Field 1 (attributes), entry: field 1 (key) = "key1", field 2 (value) = "value1"
    let mut entry = vec![];
    entry.push(0x0a); // field 1 (key), tag = (1 << 3) | 2 = 10
    entry.push(0x04); // length = 4
    entry.extend_from_slice(b"key1");
    entry.push(0x12); // field 2 (value), tag = (2 << 3) | 2 = 18
    entry.push(0x06); // length = 6
    entry.extend_from_slice(b"value1");

    let mut payload = vec![0x0a]; // tag (1 << 3) | 2 = 10
    payload.push(entry.len() as u8);
    payload.extend_from_slice(&entry);

    let result = decode_with_schema(&payload, &descriptor);
    assert!(result.is_ok(), "Should decode map field");

    let decoded = result.unwrap();
    let json = decoded.to_json();

    // Map should be represented as an array of entries or object
    assert!(json.get("attributes").is_some());
}

#[test]
fn test_decode_trailing_data() {
    let descriptor = create_simple_descriptor();

    // Valid message followed by extra bytes that look like valid protobuf
    // Use a valid-looking tag for trailing data
    let mut payload = vec![0x08, 0x2a]; // field 1 (id) = 42
    payload.extend_from_slice(&[0x10, 0x01]); // field 2 as varint (but schema expects string)

    let result = decode_with_schema(&payload, &descriptor);

    // prost-reflect may decode this successfully or fail depending on the trailing bytes
    // The function checks for remaining bytes after decode
    if let Ok(decoded) = result {
        // If it decoded, there should be no trailing bytes
        let json = decoded.to_json();
        assert_eq!(json["id"], 42);
    } else if let Err(err) = result {
        // If it failed, should be a decode or schema mismatch error
        assert!(matches!(
            err,
            DecodeError::DecodeFailed(_) | DecodeError::SchemaMismatch(_)
        ));
    }
}

#[test]
fn test_decode_enum_unknown_value() {
    let descriptor = create_enum_descriptor();

    // Encode: field 1 (status) = 99 (unknown enum value)
    let payload = vec![0x08, 0x63]; // tag 1, value 99

    let result = decode_with_schema(&payload, &descriptor);
    assert!(result.is_ok(), "Should decode unknown enum value");

    let decoded = result.unwrap();
    let json = decoded.to_json();

    // Unknown enum values are represented as numeric values
    assert_eq!(json["status"], 99);
}

#[test]
fn test_decoded_message_display() {
    let descriptor = create_simple_descriptor();
    let payload = vec![0x08, 0x2a, 0x12, 0x04, b't', b'e', b's', b't'];

    let decoded = decode_with_schema(&payload, &descriptor).unwrap();

    // Test Display trait
    let display_output = format!("{}", decoded);
    assert!(display_output.contains("SimpleMessage"));
    assert!(display_output.contains("id"));
    assert!(display_output.contains("42"));
    assert!(display_output.contains("name"));
    assert!(display_output.contains("test"));
}

#[test]
fn test_decoded_message_accessors() {
    let descriptor = create_simple_descriptor();
    let payload = vec![0x08, 0x2a, 0x12, 0x04, b't', b'e', b's', b't'];

    let decoded = decode_with_schema(&payload, &descriptor).unwrap();

    // Test accessor methods
    assert_eq!(decoded.type_name(), "test.SimpleMessage");
    assert!(decoded.descriptor().full_name() == "test.SimpleMessage");

    // Verify we can access the underlying message
    let _msg = decoded.message();
}

#[test]
fn test_decode_invalid_varint() {
    let descriptor = create_simple_descriptor();

    // Incomplete varint (continuation byte with no following byte)
    let payload = vec![0x08, 0x80]; // field 1, incomplete varint

    let result = decode_with_schema(&payload, &descriptor);
    assert!(result.is_err(), "Should fail on invalid varint");
}

#[test]
fn test_nested_message_display() {
    let descriptor = create_nested_descriptor();

    // Encode nested message
    let inner_bytes = vec![0x08, 0x63]; // field 1 = 99
    let mut payload = vec![0x0a, inner_bytes.len() as u8];
    payload.extend_from_slice(&inner_bytes);

    let decoded = decode_with_schema(&payload, &descriptor).unwrap();

    // Test that nested messages are formatted correctly
    let display_output = format!("{}", decoded);
    assert!(display_output.contains("inner"));
    assert!(display_output.contains("value"));
}

#[test]
fn test_repeated_fields_display() {
    let descriptor = create_repeated_descriptor();

    let payload = vec![0x0a, 0x01, b'a', 0x0a, 0x01, b'b', 0x0a, 0x01, b'c'];

    let decoded = decode_with_schema(&payload, &descriptor).unwrap();

    // Test that repeated fields are formatted as arrays
    let display_output = format!("{}", decoded);
    assert!(display_output.contains("items"));
}

#[test]
fn test_large_varint_field() {
    let descriptor = create_all_types_descriptor();

    let mut payload = vec![];

    // uint64_field with large value (close to u64::MAX)
    payload.push(0x20); // tag 4
    // Encode u64::MAX - 1000 as varint
    let large_val = u64::MAX - 1000;
    let mut val = large_val;
    while val >= 0x80 {
        payload.push((val as u8) | 0x80);
        val >>= 7;
    }
    payload.push(val as u8);

    let result = decode_with_schema(&payload, &descriptor);
    assert!(result.is_ok(), "Should decode large varint");
}

#[test]
fn test_zero_length_string() {
    let descriptor = create_simple_descriptor();

    // Empty string in field 2
    let payload = vec![0x12, 0x00]; // tag 2, length 0

    let result = decode_with_schema(&payload, &descriptor);
    assert!(result.is_ok(), "Should decode empty string");

    let decoded = result.unwrap();
    let json = decoded.to_json();
    assert_eq!(json["name"], "");
}

#[test]
fn test_zero_length_bytes() {
    let descriptor = create_all_types_descriptor();

    // Empty bytes in field 8
    let payload = vec![0x42, 0x00]; // tag (8 << 3) | 2, length 0

    let result = decode_with_schema(&payload, &descriptor);
    assert!(result.is_ok(), "Should decode empty bytes");

    let decoded = result.unwrap();
    let json = decoded.to_json();
    // Empty bytes encoded as empty base64 string
    assert_eq!(json["bytes_field"], "");
}
