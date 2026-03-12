//! Types for protocol detection.

/// Unique identifier for a protocol.
///
/// Built-in protocols use well-known string IDs. Plugin protocols use
/// their registered name (e.g., "thrift", "capnproto").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProtocolId(pub String);

impl ProtocolId {
    /// Protocol ID for gRPC.
    pub const GRPC: &str = "grpc";
    /// Protocol ID for `ZeroMQ` (ZMTP).
    pub const ZMTP: &str = "zmtp";
    /// Protocol ID for DDS RTPS.
    pub const RTPS: &str = "rtps";
    /// Protocol ID for HTTP/2.
    pub const HTTP2: &str = "http2";
    /// Protocol ID for HTTP/1.x.
    pub const HTTP1: &str = "http1";
    /// Protocol ID for TLS.
    pub const TLS: &str = "tls";
    /// Protocol ID for unknown/unidentified protocols.
    pub const UNKNOWN: &str = "unknown";

    /// Create a new protocol ID from a string.
    #[must_use] 
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

/// Method used to detect a protocol.
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

/// Transport layer (TCP or UDP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportLayer {
    /// TCP transport.
    Tcp,
    /// UDP transport.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_id_constants() {
        assert_eq!(ProtocolId::GRPC, "grpc");
        assert_eq!(ProtocolId::ZMTP, "zmtp");
        assert_eq!(ProtocolId::RTPS, "rtps");
        assert_eq!(ProtocolId::HTTP2, "http2");
        assert_eq!(ProtocolId::HTTP1, "http1");
        assert_eq!(ProtocolId::TLS, "tls");
        assert_eq!(ProtocolId::UNKNOWN, "unknown");
    }

    #[test]
    fn protocol_id_from_string() {
        let id: ProtocolId = "custom".into();
        assert_eq!(id.0, "custom");

        let id: ProtocolId = String::from("another").into();
        assert_eq!(id.0, "another");

        let id = ProtocolId::new(ProtocolId::GRPC);
        assert_eq!(id.0, ProtocolId::GRPC);
    }

    #[test]
    fn detection_method_equality() {
        assert_eq!(DetectionMethod::UserOverride, DetectionMethod::UserOverride);
        assert_ne!(DetectionMethod::PortMapping, DetectionMethod::MagicBytes);
        assert_eq!(DetectionMethod::Heuristic, DetectionMethod::Heuristic);
    }

    #[test]
    fn transport_layer_equality() {
        assert_eq!(TransportLayer::Tcp, TransportLayer::Tcp);
        assert_ne!(TransportLayer::Tcp, TransportLayer::Udp);
    }
}
