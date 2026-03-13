//! Schema resolution types.

/// Resolved schema information.
///
/// Contains the schema content and metadata needed to decode protocol messages.
///
/// # Examples
///
/// ```
/// use prb_core::ResolvedSchema;
/// use bytes::Bytes;
///
/// let schema = ResolvedSchema {
///     name: "example.Message".to_string(),
///     content: Bytes::from(vec![0x0a, 0x0b]),
///     format: "protobuf".to_string(),
/// };
///
/// assert_eq!(schema.name, "example.Message");
/// assert_eq!(schema.format, "protobuf");
/// ```
#[derive(Debug, Clone)]
pub struct ResolvedSchema {
    /// Schema name.
    pub name: String,
    /// Schema content (e.g., protobuf `FileDescriptorSet` bytes, JSON schema).
    pub content: bytes::Bytes,
    /// Schema format identifier (e.g., "protobuf", "json-schema").
    pub format: String,
}
