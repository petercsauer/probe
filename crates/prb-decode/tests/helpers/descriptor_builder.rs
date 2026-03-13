//! Test helper for building protobuf message descriptors.
//!
//! Provides a fluent API for creating `MessageDescriptor` instances without verbose boilerplate.

use prost_reflect::{DescriptorPool, MessageDescriptor};
use prost_types::{
    DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet,
    field_descriptor_proto,
};

/// Builder for creating protobuf message descriptors in tests.
#[derive(Debug, Clone)]
pub struct DescriptorBuilder {
    name: String,
    package: String,
    fields: Vec<FieldDef>,
}

#[derive(Debug, Clone)]
struct FieldDef {
    name: String,
    number: i32,
    field_type: FieldType,
    label: field_descriptor_proto::Label,
}

/// Protobuf field types supported by the builder.
#[derive(Debug, Clone)]
pub enum FieldType {
    Int32,
    Int64,
    Uint32,
    Uint64,
    Sint32,
    Sint64,
    Bool,
    String,
    Bytes,
    Fixed32,
    Fixed64,
    Sfixed32,
    Sfixed64,
    Float,
    Double,
}

impl DescriptorBuilder {
    /// Create a new descriptor builder for a message type.
    #[must_use]
    pub fn message(name: &str) -> Self {
        Self {
            name: name.to_string(),
            package: "test".to_string(),
            fields: Vec::new(),
        }
    }

    /// Set the package name (default: "test").
    #[must_use]
    pub fn package(mut self, package: &str) -> Self {
        self.package = package.to_string();
        self
    }

    /// Add an optional field.
    #[must_use]
    pub fn field(mut self, name: &str, number: i32, field_type: FieldType) -> Self {
        self.fields.push(FieldDef {
            name: name.to_string(),
            number,
            field_type,
            label: field_descriptor_proto::Label::Optional,
        });
        self
    }

    /// Add a repeated field.
    #[must_use]
    pub fn repeated_field(mut self, name: &str, number: i32, field_type: FieldType) -> Self {
        self.fields.push(FieldDef {
            name: name.to_string(),
            number,
            field_type,
            label: field_descriptor_proto::Label::Repeated,
        });
        self
    }

    /// Build the `MessageDescriptor`.
    ///
    /// # Panics
    /// Panics if descriptor creation fails (test helper).
    #[must_use]
    pub fn build(self) -> MessageDescriptor {
        let proto_fields: Vec<FieldDescriptorProto> = self
            .fields
            .into_iter()
            .map(|f| {
                let proto_type = match f.field_type {
                    FieldType::Int32 => field_descriptor_proto::Type::Int32,
                    FieldType::Int64 => field_descriptor_proto::Type::Int64,
                    FieldType::Uint32 => field_descriptor_proto::Type::Uint32,
                    FieldType::Uint64 => field_descriptor_proto::Type::Uint64,
                    FieldType::Sint32 => field_descriptor_proto::Type::Sint32,
                    FieldType::Sint64 => field_descriptor_proto::Type::Sint64,
                    FieldType::Bool => field_descriptor_proto::Type::Bool,
                    FieldType::String => field_descriptor_proto::Type::String,
                    FieldType::Bytes => field_descriptor_proto::Type::Bytes,
                    FieldType::Fixed32 => field_descriptor_proto::Type::Fixed32,
                    FieldType::Fixed64 => field_descriptor_proto::Type::Fixed64,
                    FieldType::Sfixed32 => field_descriptor_proto::Type::Sfixed32,
                    FieldType::Sfixed64 => field_descriptor_proto::Type::Sfixed64,
                    FieldType::Float => field_descriptor_proto::Type::Float,
                    FieldType::Double => field_descriptor_proto::Type::Double,
                };

                FieldDescriptorProto {
                    name: Some(f.name),
                    number: Some(f.number),
                    label: Some(f.label as i32),
                    r#type: Some(proto_type as i32),
                    ..Default::default()
                }
            })
            .collect();

        let file = FileDescriptorProto {
            name: Some(format!("{}.proto", self.name.to_lowercase())),
            package: Some(self.package.clone()),
            message_type: vec![DescriptorProto {
                name: Some(self.name.clone()),
                field: proto_fields,
                ..Default::default()
            }],
            ..Default::default()
        };

        let fds = FileDescriptorSet { file: vec![file] };
        let pool = DescriptorPool::from_file_descriptor_set(fds)
            .expect("Failed to create descriptor pool");

        pool.get_message_by_name(&format!("{}.{}", self.package, self.name))
            .expect("Failed to get message descriptor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_simple_message() {
        let desc = DescriptorBuilder::message("TestMsg")
            .field("id", 1, FieldType::Int32)
            .field("name", 2, FieldType::String)
            .build();

        assert_eq!(desc.full_name(), "test.TestMsg");
        assert_eq!(desc.fields().len(), 2);
    }

    #[test]
    fn test_builder_all_types() {
        let desc = DescriptorBuilder::message("AllTypes")
            .field("int32_field", 1, FieldType::Int32)
            .field("int64_field", 2, FieldType::Int64)
            .field("uint32_field", 3, FieldType::Uint32)
            .field("uint64_field", 4, FieldType::Uint64)
            .field("sint32_field", 5, FieldType::Sint32)
            .field("sint64_field", 6, FieldType::Sint64)
            .field("bool_field", 7, FieldType::Bool)
            .field("string_field", 8, FieldType::String)
            .field("bytes_field", 9, FieldType::Bytes)
            .field("fixed32_field", 10, FieldType::Fixed32)
            .field("fixed64_field", 11, FieldType::Fixed64)
            .field("sfixed32_field", 12, FieldType::Sfixed32)
            .field("sfixed64_field", 13, FieldType::Sfixed64)
            .field("float_field", 14, FieldType::Float)
            .field("double_field", 15, FieldType::Double)
            .build();

        assert_eq!(desc.fields().len(), 15);
    }

    #[test]
    fn test_builder_repeated_field() {
        let desc = DescriptorBuilder::message("RepeatedMsg")
            .repeated_field("items", 1, FieldType::String)
            .build();

        assert_eq!(desc.fields().len(), 1);
        let field = desc.fields().next().unwrap();
        assert!(field.is_list());
    }

    #[test]
    fn test_builder_custom_package() {
        let desc = DescriptorBuilder::message("MyMsg")
            .package("custom.pkg")
            .field("value", 1, FieldType::Int32)
            .build();

        assert_eq!(desc.full_name(), "custom.pkg.MyMsg");
    }
}
