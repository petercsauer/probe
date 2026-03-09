//! Integration tests for schema-backed protobuf decoding.

use prb_decode::schema_backed::{decode_with_schema, DecodeError};
use prost_reflect::DescriptorPool;
use prost_types::{
    field_descriptor_proto, DescriptorProto, EnumDescriptorProto, EnumValueDescriptorProto,
    FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet,
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

    let fds = FileDescriptorSet {
        file: vec![file],
    };

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

    let fds = FileDescriptorSet {
        file: vec![file],
    };

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

    let fds = FileDescriptorSet {
        file: vec![file],
    };

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

    let fds = FileDescriptorSet {
        file: vec![file],
    };

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
    assert!(result.is_ok(), "Should decode with unknown fields (ignored)");
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
