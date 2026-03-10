//! Error types for DDS/RTPS decoding.

use thiserror::Error;

/// Errors that can occur during DDS/RTPS decoding.
#[derive(Debug, Error)]
pub enum DdsError {
    /// RTPS message parsing error.
    #[error("RTPS parse error: {0}")]
    RtpsParse(String),

    /// CDR deserialization error.
    #[error("CDR decode error: {0}")]
    CdrDecode(String),

    /// Discovery data parsing error.
    #[error("Discovery parse error: {0}")]
    DiscoveryParse(String),

    /// Invalid RTPS magic number.
    #[error("Invalid RTPS magic: expected 'RTPS', found {0:?}")]
    InvalidMagic([u8; 4]),

    /// General decoding error.
    #[error("Decode error: {0}")]
    Decode(String),
}
