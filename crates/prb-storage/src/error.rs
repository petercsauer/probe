//! Error types for the storage layer.

use std::io;
use thiserror::Error;

/// Errors that can occur in the storage layer.
#[derive(Debug, Error)]
pub enum StorageError {
    /// MCAP file operation failed.
    #[error("MCAP error: {0}")]
    Mcap(#[from] mcap::McapError),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Invalid session file.
    #[error("Invalid session file: {0}")]
    InvalidSession(String),

    /// Channel not found.
    #[error("Channel not found: {0}")]
    ChannelNotFound(String),
}

/// Result type for storage operations.
pub type Result<T> = std::result::Result<T, StorageError>;
