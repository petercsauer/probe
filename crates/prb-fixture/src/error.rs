//! Error types for prb-fixture.

use std::fmt;

/// Fixture-specific error type.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FixtureError {
    /// I/O error reading fixture file.
    #[error("I/O error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    /// JSON parse error.
    #[error("JSON parse error: {source}")]
    Parse {
        #[from]
        source: serde_json::Error,
    },

    /// Unsupported fixture version.
    #[error("unsupported fixture version: {0}")]
    UnsupportedVersion(u64),

    /// Invalid fixture format.
    #[error("invalid fixture format: {0}")]
    InvalidFormat(String),

    /// Base64 decode error.
    #[error("base64 decode error: {0}")]
    Base64Decode(String),

    /// Invalid transport kind.
    #[error("invalid transport kind: {0}")]
    InvalidTransport(String),

    /// Invalid direction.
    #[error("invalid direction: {0}")]
    InvalidDirection(String),
}

impl FixtureError {
    /// Create an invalid format error.
    pub fn invalid_format(msg: impl fmt::Display) -> Self {
        Self::InvalidFormat(msg.to_string())
    }
}

impl From<FixtureError> for prb_core::CoreError {
    fn from(err: FixtureError) -> Self {
        prb_core::CoreError::other(err)
    }
}
