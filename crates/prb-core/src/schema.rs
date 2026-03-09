//! Schema resolution types.

/// Resolved schema information.
#[derive(Debug, Clone)]
pub struct ResolvedSchema {
    /// Schema name.
    pub name: String,
    /// Schema content (e.g., protobuf FileDescriptorSet bytes, JSON schema).
    pub content: bytes::Bytes,
    /// Schema format identifier (e.g., "protobuf", "json-schema").
    pub format: String,
}
