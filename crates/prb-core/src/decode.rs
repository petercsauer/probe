//! Decoding context for protocol decoders.

/// Decoding context for ProtocolDecoder.
#[derive(Debug, Clone)]
pub struct DecodeContext {
    /// Source address information (e.g., "192.168.1.1:8080").
    pub src_addr: Option<String>,
    /// Destination address information.
    pub dst_addr: Option<String>,
    /// Additional context metadata.
    pub metadata: std::collections::BTreeMap<String, String>,
}

impl DecodeContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self {
            src_addr: None,
            dst_addr: None,
            metadata: std::collections::BTreeMap::new(),
        }
    }

    /// Set source address.
    pub fn with_src_addr(mut self, addr: impl Into<String>) -> Self {
        self.src_addr = Some(addr.into());
        self
    }

    /// Set destination address.
    pub fn with_dst_addr(mut self, addr: impl Into<String>) -> Self {
        self.dst_addr = Some(addr.into());
        self
    }

    /// Add metadata entry.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

impl Default for DecodeContext {
    fn default() -> Self {
        Self::new()
    }
}
