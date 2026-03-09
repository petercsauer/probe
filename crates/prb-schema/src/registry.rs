//! Schema registry implementation.

use crate::error::{Result, SchemaError};
use prost::Message as ProstMessage;
use prost_reflect::{DescriptorPool, MessageDescriptor};
use prost_types::FileDescriptorSet;
use std::path::Path;

/// Protobuf schema registry.
///
/// Manages loading and resolving protobuf message schemas from both
/// pre-compiled descriptor sets and runtime-compiled .proto files.
pub struct SchemaRegistry {
    /// Descriptor pool containing all loaded schemas.
    pool: DescriptorPool,
    /// Raw FileDescriptorSet bytes for MCAP embedding.
    loaded_sets: Vec<Vec<u8>>,
    /// Accumulated file descriptors for merging
    all_files: Vec<prost_types::FileDescriptorProto>,
}

impl SchemaRegistry {
    /// Create a new empty schema registry.
    pub fn new() -> Self {
        Self {
            pool: DescriptorPool::global(),
            loaded_sets: Vec::new(),
            all_files: Vec::new(),
        }
    }

    /// Load a descriptor set from raw bytes.
    ///
    /// The bytes should be a serialized `FileDescriptorSet` message.
    /// Stores the raw bytes for later MCAP embedding.
    pub fn load_descriptor_set(&mut self, bytes: &[u8]) -> Result<()> {
        // Decode the FileDescriptorSet
        let fds = FileDescriptorSet::decode(bytes)
            .map_err(|e: prost::DecodeError| SchemaError::InvalidDescriptor(e.to_string()))?;

        // Add files to accumulator
        self.all_files.extend(fds.file);

        // Rebuild pool with all files
        let merged_fds = FileDescriptorSet {
            file: self.all_files.clone(),
        };
        self.pool = DescriptorPool::from_file_descriptor_set(merged_fds)
            .map_err(|e| SchemaError::LoadDescriptorSet(e.to_string()))?;

        // Store raw bytes for MCAP embedding
        self.loaded_sets.push(bytes.to_vec());

        Ok(())
    }

    /// Load a descriptor set from a file.
    ///
    /// Reads and parses a binary FileDescriptorSet file (typically .desc extension).
    pub fn load_descriptor_set_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        tracing::debug!("Loading descriptor set from {}", path.display());

        let bytes = std::fs::read(path)?;
        self.load_descriptor_set(&bytes)
            .map_err(|e| match e {
                SchemaError::InvalidDescriptor(msg) => {
                    SchemaError::LoadDescriptorSet(format!("{}: {}", path.display(), msg))
                }
                other => other,
            })
    }

    /// Load and compile .proto files at runtime.
    ///
    /// # Arguments
    /// * `files` - Paths to .proto files to compile
    /// * `includes` - Include paths for resolving imports
    pub fn load_proto_files(
        &mut self,
        files: &[impl AsRef<Path>],
        includes: &[impl AsRef<Path>],
    ) -> Result<()> {
        if files.is_empty() {
            return Ok(());
        }

        let file_paths: Vec<_> = files.iter().map(|p| p.as_ref()).collect();
        let include_paths: Vec<_> = includes.iter().map(|p| p.as_ref()).collect();

        tracing::debug!(
            "Compiling proto files: {:?} with includes: {:?}",
            file_paths,
            include_paths
        );

        // Compile using protox
        let fds = protox::compile(&file_paths, &include_paths)
            .map_err(|e| SchemaError::CompileProto {
                file: file_paths.first().unwrap().to_path_buf(),
                message: e.to_string(),
            })?;

        // Encode to bytes
        let mut buf = Vec::new();
        fds.encode(&mut buf)
            .map_err(|e: prost::EncodeError| SchemaError::InvalidDescriptor(e.to_string()))?;

        // Load the compiled descriptor set
        self.load_descriptor_set(&buf)
    }

    /// Get a message descriptor by fully qualified name.
    ///
    /// # Arguments
    /// * `fqn` - Fully qualified message name (e.g., "foo.bar.MyMessage")
    ///
    /// # Returns
    /// `Some(descriptor)` if found, `None` otherwise.
    pub fn get_message(&self, fqn: &str) -> Option<MessageDescriptor> {
        self.pool.get_message_by_name(fqn)
    }

    /// List all known message type names.
    ///
    /// Returns fully qualified names of all messages in the registry.
    pub fn list_messages(&self) -> Vec<String> {
        self.pool
            .all_messages()
            .map(|desc| desc.full_name().to_string())
            .collect()
    }

    /// List all known service names.
    ///
    /// Returns fully qualified names of all services in the registry.
    pub fn list_services(&self) -> Vec<String> {
        self.pool
            .services()
            .map(|desc| desc.full_name().to_string())
            .collect()
    }

    /// Get raw descriptor set bytes for MCAP embedding.
    ///
    /// Returns a slice of all loaded descriptor sets as raw bytes.
    pub fn descriptor_sets(&self) -> &[Vec<u8>] {
        &self.loaded_sets
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Implement the SchemaResolver trait from prb-core
impl prb_core::SchemaResolver for SchemaRegistry {
    fn resolve(&self, schema_name: &str) -> std::result::Result<Option<prb_core::ResolvedSchema>, prb_core::CoreError> {
        match self.get_message(schema_name) {
            Some(_desc) => {
                // For protobuf, we return the entire FileDescriptorSet that contains this message
                // In a more optimized implementation, we could return just the relevant subset
                // For now, we return the first descriptor set (simplification)
                if let Some(bytes) = self.loaded_sets.first() {
                    Ok(Some(prb_core::ResolvedSchema {
                        name: schema_name.to_string(),
                        content: bytes::Bytes::copy_from_slice(bytes),
                        format: "protobuf".to_string(),
                    }))
                } else {
                    // Message exists but no raw bytes stored (e.g., from global pool)
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    fn list_schemas(&self) -> Vec<String> {
        self.list_messages()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message as ProstMessage;
    use prost_types::{FileDescriptorProto, FileDescriptorSet};

    fn create_test_descriptor_set() -> Vec<u8> {
        // Create a minimal FileDescriptorSet with one message type
        let file = FileDescriptorProto {
            name: Some("test.proto".to_string()),
            package: Some("test".to_string()),
            message_type: vec![prost_types::DescriptorProto {
                name: Some("TestMessage".to_string()),
                field: vec![prost_types::FieldDescriptorProto {
                    name: Some("id".to_string()),
                    number: Some(1),
                    label: Some(prost_types::field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(prost_types::field_descriptor_proto::Type::Int32 as i32),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };

        let fds = FileDescriptorSet {
            file: vec![file],
        };

        let mut buf = Vec::new();
        fds.encode(&mut buf).unwrap();
        buf
    }

    #[test]
    fn test_load_descriptor_set() {
        let mut registry = SchemaRegistry::new();
        let bytes = create_test_descriptor_set();

        registry.load_descriptor_set(&bytes).unwrap();

        // Verify message can be resolved
        let msg = registry.get_message("test.TestMessage");
        assert!(msg.is_some(), "Message should be found");
        assert_eq!(msg.unwrap().full_name(), "test.TestMessage");
    }

    #[test]
    fn test_list_messages() {
        let mut registry = SchemaRegistry::new();
        let bytes = create_test_descriptor_set();

        registry.load_descriptor_set(&bytes).unwrap();

        let messages = registry.list_messages();
        assert!(
            messages.iter().any(|m| m == "test.TestMessage"),
            "TestMessage should be in list"
        );
    }

    #[test]
    fn test_load_invalid_descriptor() {
        let mut registry = SchemaRegistry::new();
        let garbage = b"not a valid descriptor set";

        let result = registry.load_descriptor_set(garbage);
        assert!(result.is_err(), "Should fail on invalid data");
        assert!(matches!(result.unwrap_err(), SchemaError::InvalidDescriptor(_)));
    }

    #[test]
    fn test_schema_resolver_trait() {
        let mut registry = SchemaRegistry::new();
        let bytes = create_test_descriptor_set();

        registry.load_descriptor_set(&bytes).unwrap();

        // Test via SchemaResolver trait
        use prb_core::SchemaResolver;
        let resolved = registry.resolve("test.TestMessage").unwrap();
        assert!(resolved.is_some());

        let schema = resolved.unwrap();
        assert_eq!(schema.name, "test.TestMessage");
        assert_eq!(schema.format, "protobuf");
        assert!(!schema.content.is_empty());
    }

    #[test]
    fn test_load_descriptor_set_file() {
        use std::io::Write;
        let temp_dir = tempfile::tempdir().unwrap();
        let desc_path = temp_dir.path().join("test.desc");

        // Write descriptor set to file
        let bytes = create_test_descriptor_set();
        let mut file = std::fs::File::create(&desc_path).unwrap();
        file.write_all(&bytes).unwrap();
        drop(file);

        // Load from file
        let mut registry = SchemaRegistry::new();
        registry.load_descriptor_set_file(&desc_path).unwrap();

        // Verify message can be resolved
        let msg = registry.get_message("test.TestMessage");
        assert!(msg.is_some());
    }

    #[test]
    fn test_load_proto_file() {
        use std::io::Write;
        let temp_dir = tempfile::tempdir().unwrap();
        let proto_path = temp_dir.path().join("simple.proto");

        // Write a simple .proto file
        let proto_content = r#"
syntax = "proto3";
package simple;

message SimpleMessage {
    int32 id = 1;
    string name = 2;
}
"#;
        let mut file = std::fs::File::create(&proto_path).unwrap();
        file.write_all(proto_content.as_bytes()).unwrap();
        drop(file);

        // Load and compile
        let mut registry = SchemaRegistry::new();
        registry.load_proto_files(&[&proto_path], &[temp_dir.path()]).unwrap();

        // Verify message can be resolved
        let msg = registry.get_message("simple.SimpleMessage");
        assert!(msg.is_some(), "SimpleMessage should be found after proto compilation");
    }

    #[test]
    fn test_load_proto_with_imports() {
        use std::io::Write;
        let temp_dir = tempfile::tempdir().unwrap();

        // Create base.proto
        let base_path = temp_dir.path().join("base.proto");
        let base_content = r#"
syntax = "proto3";
package base;

message BaseMessage {
    int32 id = 1;
}
"#;
        let mut file = std::fs::File::create(&base_path).unwrap();
        file.write_all(base_content.as_bytes()).unwrap();
        drop(file);

        // Create derived.proto that imports base.proto
        let derived_path = temp_dir.path().join("derived.proto");
        let derived_content = r#"
syntax = "proto3";
package derived;

import "base.proto";

message DerivedMessage {
    base.BaseMessage base = 1;
    string extra = 2;
}
"#;
        let mut file = std::fs::File::create(&derived_path).unwrap();
        file.write_all(derived_content.as_bytes()).unwrap();
        drop(file);

        // Load and compile with derived (should resolve imports)
        let mut registry = SchemaRegistry::new();
        registry.load_proto_files(&[&derived_path], &[temp_dir.path()]).unwrap();

        // Verify both messages can be resolved
        let base_msg = registry.get_message("base.BaseMessage");
        assert!(base_msg.is_some(), "BaseMessage should be found after import resolution");

        let derived_msg = registry.get_message("derived.DerivedMessage");
        assert!(derived_msg.is_some(), "DerivedMessage should be found");

        // List should contain both
        let messages = registry.list_messages();
        assert!(messages.iter().any(|m| m == "base.BaseMessage"));
        assert!(messages.iter().any(|m| m == "derived.DerivedMessage"));
    }
}
