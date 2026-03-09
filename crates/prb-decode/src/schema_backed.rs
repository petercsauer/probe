//! Schema-backed protobuf decoder using prost-reflect.
//!
//! Provides decoding of protobuf messages using loaded schemas (MessageDescriptor)
//! for accurate field names, types, and nested message structures.

use base64::Engine;
use bytes::Buf;
use prost_reflect::{DynamicMessage, MapKey, MessageDescriptor, ReflectMessage, Value};
use std::fmt;
use thiserror::Error;

/// Error type for schema-backed decoding.
#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("Failed to decode message: {0}")]
    DecodeFailed(String),

    #[error("Invalid protobuf data: {0}")]
    InvalidData(String),

    #[error("Schema mismatch: {0}")]
    SchemaMismatch(String),
}

/// A decoded protobuf message with schema information.
///
/// Wraps a prost-reflect DynamicMessage and provides formatted display
/// and JSON serialization.
#[derive(Debug, Clone)]
pub struct DecodedMessage {
    message: DynamicMessage,
    descriptor: MessageDescriptor,
}

impl DecodedMessage {
    /// Create a new decoded message.
    pub fn new(message: DynamicMessage, descriptor: MessageDescriptor) -> Self {
        Self {
            message,
            descriptor,
        }
    }

    /// Get the message type name.
    pub fn type_name(&self) -> &str {
        self.descriptor.full_name()
    }

    /// Get the underlying DynamicMessage.
    pub fn message(&self) -> &DynamicMessage {
        &self.message
    }

    /// Get the message descriptor.
    pub fn descriptor(&self) -> &MessageDescriptor {
        &self.descriptor
    }

    /// Convert to JSON value.
    pub fn to_json(&self) -> serde_json::Value {
        message_to_json(&self.message)
    }
}

impl fmt::Display for DecodedMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} {{", self.descriptor.full_name())?;
        format_message_fields(f, &self.message, 1)?;
        write!(f, "}}")
    }
}

impl serde::Serialize for DecodedMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_json().serialize(serializer)
    }
}

/// Decode a protobuf message using a schema.
///
/// # Arguments
/// * `payload` - Raw protobuf-encoded bytes
/// * `descriptor` - Message schema descriptor
///
/// # Returns
/// A `DecodedMessage` on success, or a `DecodeError` on failure.
///
/// # Example
/// ```no_run
/// use prb_decode::schema_backed::decode_with_schema;
/// # use prost_reflect::MessageDescriptor;
/// # fn example(payload: &[u8], descriptor: MessageDescriptor) {
/// let decoded = decode_with_schema(payload, &descriptor).unwrap();
/// println!("{}", decoded);
/// # }
/// ```
pub fn decode_with_schema(
    payload: &[u8],
    descriptor: &MessageDescriptor,
) -> Result<DecodedMessage, DecodeError> {
    // Create a Buf from the payload
    let mut buf = payload;

    // Decode using prost-reflect
    let message = DynamicMessage::decode(descriptor.clone(), &mut buf).map_err(|e| {
        DecodeError::DecodeFailed(format!("Failed to decode {}: {}", descriptor.full_name(), e))
    })?;

    // Check if there's trailing data (potential schema mismatch)
    if buf.has_remaining() {
        let remaining = buf.remaining();
        return Err(DecodeError::SchemaMismatch(format!(
            "Decoded successfully but {} bytes remain (possible schema mismatch)",
            remaining
        )));
    }

    Ok(DecodedMessage::new(message, descriptor.clone()))
}

/// Format message fields with indentation.
fn format_message_fields(
    f: &mut fmt::Formatter<'_>,
    message: &DynamicMessage,
    indent: usize,
) -> fmt::Result {
    let descriptor = message.descriptor();
    let indent_str = "  ".repeat(indent);

    for field in descriptor.fields() {
        let value = message.get_field(&field);

        write!(f, "{}{}: ", indent_str, field.name())?;

        format_value(f, &value, indent)?;
        writeln!(f)?;
    }

    Ok(())
}

/// Format a map key.
fn format_map_key(f: &mut fmt::Formatter<'_>, key: &MapKey) -> fmt::Result {
    match key {
        MapKey::Bool(b) => write!(f, "{}", b),
        MapKey::I32(i) => write!(f, "{}", i),
        MapKey::I64(i) => write!(f, "{}", i),
        MapKey::U32(u) => write!(f, "{}", u),
        MapKey::U64(u) => write!(f, "{}", u),
        MapKey::String(s) => write!(f, "\"{}\"", s),
    }
}

/// Format a protobuf value.
fn format_value(f: &mut fmt::Formatter<'_>, value: &Value, indent: usize) -> fmt::Result {
    match value {
        Value::Bool(b) => write!(f, "{}", b),
        Value::I32(i) => write!(f, "{}", i),
        Value::I64(i) => write!(f, "{}", i),
        Value::U32(u) => write!(f, "{}", u),
        Value::U64(u) => write!(f, "{}", u),
        Value::F32(fl) => write!(f, "{}", fl),
        Value::F64(fl) => write!(f, "{}", fl),
        Value::String(s) => write!(f, "\"{}\"", s),
        Value::Bytes(b) => {
            write!(f, "0x")?;
            for byte in b.iter() {
                write!(f, "{:02x}", byte)?;
            }
            Ok(())
        }
        Value::EnumNumber(num) => {
            // Try to get the enum descriptor and find the name
            write!(f, "{}", num)
        }
        Value::Message(msg) => {
            writeln!(f, "{{")?;
            format_message_fields(f, msg, indent + 1)?;
            write!(f, "{}}}", "  ".repeat(indent))
        }
        Value::List(items) => {
            write!(f, "[")?;
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                format_value(f, item, indent)?;
            }
            write!(f, "]")
        }
        Value::Map(entries) => {
            writeln!(f, "{{")?;
            let indent_str = "  ".repeat(indent + 1);
            for (key, val) in entries.iter() {
                write!(f, "{}", indent_str)?;
                format_map_key(f, key)?;
                write!(f, ": ")?;
                format_value(f, val, indent + 1)?;
                writeln!(f)?;
            }
            write!(f, "{}}}", "  ".repeat(indent))
        }
    }
}

/// Convert a DynamicMessage to a JSON value.
fn message_to_json(message: &DynamicMessage) -> serde_json::Value {
    let mut map = serde_json::Map::new();

    for field in message.descriptor().fields() {
        let value = message.get_field(&field);
        let json_value = value_to_json(&value);
        map.insert(field.name().to_string(), json_value);
    }

    serde_json::Value::Object(map)
}

/// Convert a MapKey to a string for JSON object keys.
fn map_key_to_string(key: &MapKey) -> String {
    match key {
        MapKey::Bool(b) => b.to_string(),
        MapKey::I32(i) => i.to_string(),
        MapKey::I64(i) => i.to_string(),
        MapKey::U32(u) => u.to_string(),
        MapKey::U64(u) => u.to_string(),
        MapKey::String(s) => s.clone(),
    }
}

/// Convert a prost-reflect Value to a JSON value.
fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::I32(i) => serde_json::Value::Number((*i).into()),
        Value::I64(i) => serde_json::Value::Number((*i).into()),
        Value::U32(u) => serde_json::Value::Number((*u).into()),
        Value::U64(u) => serde_json::Value::Number((*u).into()),
        Value::F32(f) => serde_json::Number::from_f64(*f as f64)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::F64(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Bytes(b) => {
            // Encode bytes as base64
            serde_json::Value::String(base64::engine::general_purpose::STANDARD.encode(b))
        }
        Value::EnumNumber(n) => serde_json::Value::Number((*n).into()),
        Value::Message(msg) => message_to_json(msg),
        Value::List(items) => {
            let json_items: Vec<_> = items.iter().map(value_to_json).collect();
            serde_json::Value::Array(json_items)
        }
        Value::Map(entries) => {
            let mut map = serde_json::Map::new();
            for (key, val) in entries.iter() {
                // Convert key to string for JSON object key
                let key_str = map_key_to_string(key);
                map.insert(key_str, value_to_json(val));
            }
            serde_json::Value::Object(map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost_reflect::DescriptorPool;
    use prost_types::{
        field_descriptor_proto, DescriptorProto, FieldDescriptorProto, FileDescriptorProto,
        FileDescriptorSet,
    };

    fn create_simple_descriptor() -> MessageDescriptor {
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

    #[test]
    fn test_decode_simple_message() {
        let descriptor = create_simple_descriptor();

        // Manually encode: field 1 (id) = 42, field 2 (name) = "test"
        // Field 1: tag = (1 << 3) | 0 = 0x08, value = 42 = 0x2a
        // Field 2: tag = (2 << 3) | 2 = 0x12, length = 4, value = "test"
        let payload = vec![0x08, 0x2a, 0x12, 0x04, b't', b'e', b's', b't'];

        let result = decode_with_schema(&payload, &descriptor);
        assert!(result.is_ok(), "Should decode successfully");

        let decoded = result.unwrap();
        assert_eq!(decoded.type_name(), "test.SimpleMessage");

        // Check JSON output
        let json = decoded.to_json();
        assert_eq!(json["id"], 42);
        assert_eq!(json["name"], "test");
    }

    #[test]
    fn test_decode_truncated_payload() {
        let descriptor = create_simple_descriptor();

        // Truncated payload: field 2 tag + incomplete length
        let payload = vec![0x12, 0x04, b't', b'e'];

        let result = decode_with_schema(&payload, &descriptor);
        assert!(result.is_err(), "Should fail on truncated data");
        assert!(matches!(result.unwrap_err(), DecodeError::DecodeFailed(_)));
    }
}
