//! Configuration for live packet capture.

use std::path::PathBuf;

/// Configuration for a live packet capture session.
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    /// Network interface name to capture from.
    pub interface: String,

    /// Optional BPF filter expression (e.g., "tcp port 443").
    pub bpf_filter: Option<String>,

    /// Snapshot length (maximum bytes per packet).
    pub snaplen: u32,

    /// Enable promiscuous mode.
    pub promisc: bool,

    /// Enable immediate mode (deliver packets immediately, don't buffer).
    pub immediate_mode: bool,

    /// Kernel ring buffer size in bytes.
    pub buffer_size: u32,

    /// Read timeout in milliseconds.
    pub timeout_ms: i32,

    /// Optional path to TLS keylog file for decryption.
    pub tls_keylog_path: Option<PathBuf>,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            interface: String::new(),
            bpf_filter: None,
            snaplen: 65535, // Full packet capture
            promisc: true,
            immediate_mode: true,
            buffer_size: 16 * 1024 * 1024, // 16 MB
            timeout_ms: 1000,              // 1 second
            tls_keylog_path: None,
        }
    }
}

impl CaptureConfig {
    /// Create a new capture configuration for the specified interface.
    pub fn new(interface: impl Into<String>) -> Self {
        Self {
            interface: interface.into(),
            ..Default::default()
        }
    }

    /// Set the BPF filter expression.
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.bpf_filter = Some(filter.into());
        self
    }

    /// Set the snapshot length.
    #[must_use]
    pub const fn with_snaplen(mut self, snaplen: u32) -> Self {
        self.snaplen = snaplen;
        self
    }

    /// Set promiscuous mode.
    #[must_use]
    pub const fn with_promisc(mut self, promisc: bool) -> Self {
        self.promisc = promisc;
        self
    }

    /// Set the kernel buffer size.
    #[must_use]
    pub const fn with_buffer_size(mut self, size: u32) -> Self {
        self.buffer_size = size;
        self
    }

    /// Set the TLS keylog file path.
    pub fn with_tls_keylog(mut self, path: impl Into<PathBuf>) -> Self {
        self.tls_keylog_path = Some(path.into());
        self
    }
}
