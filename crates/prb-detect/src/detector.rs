//! Built-in protocol detectors.

use crate::types::{
    DetectionContext, DetectionMethod, DetectionResult, ProtocolDetector, ProtocolId,
    TransportLayer,
};
use std::collections::HashMap;

/// Static port → protocol mapping detector.
pub struct PortMappingDetector {
    tcp_mappings: HashMap<u16, ProtocolId>,
    udp_mappings: HashMap<u16, ProtocolId>,
}

impl PortMappingDetector {
    pub fn with_defaults() -> Self {
        let mut tcp_mappings = HashMap::new();
        tcp_mappings.insert(50051, ProtocolId::new(ProtocolId::GRPC));
        tcp_mappings.insert(80, ProtocolId::new(ProtocolId::HTTP2));
        tcp_mappings.insert(8080, ProtocolId::new(ProtocolId::HTTP2));
        tcp_mappings.insert(443, ProtocolId::new(ProtocolId::HTTP2));
        tcp_mappings.insert(8443, ProtocolId::new(ProtocolId::HTTP2));
        tcp_mappings.insert(5555, ProtocolId::new(ProtocolId::ZMTP));
        tcp_mappings.insert(5556, ProtocolId::new(ProtocolId::ZMTP));

        let mut udp_mappings = HashMap::new();
        // RTPS common port range: 7400-7500
        for port in 7400..=7500 {
            udp_mappings.insert(port, ProtocolId::new(ProtocolId::RTPS));
        }

        Self {
            tcp_mappings,
            udp_mappings,
        }
    }

    pub fn add_tcp_mapping(&mut self, port: u16, protocol: ProtocolId) {
        self.tcp_mappings.insert(port, protocol);
    }

    pub fn add_udp_mapping(&mut self, port: u16, protocol: ProtocolId) {
        self.udp_mappings.insert(port, protocol);
    }
}

impl ProtocolDetector for PortMappingDetector {
    fn name(&self) -> &str {
        "port-mapping"
    }

    fn transport(&self) -> TransportLayer {
        // This detector handles both TCP and UDP
        TransportLayer::Tcp
    }

    fn detect(&self, ctx: &DetectionContext<'_>) -> Option<DetectionResult> {
        let protocol = match ctx.transport {
            TransportLayer::Tcp => self
                .tcp_mappings
                .get(&ctx.dst_port)
                .or_else(|| self.tcp_mappings.get(&ctx.src_port)),
            TransportLayer::Udp => self
                .udp_mappings
                .get(&ctx.dst_port)
                .or_else(|| self.udp_mappings.get(&ctx.src_port)),
        }?;

        Some(DetectionResult {
            protocol: protocol.clone(),
            confidence: 0.5,
            method: DetectionMethod::PortMapping,
            version: None,
        })
    }
}

/// Detects HTTP/2 and gRPC via connection preface and frame heuristics.
pub struct GrpcDetector;

impl ProtocolDetector for GrpcDetector {
    fn name(&self) -> &str {
        "grpc-magic-bytes"
    }

    fn transport(&self) -> TransportLayer {
        TransportLayer::Tcp
    }

    fn detect(&self, ctx: &DetectionContext<'_>) -> Option<DetectionResult> {
        if ctx.transport != TransportLayer::Tcp {
            return None;
        }

        // Check for HTTP/2 connection preface
        const H2_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
        if ctx.initial_bytes.starts_with(H2_PREFACE) {
            return Some(DetectionResult {
                protocol: ProtocolId::new(ProtocolId::GRPC),
                confidence: 0.95,
                method: DetectionMethod::MagicBytes,
                version: Some("h2".into()),
            });
        }

        // Heuristic: HTTP/2 frame (9-byte header, type 0x00-0x09, length < 16MB)
        if ctx.initial_bytes.len() >= 9 {
            let len = u32::from_be_bytes([
                0,
                ctx.initial_bytes[0],
                ctx.initial_bytes[1],
                ctx.initial_bytes[2],
            ]);
            let frame_type = ctx.initial_bytes[3];
            if len < 16_777_216 && frame_type <= 0x09 {
                return Some(DetectionResult {
                    protocol: ProtocolId::new(ProtocolId::HTTP2),
                    confidence: 0.6,
                    method: DetectionMethod::Heuristic,
                    version: Some("h2".into()),
                });
            }
        }

        None
    }
}

/// Detects ZMTP 3.x via greeting signature.
pub struct ZmtpDetector;

impl ProtocolDetector for ZmtpDetector {
    fn name(&self) -> &str {
        "zmtp-magic-bytes"
    }

    fn transport(&self) -> TransportLayer {
        TransportLayer::Tcp
    }

    fn detect(&self, ctx: &DetectionContext<'_>) -> Option<DetectionResult> {
        if ctx.transport != TransportLayer::Tcp {
            return None;
        }
        if ctx.initial_bytes.len() < 10 {
            return None;
        }

        // ZMTP 3.x signature: [0xFF, 8 padding bytes, 0x7F]
        if ctx.initial_bytes[0] == 0xFF && ctx.initial_bytes[9] == 0x7F {
            let version = if ctx.initial_bytes.len() >= 11 {
                format!("3.{}", ctx.initial_bytes[10])
            } else {
                "3.x".into()
            };
            return Some(DetectionResult {
                protocol: ProtocolId::new(ProtocolId::ZMTP),
                confidence: 0.95,
                method: DetectionMethod::MagicBytes,
                version: Some(version),
            });
        }

        None
    }
}

/// Detects RTPS via magic bytes.
pub struct RtpsDetector;

impl ProtocolDetector for RtpsDetector {
    fn name(&self) -> &str {
        "rtps-magic-bytes"
    }

    fn transport(&self) -> TransportLayer {
        TransportLayer::Udp
    }

    fn detect(&self, ctx: &DetectionContext<'_>) -> Option<DetectionResult> {
        if ctx.transport != TransportLayer::Udp {
            return None;
        }
        if ctx.initial_bytes.len() < 4 {
            return None;
        }

        if &ctx.initial_bytes[0..4] == b"RTPS" {
            let version = if ctx.initial_bytes.len() >= 6 {
                Some(format!(
                    "{}.{}",
                    ctx.initial_bytes[4], ctx.initial_bytes[5]
                ))
            } else {
                None
            };
            return Some(DetectionResult {
                protocol: ProtocolId::new(ProtocolId::RTPS),
                confidence: 0.99,
                method: DetectionMethod::MagicBytes,
                version,
            });
        }

        None
    }
}

/// Wraps the `guess` crate for zero-copy detection.
pub struct GuessCrateDetector {
    tcp_detector: guess::ProtocolDetector<guess::Tcp>,
    udp_detector: guess::ProtocolDetector<guess::Udp>,
}

impl GuessCrateDetector {
    pub fn new() -> Self {
        Self {
            tcp_detector: guess::ProtocolDetector::builder()
                .tcp()
                .http()
                .tls()
                .ssh()
                .build(),
            udp_detector: guess::ProtocolDetector::builder()
                .udp()
                .dns()
                .build(),
        }
    }
}

impl Default for GuessCrateDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolDetector for GuessCrateDetector {
    fn name(&self) -> &str {
        "guess-crate"
    }

    fn transport(&self) -> TransportLayer {
        TransportLayer::Tcp
    }

    fn detect(&self, ctx: &DetectionContext<'_>) -> Option<DetectionResult> {
        let guess_result = match ctx.transport {
            TransportLayer::Tcp => self.tcp_detector.detect(ctx.initial_bytes),
            TransportLayer::Udp => self.udp_detector.detect(ctx.initial_bytes),
        };

        match guess_result {
            Ok(Some(protocol)) => {
                let protocol_id = match protocol {
                    guess::Protocol::Http => ProtocolId::new(ProtocolId::HTTP1),
                    guess::Protocol::Tls => ProtocolId::new(ProtocolId::TLS),
                    _ => ProtocolId::new(ProtocolId::UNKNOWN),
                };

                Some(DetectionResult {
                    protocol: protocol_id,
                    confidence: 0.85,
                    method: DetectionMethod::MagicBytes,
                    version: None,
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_mapping_tcp() {
        let detector = PortMappingDetector::with_defaults();
        let ctx = DetectionContext {
            initial_bytes: &[],
            src_port: 12345,
            dst_port: 50051,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        let result = detector.detect(&ctx).unwrap();
        assert_eq!(result.protocol.0, ProtocolId::GRPC);
        assert_eq!(result.confidence, 0.5);
        assert_eq!(result.method, DetectionMethod::PortMapping);
    }

    #[test]
    fn test_port_mapping_udp_rtps() {
        let detector = PortMappingDetector::with_defaults();
        let ctx = DetectionContext {
            initial_bytes: &[],
            src_port: 12345,
            dst_port: 7400,
            transport: TransportLayer::Udp,
            tls_decrypted: false,
        };

        let result = detector.detect(&ctx).unwrap();
        assert_eq!(result.protocol.0, ProtocolId::RTPS);
        assert_eq!(result.confidence, 0.5);
        assert_eq!(result.method, DetectionMethod::PortMapping);
    }

    #[test]
    fn test_grpc_detector_preface() {
        let detector = GrpcDetector;
        let preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
        let ctx = DetectionContext {
            initial_bytes: preface,
            src_port: 12345,
            dst_port: 50051,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        let result = detector.detect(&ctx).unwrap();
        assert_eq!(result.protocol.0, ProtocolId::GRPC);
        assert_eq!(result.confidence, 0.95);
        assert_eq!(result.method, DetectionMethod::MagicBytes);
        assert_eq!(result.version.as_deref(), Some("h2"));
    }

    #[test]
    fn test_grpc_detector_heuristic() {
        let detector = GrpcDetector;
        // HTTP/2 SETTINGS frame: 3-byte length (0x00000C), type 0x04, flags, stream ID
        let frame = [0x00, 0x00, 0x0C, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00];
        let ctx = DetectionContext {
            initial_bytes: &frame,
            src_port: 12345,
            dst_port: 50051,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        let result = detector.detect(&ctx).unwrap();
        assert_eq!(result.protocol.0, ProtocolId::HTTP2);
        assert_eq!(result.confidence, 0.6);
        assert_eq!(result.method, DetectionMethod::Heuristic);
    }

    #[test]
    fn test_grpc_detector_no_match() {
        let detector = GrpcDetector;
        let ctx = DetectionContext {
            initial_bytes: b"random data",
            src_port: 12345,
            dst_port: 50051,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        assert!(detector.detect(&ctx).is_none());
    }

    #[test]
    fn test_zmtp_detector_v30() {
        let detector = ZmtpDetector;
        // ZMTP 3.0 greeting
        let greeting = [0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0x7F, 0x00];
        let ctx = DetectionContext {
            initial_bytes: &greeting,
            src_port: 12345,
            dst_port: 5555,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        let result = detector.detect(&ctx).unwrap();
        assert_eq!(result.protocol.0, ProtocolId::ZMTP);
        assert_eq!(result.confidence, 0.95);
        assert_eq!(result.method, DetectionMethod::MagicBytes);
        assert_eq!(result.version.as_deref(), Some("3.0"));
    }

    #[test]
    fn test_zmtp_detector_v31() {
        let detector = ZmtpDetector;
        // ZMTP 3.1 greeting
        let greeting = [0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0x7F, 0x01];
        let ctx = DetectionContext {
            initial_bytes: &greeting,
            src_port: 12345,
            dst_port: 5555,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        let result = detector.detect(&ctx).unwrap();
        assert_eq!(result.protocol.0, ProtocolId::ZMTP);
        assert_eq!(result.version.as_deref(), Some("3.1"));
    }

    #[test]
    fn test_zmtp_detector_no_match() {
        let detector = ZmtpDetector;
        let ctx = DetectionContext {
            initial_bytes: b"random data",
            src_port: 12345,
            dst_port: 5555,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        assert!(detector.detect(&ctx).is_none());
    }

    #[test]
    fn test_rtps_detector_v23() {
        let detector = RtpsDetector;
        let header = b"RTPS\x02\x03";
        let ctx = DetectionContext {
            initial_bytes: header,
            src_port: 12345,
            dst_port: 7400,
            transport: TransportLayer::Udp,
            tls_decrypted: false,
        };

        let result = detector.detect(&ctx).unwrap();
        assert_eq!(result.protocol.0, ProtocolId::RTPS);
        assert_eq!(result.confidence, 0.99);
        assert_eq!(result.method, DetectionMethod::MagicBytes);
        assert_eq!(result.version.as_deref(), Some("2.3"));
    }

    #[test]
    fn test_rtps_detector_no_match() {
        let detector = RtpsDetector;
        let ctx = DetectionContext {
            initial_bytes: b"random data",
            src_port: 12345,
            dst_port: 7400,
            transport: TransportLayer::Udp,
            tls_decrypted: false,
        };

        assert!(detector.detect(&ctx).is_none());
    }

    #[test]
    fn test_port_mapping_custom() {
        let mut detector = PortMappingDetector::with_defaults();
        detector.add_tcp_mapping(9999, ProtocolId::new("custom-proto"));
        detector.add_udp_mapping(8888, ProtocolId::new("custom-udp"));

        let tcp_ctx = DetectionContext {
            initial_bytes: &[],
            src_port: 12345,
            dst_port: 9999,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };
        let result = detector.detect(&tcp_ctx).unwrap();
        assert_eq!(result.protocol.0, "custom-proto");

        let udp_ctx = DetectionContext {
            initial_bytes: &[],
            src_port: 12345,
            dst_port: 8888,
            transport: TransportLayer::Udp,
            tls_decrypted: false,
        };
        let result = detector.detect(&udp_ctx).unwrap();
        assert_eq!(result.protocol.0, "custom-udp");
    }

    #[test]
    fn test_grpc_detector_short_payload() {
        let detector = GrpcDetector;
        let ctx = DetectionContext {
            initial_bytes: b"PRI",
            src_port: 12345,
            dst_port: 50051,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        assert!(detector.detect(&ctx).is_none());
    }

    #[test]
    fn test_grpc_detector_wrong_transport() {
        let detector = GrpcDetector;
        let preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
        let ctx = DetectionContext {
            initial_bytes: preface,
            src_port: 12345,
            dst_port: 50051,
            transport: TransportLayer::Udp,
            tls_decrypted: false,
        };

        assert!(detector.detect(&ctx).is_none());
    }

    #[test]
    fn test_zmtp_detector_short_payload() {
        let detector = ZmtpDetector;
        let ctx = DetectionContext {
            initial_bytes: &[0xFF, 0, 0, 0],
            src_port: 12345,
            dst_port: 5555,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        assert!(detector.detect(&ctx).is_none());
    }

    #[test]
    fn test_rtps_detector_short_payload() {
        let detector = RtpsDetector;
        let ctx = DetectionContext {
            initial_bytes: b"RT",
            src_port: 12345,
            dst_port: 7400,
            transport: TransportLayer::Udp,
            tls_decrypted: false,
        };

        assert!(detector.detect(&ctx).is_none());
    }

    #[test]
    fn test_rtps_detector_wrong_transport() {
        let detector = RtpsDetector;
        let header = b"RTPS\x02\x03";
        let ctx = DetectionContext {
            initial_bytes: header,
            src_port: 12345,
            dst_port: 7400,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        assert!(detector.detect(&ctx).is_none());
    }

    #[test]
    fn test_port_mapping_source_port() {
        let detector = PortMappingDetector::with_defaults();
        let ctx = DetectionContext {
            initial_bytes: &[],
            src_port: 50051,
            dst_port: 12345,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        let result = detector.detect(&ctx).unwrap();
        assert_eq!(result.protocol.0, ProtocolId::GRPC);
    }
}
