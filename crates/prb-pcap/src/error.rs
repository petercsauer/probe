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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pcap_error_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = PcapError::from(io_err);
        let msg = err.to_string();
        assert!(msg.contains("I/O error"));
        assert!(msg.contains("file not found"));
    }

    #[test]
    fn test_pcap_error_parse() {
        let err = PcapError::Parse("bad magic number".to_string());
        let msg = err.to_string();
        assert!(msg.contains("PCAP parse error"));
        assert!(msg.contains("bad magic number"));
    }

    #[test]
    fn test_pcap_error_unsupported_format() {
        let err = PcapError::UnsupportedFormat("pcapng SHB missing".to_string());
        let msg = err.to_string();
        assert!(msg.contains("unsupported format"));
        assert!(msg.contains("pcapng SHB missing"));
    }

    #[test]
    fn test_pcap_error_invalid_linktype() {
        let err = PcapError::InvalidLinktype("unknown linktype 999".to_string());
        let msg = err.to_string();
        assert!(msg.contains("invalid linktype"));
        assert!(msg.contains("unknown linktype 999"));
    }

    #[test]
    fn test_pcap_error_tls_key() {
        let err = PcapError::TlsKey("missing CLIENT_RANDOM".to_string());
        let msg = err.to_string();
        assert!(msg.contains("TLS key error"));
        assert!(msg.contains("missing CLIENT_RANDOM"));
    }

    #[test]
    fn test_pcap_error_to_core_error() {
        let err = PcapError::Parse("test".to_string());
        let core_err: CoreError = err.into();
        let msg = core_err.to_string();
        assert!(msg.contains("PCAP parse error"));
    }
}
