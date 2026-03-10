//! Error types for ZMTP parsing.

use thiserror::Error;

/// Errors that can occur during ZMTP parsing.
#[derive(Debug, Error)]
pub enum ZmqError {
    #[error("Invalid ZMTP greeting signature: expected 0xFF at byte 0 and 0x7F at byte 9")]
    InvalidGreetingSignature,

    #[error("Unsupported ZMTP version {major}.{minor}")]
    UnsupportedVersion { major: u8, minor: u8 },

    #[error("Invalid frame flag byte: 0x{0:02X} (bits 7-3 must be zero)")]
    InvalidFlagByte(u8),

    #[error("Invalid command name length: {0}")]
    InvalidCommandNameLength(u8),

    #[error("Invalid property metadata: {0}")]
    InvalidPropertyMetadata(String),

    #[error("Frame too large: {0} bytes")]
    FrameTooLarge(u64),

    #[error("Too many frames in multipart message: {0}")]
    TooManyFrames(usize),

    #[error("UTF-8 decode error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("UTF-8 string decode error: {0}")]
    Utf8StrError(#[from] std::str::Utf8Error),
}
