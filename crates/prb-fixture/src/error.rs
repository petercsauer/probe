//! Error types for prb-fixture.

use std::fmt;

/// Fixture-specific error type.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FixtureError {
    /// I/O error reading fixture file.
    #[error("I/O error: {source}")]
    Io {
        /// The underlying I/O error.
        #[from]
        source: std::io::Error,
    },

    /// JSON parse error.
    #[error("JSON parse error: {source}")]
    Parse {
        /// The underlying JSON parse error.
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

    /// Core error.
    #[error(transparent)]
    Core(#[from] prb_core::CoreError),
}

impl FixtureError {
    /// Create an invalid format error.
    pub fn invalid_format(msg: impl fmt::Display) -> Self {
        Self::InvalidFormat(msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixture_error_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "fixture.json not found");
        let err = FixtureError::from(io_err);
        let msg = err.to_string();
        assert!(msg.contains("I/O error"));
        assert!(msg.contains("fixture.json not found"));
    }

    #[test]
    fn test_fixture_error_parse() {
        let json_str = "{ invalid json }";
        let parse_err = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let err = FixtureError::from(parse_err);
        let msg = err.to_string();
        assert!(msg.contains("JSON parse error"));
    }

    #[test]
    fn test_fixture_error_unsupported_version() {
        let err = FixtureError::UnsupportedVersion(99);
        let msg = err.to_string();
        assert!(msg.contains("unsupported fixture version"));
        assert!(msg.contains("99"));
    }

    #[test]
    fn test_fixture_error_invalid_format() {
        let err = FixtureError::InvalidFormat("missing required field".to_string());
        let msg = err.to_string();
        assert!(msg.contains("invalid fixture format"));
        assert!(msg.contains("missing required field"));
    }

    #[test]
    fn test_fixture_error_invalid_format_helper() {
        let err = FixtureError::invalid_format("test message");
        let msg = err.to_string();
        assert!(msg.contains("invalid fixture format"));
        assert!(msg.contains("test message"));
    }

    #[test]
    fn test_fixture_error_base64_decode() {
        let err = FixtureError::Base64Decode("invalid character".to_string());
        let msg = err.to_string();
        assert!(msg.contains("base64 decode error"));
        assert!(msg.contains("invalid character"));
    }

    #[test]
    fn test_fixture_error_invalid_transport() {
        let err = FixtureError::InvalidTransport("unknown-protocol".to_string());
        let msg = err.to_string();
        assert!(msg.contains("invalid transport kind"));
        assert!(msg.contains("unknown-protocol"));
    }

    #[test]
    fn test_fixture_error_invalid_direction() {
        let err = FixtureError::InvalidDirection("sideways".to_string());
        let msg = err.to_string();
        assert!(msg.contains("invalid direction"));
        assert!(msg.contains("sideways"));
    }

    #[test]
    fn test_fixture_error_core() {
        let core_err = prb_core::CoreError::Schema("schema error".to_string());
        let err = FixtureError::from(core_err);
        let msg = err.to_string();
        assert!(msg.contains("schema error"));
    }
}
