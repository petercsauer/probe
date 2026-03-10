//! Error types for gRPC decoder.

use thiserror::Error;

/// Error type for gRPC decoding operations.
#[derive(Debug, Error)]
pub enum GrpcError {
    /// HTTP/2 frame parsing error.
    #[error("HTTP/2 frame parsing error: {0}")]
    H2FrameError(String),

    /// HPACK header decompression error.
    #[error("HPACK decompression error: {0}")]
    HpackError(String),

    /// gRPC message parsing error.
    #[error("gRPC message parsing error: {0}")]
    MessageError(String),

    /// Decompression error.
    #[error("Decompression error: {0}")]
    DecompressionError(String),

    /// Invalid gRPC state.
    #[error("Invalid gRPC state: {0}")]
    InvalidState(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
