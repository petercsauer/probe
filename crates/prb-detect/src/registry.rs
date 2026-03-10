//! Decoder registry and dispatch layer.
//!
//! This module provides the central coordination point for protocol detection
//! and decoding, managing detector engines and decoder instances per stream.

use crate::engine::DetectionEngine;
use crate::types::{DetectionContext, ProtocolId, TransportLayer};
use prb_core::{CoreError, DebugEvent, DecodeContext, ProtocolDecoder};
use std::collections::HashMap;

/// Unique identifier for a network stream.
///
/// Identifies a unidirectional or bidirectional communication channel
/// by its network endpoints and transport protocol.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamKey {
    /// Source address (IP:port or just IP for datagram protocols).
    pub src_addr: String,
    /// Destination address.
    pub dst_addr: String,
    /// Transport protocol (tcp or udp).
    pub transport: TransportLayer,
}

impl StreamKey {
    /// Create a new stream key.
    pub fn new(src_addr: String, dst_addr: String, transport: TransportLayer) -> Self {
        Self {
            src_addr,
            dst_addr,
            transport,
        }
    }

    /// Create a stream key from DecodeContext.
    pub fn from_decode_context(ctx: &DecodeContext, transport: TransportLayer) -> Option<Self> {
        let src_addr = ctx.src_addr.clone()?;
        let dst_addr = ctx.dst_addr.clone()?;
        Some(Self::new(src_addr, dst_addr, transport))
    }
}

/// Factory for creating decoder instances.
///
/// Decoders are stateful (e.g., HTTP/2 frame reassembly, ZMTP handshake state),
/// so each stream needs its own decoder instance.
pub trait DecoderFactory: Send + Sync {
    /// Returns the protocol ID this factory creates decoders for.
    fn protocol_id(&self) -> ProtocolId;

    /// Create a new decoder instance.
    fn create_decoder(&self) -> Box<dyn ProtocolDecoder>;
}

/// Central registry for protocol detection and decoding.
///
/// The registry:
/// - Owns the detection engine and decoder factories
/// - Maintains active decoder instances per stream
/// - Routes detected streams to the appropriate decoder
/// - Supports user overrides to bypass detection
pub struct DecoderRegistry {
    /// Detection engine for identifying protocols.
    detection_engine: DetectionEngine,
    /// Decoder factories keyed by protocol ID.
    factories: HashMap<String, Box<dyn DecoderFactory>>,
    /// Active decoders per stream.
    active_decoders: HashMap<StreamKey, Box<dyn ProtocolDecoder>>,
    /// User-specified protocol overrides (bypasses detection).
    user_overrides: HashMap<StreamKey, ProtocolId>,
}

impl DecoderRegistry {
    /// Create a new registry with default detection engine.
    pub fn new() -> Self {
        Self {
            detection_engine: DetectionEngine::with_defaults(),
            factories: HashMap::new(),
            active_decoders: HashMap::new(),
            user_overrides: HashMap::new(),
        }
    }

    /// Create a registry with a custom detection engine.
    pub fn with_engine(engine: DetectionEngine) -> Self {
        Self {
            detection_engine: engine,
            factories: HashMap::new(),
            active_decoders: HashMap::new(),
            user_overrides: HashMap::new(),
        }
    }

    /// Register a decoder factory.
    pub fn register_factory(&mut self, factory: Box<dyn DecoderFactory>) {
        let protocol_id = factory.protocol_id().0.clone();
        tracing::debug!(protocol = %protocol_id, "Registering decoder factory");
        self.factories.insert(protocol_id, factory);
    }

    /// Set a user override for a specific stream.
    ///
    /// When set, detection will be bypassed and the specified protocol
    /// decoder will be used directly.
    pub fn set_override(&mut self, key: StreamKey, protocol: ProtocolId) {
        tracing::debug!(
            stream = ?key,
            protocol = %protocol.0,
            "Setting user protocol override"
        );
        self.user_overrides.insert(key, protocol);
    }

    /// Process a TCP stream, detecting protocol and decoding messages.
    ///
    /// This method:
    /// 1. Checks for user overrides
    /// 2. Runs protocol detection (if no override)
    /// 3. Looks up or creates decoder for the stream
    /// 4. Decodes the data
    pub fn process_stream(
        &mut self,
        stream_key: StreamKey,
        data: &[u8],
        ctx: &DecodeContext,
    ) -> Result<Vec<DebugEvent>, CoreError> {
        // Check for user override first
        let protocol = if let Some(override_protocol) = self.user_overrides.get(&stream_key) {
            tracing::debug!(
                stream = ?stream_key,
                protocol = %override_protocol.0,
                "Using user override"
            );
            override_protocol.clone()
        } else {
            // Run detection
            let detection_ctx = DetectionContext {
                initial_bytes: &data[..data.len().min(256)],
                src_port: Self::extract_port(&stream_key.src_addr),
                dst_port: Self::extract_port(&stream_key.dst_addr),
                transport: stream_key.transport,
                tls_decrypted: false, // TODO: wire through TLS state
            };

            let detection_result = self.detection_engine.detect(&detection_ctx);
            tracing::debug!(
                stream = ?stream_key,
                protocol = %detection_result.protocol.0,
                confidence = detection_result.confidence,
                method = ?detection_result.method,
                "Detected protocol"
            );

            detection_result.protocol
        };

        // Get or create decoder for this stream
        let decoder = self.get_or_create_decoder(&stream_key, &protocol)?;

        // Decode the stream
        decoder.decode_stream(data, ctx)
    }

    /// Process a UDP datagram, detecting protocol and decoding.
    ///
    /// Unlike TCP streams, UDP datagrams are typically stateless, but we
    /// still maintain per-connection decoders for protocols like DDS that
    /// track discovery state.
    pub fn process_datagram(
        &mut self,
        stream_key: StreamKey,
        data: &[u8],
        ctx: &DecodeContext,
    ) -> Result<Vec<DebugEvent>, CoreError> {
        // Check for user override first
        let protocol = if let Some(override_protocol) = self.user_overrides.get(&stream_key) {
            tracing::debug!(
                stream = ?stream_key,
                protocol = %override_protocol.0,
                "Using user override"
            );
            override_protocol.clone()
        } else {
            // Run detection
            let detection_ctx = DetectionContext {
                initial_bytes: &data[..data.len().min(256)],
                src_port: Self::extract_port(&stream_key.src_addr),
                dst_port: Self::extract_port(&stream_key.dst_addr),
                transport: stream_key.transport,
                tls_decrypted: false,
            };

            let detection_result = self.detection_engine.detect(&detection_ctx);
            tracing::debug!(
                stream = ?stream_key,
                protocol = %detection_result.protocol.0,
                confidence = detection_result.confidence,
                method = ?detection_result.method,
                "Detected protocol"
            );

            detection_result.protocol
        };

        // Get or create decoder for this datagram stream
        let decoder = self.get_or_create_decoder(&stream_key, &protocol)?;

        // Decode the datagram
        decoder.decode_stream(data, ctx)
    }

    /// Get existing decoder or create a new one for the stream.
    fn get_or_create_decoder(
        &mut self,
        stream_key: &StreamKey,
        protocol: &ProtocolId,
    ) -> Result<&mut Box<dyn ProtocolDecoder>, CoreError> {
        // Check if we already have an active decoder for this stream
        if !self.active_decoders.contains_key(stream_key) {
            // Need to create a new decoder
            let factory = self.factories.get(&protocol.0).ok_or_else(|| {
                CoreError::PayloadDecode(format!(
                    "No decoder factory registered for protocol: {}",
                    protocol.0
                ))
            })?;

            let decoder = factory.create_decoder();
            tracing::debug!(
                stream = ?stream_key,
                protocol = %protocol.0,
                "Created new decoder instance"
            );
            self.active_decoders.insert(stream_key.clone(), decoder);
        }

        // Return mutable reference to the decoder
        Ok(self
            .active_decoders
            .get_mut(stream_key)
            .expect("Decoder must exist after insert"))
    }

    /// Extract port number from address string (IP:port format).
    fn extract_port(addr: &str) -> u16 {
        addr.split(':')
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }

    /// Clear all active decoders (useful for testing or reset).
    pub fn clear_decoders(&mut self) {
        self.active_decoders.clear();
    }

    /// Get the number of active decoder instances.
    pub fn active_decoder_count(&self) -> usize {
        self.active_decoders.len()
    }
}

impl Default for DecoderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TransportLayer;
    use prb_core::{DecodeContext, TransportKind};
    use std::hash::{Hash, Hasher};

    #[test]
    fn test_stream_key_equality() {
        let key1 = StreamKey::new(
            "192.168.1.1:12345".to_string(),
            "192.168.1.2:50051".to_string(),
            TransportLayer::Tcp,
        );
        let key2 = StreamKey::new(
            "192.168.1.1:12345".to_string(),
            "192.168.1.2:50051".to_string(),
            TransportLayer::Tcp,
        );
        let key3 = StreamKey::new(
            "192.168.1.1:12346".to_string(),
            "192.168.1.2:50051".to_string(),
            TransportLayer::Tcp,
        );

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_stream_key_hash() {
        use std::collections::hash_map::DefaultHasher;

        let key1 = StreamKey::new(
            "192.168.1.1:12345".to_string(),
            "192.168.1.2:50051".to_string(),
            TransportLayer::Tcp,
        );
        let key2 = StreamKey::new(
            "192.168.1.1:12345".to_string(),
            "192.168.1.2:50051".to_string(),
            TransportLayer::Tcp,
        );

        let mut hasher1 = DefaultHasher::new();
        key1.hash(&mut hasher1);
        let hash1 = hasher1.finish();

        let mut hasher2 = DefaultHasher::new();
        key2.hash(&mut hasher2);
        let hash2 = hasher2.finish();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_stream_key_from_decode_context() {
        let ctx = DecodeContext::new()
            .with_src_addr("10.0.0.1:8080")
            .with_dst_addr("10.0.0.2:9090");

        let key = StreamKey::from_decode_context(&ctx, TransportLayer::Tcp);
        assert!(key.is_some());

        let key = key.unwrap();
        assert_eq!(key.src_addr, "10.0.0.1:8080");
        assert_eq!(key.dst_addr, "10.0.0.2:9090");
        assert_eq!(key.transport, TransportLayer::Tcp);
    }

    #[test]
    fn test_extract_port() {
        assert_eq!(DecoderRegistry::extract_port("192.168.1.1:8080"), 8080);
        assert_eq!(DecoderRegistry::extract_port("10.0.0.1:50051"), 50051);
        assert_eq!(DecoderRegistry::extract_port("invalid"), 0);
        assert_eq!(DecoderRegistry::extract_port("192.168.1.1"), 0);
    }

    // Mock decoder for testing
    struct MockDecoder {
        protocol: TransportKind,
    }

    impl ProtocolDecoder for MockDecoder {
        fn protocol(&self) -> TransportKind {
            self.protocol
        }

        fn decode_stream(
            &mut self,
            _data: &[u8],
            _ctx: &DecodeContext,
        ) -> Result<Vec<DebugEvent>, CoreError> {
            Ok(vec![])
        }
    }

    // Mock factory for testing
    struct MockFactory {
        protocol_id: ProtocolId,
        transport: TransportKind,
    }

    impl DecoderFactory for MockFactory {
        fn protocol_id(&self) -> ProtocolId {
            self.protocol_id.clone()
        }

        fn create_decoder(&self) -> Box<dyn ProtocolDecoder> {
            Box::new(MockDecoder {
                protocol: self.transport,
            })
        }
    }

    #[test]
    fn test_registry_register_factory() {
        let mut registry = DecoderRegistry::new();
        let factory = MockFactory {
            protocol_id: ProtocolId::new(ProtocolId::GRPC),
            transport: TransportKind::Grpc,
        };

        registry.register_factory(Box::new(factory));
        assert_eq!(registry.factories.len(), 1);
        assert!(registry.factories.contains_key(ProtocolId::GRPC));
    }

    #[test]
    fn test_registry_user_override() {
        let mut registry = DecoderRegistry::new();
        let stream_key = StreamKey::new(
            "10.0.0.1:8080".to_string(),
            "10.0.0.2:9090".to_string(),
            TransportLayer::Tcp,
        );
        let protocol = ProtocolId::new(ProtocolId::GRPC);

        registry.set_override(stream_key.clone(), protocol.clone());
        assert!(registry.user_overrides.contains_key(&stream_key));
        assert_eq!(
            registry.user_overrides.get(&stream_key).unwrap().0,
            protocol.0
        );
    }

    #[test]
    fn test_registry_clear_decoders() {
        let mut registry = DecoderRegistry::new();

        // Register a factory
        let factory = MockFactory {
            protocol_id: ProtocolId::new(ProtocolId::GRPC),
            transport: TransportKind::Grpc,
        };
        registry.register_factory(Box::new(factory));

        // Create a stream key and process some data
        let stream_key = StreamKey::new(
            "10.0.0.1:8080".to_string(),
            "10.0.0.2:50051".to_string(),
            TransportLayer::Tcp,
        );

        // Set override so detection doesn't fail
        registry.set_override(stream_key.clone(), ProtocolId::new(ProtocolId::GRPC));

        let ctx = DecodeContext::new()
            .with_src_addr("10.0.0.1:8080")
            .with_dst_addr("10.0.0.2:50051");

        let _ = registry.process_stream(stream_key, b"test data", &ctx);

        assert_eq!(registry.active_decoder_count(), 1);

        registry.clear_decoders();
        assert_eq!(registry.active_decoder_count(), 0);
    }
}
