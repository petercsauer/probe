//! Decoder factory implementations for built-in protocol decoders.
//!
//! This module provides factory implementations that create decoder instances
//! for gRPC, ZMTP, and DDS/RTPS protocols. Factories are registered with the
//! `DecoderRegistry` to enable protocol detection and decoding.

#[cfg(feature = "builtin-decoders")]
use prb_core::ProtocolDecoder;
#[cfg(feature = "builtin-decoders")]
use prb_detect::{DecoderFactory, ProtocolId};

/// Factory for creating gRPC decoder instances.
#[cfg(feature = "builtin-decoders")]
pub struct GrpcDecoderFactory;

#[cfg(feature = "builtin-decoders")]
impl DecoderFactory for GrpcDecoderFactory {
    fn protocol_id(&self) -> ProtocolId {
        ProtocolId::new(ProtocolId::GRPC)
    }

    fn create_decoder(&self) -> Box<dyn ProtocolDecoder> {
        Box::new(prb_grpc::GrpcDecoder::new())
    }
}

/// Factory for creating ZMTP decoder instances.
#[cfg(feature = "builtin-decoders")]
pub struct ZmqDecoderFactory;

#[cfg(feature = "builtin-decoders")]
impl DecoderFactory for ZmqDecoderFactory {
    fn protocol_id(&self) -> ProtocolId {
        ProtocolId::new(ProtocolId::ZMTP)
    }

    fn create_decoder(&self) -> Box<dyn ProtocolDecoder> {
        Box::new(prb_zmq::ZmqDecoder::new())
    }
}

/// Factory for creating DDS/RTPS decoder instances.
#[cfg(feature = "builtin-decoders")]
pub struct DdsDecoderFactory;

#[cfg(feature = "builtin-decoders")]
impl DecoderFactory for DdsDecoderFactory {
    fn protocol_id(&self) -> ProtocolId {
        ProtocolId::new(ProtocolId::RTPS)
    }

    fn create_decoder(&self) -> Box<dyn ProtocolDecoder> {
        Box::new(prb_dds::DdsDecoder::new())
    }
}

/// Creates a DecoderRegistry with all built-in decoder factories registered.
///
/// This is a convenience function for the common case of using all built-in decoders.
#[cfg(feature = "builtin-decoders")]
pub fn create_registry_with_builtins() -> prb_detect::DecoderRegistry {
    let mut registry = prb_detect::DecoderRegistry::new();
    registry.register_factory(Box::new(GrpcDecoderFactory));
    registry.register_factory(Box::new(ZmqDecoderFactory));
    registry.register_factory(Box::new(DdsDecoderFactory));
    registry
}

/// Creates an empty `DecoderRegistry` when builtin-decoders feature is disabled.
#[cfg(not(feature = "builtin-decoders"))]
#[must_use] 
pub fn create_registry_with_builtins() -> prb_detect::DecoderRegistry {
    prb_detect::DecoderRegistry::new()
}
