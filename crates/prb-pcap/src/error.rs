//! Error types for PCAP operations.

use prb_core::CoreError;

/// Errors that can occur during PCAP/pcapng file operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PcapError {
    /// I/O error while reading capture file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Parse error from pcap-parser.
    #[error("PCAP parse error: {0}")]
    Parse(String),

    /// Unsupported or corrupted capture format.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    /// Invalid linktype or interface reference.
    #[error("invalid linktype: {0}")]
    InvalidLinktype(String),

    /// TLS key extraction error.
    #[error("TLS key error: {0}")]
    TlsKey(String),
}

impl From<PcapError> for CoreError {
    fn from(err: PcapError) -> Self {
        Self::PayloadDecode(err.to_string())
    }
}
