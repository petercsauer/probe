//! Decoding context for protocol decoders.

use crate::event::Timestamp;

/// Decoding context for `ProtocolDecoder`.
#[derive(Debug, Clone)]
pub struct DecodeContext {
    /// Source address information (e.g., "192.168.1.1:8080").
    pub src_addr: Option<String>,
    /// Destination address information.
    pub dst_addr: Option<String>,
    /// Additional context metadata.
    pub metadata: std::collections::BTreeMap<String, String>,
    /// Timestamp of the event (from capture or live data).
    pub timestamp: Option<Timestamp>,
}

impl DecodeContext {
    /// Create a new empty context.
    #[must_use] 
    pub const fn new() -> Self {
        Self {
            src_addr: None,
            dst_addr: None,
            metadata: std::collections::BTreeMap::new(),
            timestamp: None,
        }
    }

    /// Set source address.
    #[must_use]
    pub fn with_src_addr(mut self, addr: impl Into<String>) -> Self {
        self.src_addr = Some(addr.into());
        self
    }

    /// Set destination address.
    #[must_use]
    pub fn with_dst_addr(mut self, addr: impl Into<String>) -> Self {
        self.dst_addr = Some(addr.into());
        self
    }

    /// Add metadata entry.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set timestamp.
    #[must_use] 
    pub const fn with_timestamp(mut self, timestamp: Timestamp) -> Self {
        self.timestamp = Some(timestamp);
        self
    }
}

impl Default for DecodeContext {
    fn default() -> Self {
        Self::new()
    }
}
