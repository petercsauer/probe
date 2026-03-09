//! Error types for prb-core.

/// Core error type for PRB operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CoreError {
    /// Invalid timestamp value.
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(String),

    /// Payload decode failed.
    #[error("payload decode failed: {0}")]
    PayloadDecode(String),

    /// Unsupported transport type.
    #[error("unsupported transport: {0}")]
    UnsupportedTransport(String),

    /// Serialization error.
    #[error("serialization error: {source}")]
    Serialization {
        #[from]
        source: serde_json::Error,
    },
}

