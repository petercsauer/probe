//! Types for protocol detection.

/// Unique identifier for a protocol.
///
/// Built-in protocols use well-known string IDs. Plugin protocols use
/// their registered name (e.g., "thrift", "capnproto").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProtocolId(pub String);

impl ProtocolId {
    pub const GRPC: &str = "grpc";
    pub const ZMTP: &str = "zmtp";
    pub const RTPS: &str = "rtps";
    pub const HTTP2: &str = "http2";
    pub const HTTP1: &str = "http1";
    pub const TLS: &str = "tls";
    pub const UNKNOWN: &str = "unknown";

    pub fn new(id: &str) -> Self {
        Self(id.to_string())
    }
}

impl From<String> for ProtocolId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ProtocolId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Result of protocol detection.
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// Detected protocol identifier.
    pub protocol: ProtocolId,
    /// Confidence level (0.0 = guess, 1.0 = certain).
    pub confidence: f32,
    /// How the protocol was detected.
    pub method: DetectionMethod,
    /// Optional protocol version (e.g., "3.1" for ZMTP).
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectionMethod {
    /// User explicitly specified the protocol.
    UserOverride,
    /// Matched a known port mapping.
    PortMapping,
    /// Matched magic bytes in the payload.
    MagicBytes,
    /// Heuristic analysis of payload content.
    Heuristic,
    /// No detection succeeded; using fallback.
    Fallback,
}

/// Context available during protocol detection.
pub struct DetectionContext<'a> {
    /// First bytes of the stream/datagram (up to 256 bytes).
    pub initial_bytes: &'a [u8],
    /// Source port.
    pub src_port: u16,
    /// Destination port.
    pub dst_port: u16,
    /// Transport layer (tcp or udp).
    pub transport: TransportLayer,
    /// Whether the stream was TLS-decrypted.
    pub tls_decrypted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportLayer {
    Tcp,
    Udp,
}

/// Detects whether a byte stream or datagram matches a specific protocol.
///
/// Detectors are ordered by priority. The first detector that returns a
/// result with confidence >= threshold wins.
pub trait ProtocolDetector: Send + Sync {
    /// Human-readable name for logging (e.g., "grpc-magic-bytes").
    fn name(&self) -> &str;

    /// Which transport layer this detector applies to.
    fn transport(&self) -> TransportLayer;

    /// Attempt to detect the protocol from the initial bytes and metadata.
    ///
    /// Returns `Some(result)` if the protocol was identified, `None` if
    /// this detector cannot determine the protocol.
    fn detect(&self, ctx: &DetectionContext<'_>) -> Option<DetectionResult>;
}
