//! Error types for live packet capture.

/// Errors that can occur during live packet capture.
#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    /// Error from the underlying pcap library.
    #[error("pcap error: {0}")]
    Pcap(#[from] pcap::Error),

    /// Specified network interface was not found.
    #[error("interface not found: {0}")]
    InterfaceNotFound(String),

    /// Insufficient privileges to capture packets.
    #[error("insufficient privileges: {message}\n\nFix: {remediation}")]
    InsufficientPrivileges {
        /// Description of the privilege issue.
        message: String,
        /// Suggested remediation steps.
        remediation: String,
    },

    /// BPF filter compilation failed.
    #[error("BPF filter compilation failed: {0}")]
    FilterCompilationFailed(String),

    /// Capture channel was closed unexpectedly.
    #[error("capture channel closed")]
    ChannelClosed,

    /// Capture is already running.
    #[error("capture already running")]
    AlreadyRunning,

    /// Generic error with custom message.
    #[error("{0}")]
    Other(String),
}
