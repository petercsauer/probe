//! Error types for ZMTP parsing.

use thiserror::Error;

/// Errors that can occur during ZMTP parsing.
#[derive(Debug, Error)]
pub enum ZmqError {
    /// Invalid ZMTP greeting signature.
    #[error("Invalid ZMTP greeting signature: expected 0xFF at byte 0 and 0x7F at byte 9")]
    InvalidGreetingSignature,

    /// Unsupported ZMTP version.
    #[error("Unsupported ZMTP version {major}.{minor}")]
    UnsupportedVersion {
        /// Major version number.
        major: u8,
        /// Minor version number.
        minor: u8,
    },

    /// Invalid frame flag byte.
    #[error("Invalid frame flag byte: 0x{0:02X} (bits 7-3 must be zero)")]
    InvalidFlagByte(u8),

    /// Invalid command name length.
    #[error("Invalid command name length: {0}")]
    InvalidCommandNameLength(u8),

    /// Invalid property metadata.
    #[error("Invalid property metadata: {0}")]
    InvalidPropertyMetadata(String),

    /// Frame too large.
    #[error("Frame too large: {0} bytes")]
    FrameTooLarge(u64),

    /// Too many frames in multipart message.
    #[error("Too many frames in multipart message: {0}")]
    TooManyFrames(usize),

    /// UTF-8 decode error.
    #[error("UTF-8 decode error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    /// UTF-8 string decode error.
    #[error("UTF-8 string decode error: {0}")]
    Utf8StrError(#[from] std::str::Utf8Error),
}
