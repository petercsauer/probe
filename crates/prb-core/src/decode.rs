//! Decoding context for protocol decoders.

use crate::event::{
    DebugEvent, DebugEventBuilder, EventSource, NetworkAddr, Timestamp, TransportKind,
};

/// Decoding context for `ProtocolDecoder`.
///
/// Provides metadata and context information to protocol decoders during
/// the decoding process.
///
/// # Examples
///
/// ```
/// use prb_core::{DecodeContext, Timestamp};
///
/// let ctx = DecodeContext::new()
///     .with_src_addr("192.168.1.100:8080")
///     .with_dst_addr("10.0.0.1:443")
///     .with_metadata("interface", "eth0")
///     .with_timestamp(Timestamp::from_nanos(1_000_000_000));
///
/// assert_eq!(ctx.src_addr.as_ref().unwrap(), "192.168.1.100:8080");
/// assert_eq!(ctx.metadata.get("interface").unwrap(), "eth0");
/// ```
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

    /// Create a `DebugEventBuilder` pre-populated with context metadata.
    ///
    /// This helper centralizes event building logic shared across protocol decoders.
    /// It pre-populates timestamp, source (adapter, origin, network addresses), and transport.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    /// use prb_core::{DecodeContext, Direction, Payload, Timestamp, TransportKind};
    ///
    /// let ctx = DecodeContext::new()
    ///     .with_src_addr("192.168.1.100:8080")
    ///     .with_dst_addr("10.0.0.1:443")
    ///     .with_timestamp(Timestamp::from_nanos(1_000_000_000));
    ///
    /// let event = ctx.create_event_builder(TransportKind::Grpc)
    ///     .direction(Direction::Inbound)
    ///     .payload(Payload::Raw { raw: Bytes::from("test") })
    ///     .build();
    /// ```
    #[must_use]
    pub fn create_event_builder(&self, transport: TransportKind) -> DebugEventBuilder {
        let mut builder = DebugEvent::builder().transport(transport);

        // Set timestamp
        if let Some(ts) = self.timestamp {
            builder = builder.timestamp(ts);
        }

        // Set source with network addresses if available
        if let Some(ref src) = self.src_addr {
            if let Some(ref dst) = self.dst_addr {
                builder = builder.source(EventSource {
                    adapter: "pcap".to_string(),
                    origin: self
                        .metadata
                        .get("origin")
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_string()),
                    network: Some(NetworkAddr {
                        src: src.clone(),
                        dst: dst.clone(),
                    }),
                });
            }
        }

        builder
    }
}

impl Default for DecodeContext {
    fn default() -> Self {
        Self::new()
    }
}
