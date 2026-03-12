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
