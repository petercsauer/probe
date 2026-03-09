//! Error types for prb-core.

use std::fmt;

/// Core error type for PRB operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CoreError {
    /// Invalid event ID.
    #[error("invalid event ID: {0}")]
    InvalidEventId(String),

    /// Invalid timestamp value.
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(String),

    /// Serialization error.
    #[error("serialization error: {source}")]
    Serialization {
        #[from]
        source: serde_json::Error,
    },

    /// Invalid payload data.
    #[error("invalid payload: {0}")]
    InvalidPayload(String),

    /// Generic error with context.
    #[error("{0}")]
    Other(String),
}

impl CoreError {
    /// Create a new generic error with a message.
    pub fn other(msg: impl fmt::Display) -> Self {
        Self::Other(msg.to_string())
    }
}
