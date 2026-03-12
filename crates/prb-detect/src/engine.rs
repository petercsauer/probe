//! Detection engine that orchestrates multiple detectors.

use crate::detector::{
    GrpcDetector, GuessCrateDetector, PortMappingDetector, RtpsDetector, ZmtpDetector,
};
use crate::types::{
    DetectionContext, DetectionMethod, DetectionResult, ProtocolDetector, ProtocolId,
};

/// Runs detectors in priority order and returns the highest-confidence match.
pub struct DetectionEngine {
    /// Detectors ordered by priority (highest first).
    detectors: Vec<Box<dyn ProtocolDetector>>,
    /// Minimum confidence to accept a detection result.
    confidence_threshold: f32,
}

impl DetectionEngine {
    /// Create a new detection engine with default detectors.
    pub fn with_defaults() -> Self {
        let detectors: Vec<Box<dyn ProtocolDetector>> = vec![
            Box::new(PortMappingDetector::with_defaults()),
            Box::new(GrpcDetector),
            Box::new(ZmtpDetector),
            Box::new(RtpsDetector),
            Box::new(GuessCrateDetector::new()),
        ];

        Self {
            detectors,
            confidence_threshold: 0.5,
        }
    }

    /// Create an empty detection engine.
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
            confidence_threshold: 0.5,
        }
    }

    /// Set the confidence threshold.
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.confidence_threshold = threshold;
        self
    }

    /// Add a detector to the engine.
    pub fn add_detector(&mut self, detector: Box<dyn ProtocolDetector>) {
        self.detectors.push(detector);
    }

    /// Detect the protocol for a given context.
    ///
    /// Tries all detectors and returns the result with the highest confidence.
    /// If no detector exceeds the threshold, returns an UNKNOWN fallback.
    pub fn detect(&self, ctx: &DetectionContext<'_>) -> DetectionResult {
        let mut best_result: Option<DetectionResult> = None;
        let mut best_confidence = 0.0;

        for detector in &self.detectors {
            if let Some(result) = detector.detect(ctx) {
                tracing::debug!(
                    detector = detector.name(),
                    protocol = %result.protocol.0,
                    confidence = result.confidence,
                    "Detector matched"
                );

                if result.confidence > best_confidence {
                    best_confidence = result.confidence;
                    best_result = Some(result);
                }
            }
        }

        if let Some(result) = best_result
            && result.confidence >= self.confidence_threshold
        {
            return result;
        }

        // Fallback to UNKNOWN
        DetectionResult {
            protocol: ProtocolId::new(ProtocolId::UNKNOWN),
            confidence: 0.0,
            method: DetectionMethod::Fallback,
            version: None,
        }
    }
}

impl Default for DetectionEngine {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TransportLayer;

    #[test]
    fn test_engine_grpc_preface() {
        let engine = DetectionEngine::with_defaults();
        let preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
        let ctx = DetectionContext {
            initial_bytes: preface,
            src_port: 12345,
            dst_port: 50051,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        let result = engine.detect(&ctx);
        assert_eq!(result.protocol.0, ProtocolId::GRPC);
        assert_eq!(result.confidence, 0.95);
        assert_eq!(result.method, DetectionMethod::MagicBytes);
    }

    #[test]
    fn test_engine_zmtp_greeting() {
        let engine = DetectionEngine::with_defaults();
        let greeting = [0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0x7F, 0x01];
        let ctx = DetectionContext {
            initial_bytes: &greeting,
            src_port: 12345,
            dst_port: 5555,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        let result = engine.detect(&ctx);
        assert_eq!(result.protocol.0, ProtocolId::ZMTP);
        assert_eq!(result.confidence, 0.95);
        assert_eq!(result.method, DetectionMethod::MagicBytes);
    }

    #[test]
    fn test_engine_rtps_header() {
        let engine = DetectionEngine::with_defaults();
        let header = b"RTPS\x02\x03";
        let ctx = DetectionContext {
            initial_bytes: header,
            src_port: 12345,
            dst_port: 7400,
            transport: TransportLayer::Udp,
            tls_decrypted: false,
        };

        let result = engine.detect(&ctx);
        assert_eq!(result.protocol.0, ProtocolId::RTPS);
        assert_eq!(result.confidence, 0.99);
        assert_eq!(result.method, DetectionMethod::MagicBytes);
    }

    #[test]
    fn test_engine_unknown_fallback() {
        let engine = DetectionEngine::with_defaults();
        let ctx = DetectionContext {
            initial_bytes: b"random data",
            src_port: 12345,
            dst_port: 9999,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        let result = engine.detect(&ctx);
        assert_eq!(result.protocol.0, ProtocolId::UNKNOWN);
        assert_eq!(result.confidence, 0.0);
        assert_eq!(result.method, DetectionMethod::Fallback);
    }

    #[test]
    fn test_engine_port_plus_magic() {
        // Port mapping (0.5) should be overridden by magic bytes (0.95)
        let engine = DetectionEngine::with_defaults();
        let preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
        let ctx = DetectionContext {
            initial_bytes: preface,
            src_port: 12345,
            dst_port: 50051,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        let result = engine.detect(&ctx);
        // Should pick the magic bytes detection (0.95) over port mapping (0.5)
        assert_eq!(result.confidence, 0.95);
        assert_eq!(result.method, DetectionMethod::MagicBytes);
    }

    #[test]
    fn test_engine_with_threshold() {
        let engine = DetectionEngine::with_defaults().with_threshold(0.7);
        let ctx = DetectionContext {
            initial_bytes: &[],
            src_port: 12345,
            dst_port: 50051,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        let result = engine.detect(&ctx);
        // Port mapping (0.5) is below threshold (0.7), should fall back to UNKNOWN
        assert_eq!(result.protocol.0, ProtocolId::UNKNOWN);
        assert_eq!(result.method, DetectionMethod::Fallback);
    }

    #[test]
    fn test_empty_engine() {
        let engine = DetectionEngine::new();
        let ctx = DetectionContext {
            initial_bytes: b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n",
            src_port: 12345,
            dst_port: 50051,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        let result = engine.detect(&ctx);
        assert_eq!(result.protocol.0, ProtocolId::UNKNOWN);
        assert_eq!(result.method, DetectionMethod::Fallback);
    }

    #[test]
    fn test_add_detector() {
        let mut engine = DetectionEngine::new();
        engine.add_detector(Box::new(GrpcDetector));

        let preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
        let ctx = DetectionContext {
            initial_bytes: preface,
            src_port: 12345,
            dst_port: 50051,
            transport: TransportLayer::Tcp,
            tls_decrypted: false,
        };

        let result = engine.detect(&ctx);
        assert_eq!(result.protocol.0, ProtocolId::GRPC);
        assert_eq!(result.confidence, 0.95);
    }
}
