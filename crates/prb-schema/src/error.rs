//! Error types for schema operations.

use std::path::PathBuf;

/// Result type for schema operations.
pub type Result<T> = std::result::Result<T, SchemaError>;

/// Errors that can occur during schema operations.
#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    /// Failed to load descriptor set.
    #[error("Failed to load descriptor set: {0}")]
    LoadDescriptorSet(String),

    /// Failed to compile .proto file.
    #[error("Failed to compile .proto file {file}: {message}")]
    CompileProto { file: PathBuf, message: String },

    /// Schema not found.
    #[error("Schema not found: {0}")]
    NotFound(String),

    /// Invalid descriptor data.
    #[error("Invalid descriptor data: {0}")]
    InvalidDescriptor(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Protobuf encoding error.
    #[error("Protobuf encoding error: {0}")]
    Prost(#[from] prost::DecodeError),
}

impl From<SchemaError> for prb_core::CoreError {
    fn from(e: SchemaError) -> Self {
        Self::Schema(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_error_load_descriptor_set() {
        let err = SchemaError::LoadDescriptorSet("file corrupted".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Failed to load descriptor set"));
        assert!(msg.contains("file corrupted"));
    }

    #[test]
    fn test_schema_error_compile_proto() {
        let err = SchemaError::CompileProto {
            file: PathBuf::from("service.proto"),
            message: "syntax error at line 5".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Failed to compile .proto file"));
        assert!(msg.contains("service.proto"));
        assert!(msg.contains("syntax error at line 5"));
    }

    #[test]
    fn test_schema_error_not_found() {
        let err = SchemaError::NotFound("service.Method".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Schema not found"));
        assert!(msg.contains("service.Method"));
    }

    #[test]
    fn test_schema_error_invalid_descriptor() {
        let err = SchemaError::InvalidDescriptor("malformed field descriptor".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Invalid descriptor data"));
        assert!(msg.contains("malformed field descriptor"));
    }

    #[test]
    fn test_schema_error_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err = SchemaError::from(io_err);
        let msg = err.to_string();
        assert!(msg.contains("I/O error"));
        assert!(msg.contains("access denied"));
    }

    #[test]
    fn test_schema_error_prost() {
        // Create a prost decode error by trying to decode invalid data
        let invalid_data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let decode_err = prost::decode_length_delimiter(&invalid_data[..]).unwrap_err();
        let err = SchemaError::from(decode_err);
        let msg = err.to_string();
        assert!(msg.contains("Protobuf encoding error"));
    }

    #[test]
    fn test_schema_error_to_core_error() {
        let err = SchemaError::NotFound("test.Service".to_string());
        let core_err: prb_core::CoreError = err.into();
        let msg = core_err.to_string();
        assert!(msg.contains("Schema not found"));
    }
}
