//! Integration tests for DecoderRegistry with real decoder implementations.

use prb_core::{DecodeContext, ProtocolDecoder};
use prb_detect::{DecoderFactory, DecoderRegistry, ProtocolId, StreamKey, TransportLayer};
use prb_grpc::GrpcDecoder;
use prb_zmq::ZmqDecoder;
use prb_dds::DdsDecoder;

// Factory implementations for real decoders

struct GrpcDecoderFactory;

impl DecoderFactory for GrpcDecoderFactory {
    fn protocol_id(&self) -> ProtocolId {
        ProtocolId::new(ProtocolId::GRPC)
    }

    fn create_decoder(&self) -> Box<dyn ProtocolDecoder> {
        Box::new(GrpcDecoder::new())
    }
}

struct ZmqDecoderFactory;

impl DecoderFactory for ZmqDecoderFactory {
    fn protocol_id(&self) -> ProtocolId {
        ProtocolId::new(ProtocolId::ZMTP)
    }

    fn create_decoder(&self) -> Box<dyn ProtocolDecoder> {
        Box::new(ZmqDecoder::new())
    }
}

struct DdsDecoderFactory;

impl DecoderFactory for DdsDecoderFactory {
    fn protocol_id(&self) -> ProtocolId {
        ProtocolId::new(ProtocolId::RTPS)
    }

    fn create_decoder(&self) -> Box<dyn ProtocolDecoder> {
        Box::new(DdsDecoder::new())
    }
}

#[test]
fn test_process_stream_grpc() {
    let mut registry = DecoderRegistry::new();
    registry.register_factory(Box::new(GrpcDecoderFactory));

    // HTTP/2 connection preface (gRPC)
    let preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
    let stream_key = StreamKey::new(
        "192.168.1.1:12345".to_string(),
        "192.168.1.2:50051".to_string(),
        TransportLayer::Tcp,
    );

    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:12345")
        .with_dst_addr("192.168.1.2:50051");

    // Process the stream - should detect gRPC and route to GrpcDecoder
    let result = registry.process_stream(stream_key.clone(), preface, &ctx);

    // Should succeed (even if no events are generated from just the preface)
    assert!(result.is_ok());

    // Should have created a decoder for this stream
    assert_eq!(registry.active_decoder_count(), 1);
}

#[test]
fn test_process_stream_zmtp() {
    let mut registry = DecoderRegistry::new();
    registry.register_factory(Box::new(ZmqDecoderFactory));

    // ZMTP 3.0 greeting
    let greeting = [
        0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0x7F, // signature
        0x03, 0x00, // version 3.0
        b'N', b'U', b'L', b'L', 0, 0, 0, 0, 0, 0, // mechanism (NULL)
        0x00, // as_server = false
        0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // filler
    ];

    let stream_key = StreamKey::new(
        "10.0.0.1:5555".to_string(),
        "10.0.0.2:5555".to_string(),
        TransportLayer::Tcp,
    );

    let ctx = DecodeContext::new()
        .with_src_addr("10.0.0.1:5555")
        .with_dst_addr("10.0.0.2:5555");

    // Process the stream - should detect ZMTP and route to ZmqDecoder
    let result = registry.process_stream(stream_key.clone(), &greeting, &ctx);

    assert!(result.is_ok());
    assert_eq!(registry.active_decoder_count(), 1);
}

#[test]
fn test_process_datagram_rtps() {
    let mut registry = DecoderRegistry::new();
    registry.register_factory(Box::new(DdsDecoderFactory));

    // RTPS message header
    let rtps_header = b"RTPS\x02\x03\x00\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c";

    let stream_key = StreamKey::new(
        "192.168.1.100:12345".to_string(),
        "192.168.1.200:7400".to_string(),
        TransportLayer::Udp,
    );

    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.100:12345")
        .with_dst_addr("192.168.1.200:7400");

    // Process the datagram - should detect RTPS and route to DdsDecoder
    let result = registry.process_datagram(stream_key.clone(), rtps_header, &ctx);

    assert!(result.is_ok());
    assert_eq!(registry.active_decoder_count(), 1);
}

#[test]
fn test_unknown_protocol_fallback() {
    let mut registry = DecoderRegistry::new();
    // No factories registered

    let random_data = b"this is not a known protocol";
    let stream_key = StreamKey::new(
        "10.0.0.1:9999".to_string(),
        "10.0.0.2:8888".to_string(),
        TransportLayer::Tcp,
    );

    let ctx = DecodeContext::new()
        .with_src_addr("10.0.0.1:9999")
        .with_dst_addr("10.0.0.2:8888");

    // Process the stream - should detect as UNKNOWN and fail to find decoder
    let result = registry.process_stream(stream_key, random_data, &ctx);

    // Should fail because no decoder factory is registered for UNKNOWN
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No decoder factory"));
}

#[test]
fn test_user_override_bypasses_detection() {
    let mut registry = DecoderRegistry::new();
    registry.register_factory(Box::new(GrpcDecoderFactory));
    registry.register_factory(Box::new(ZmqDecoderFactory));

    // Random data that wouldn't normally detect as anything
    let random_data = b"random bytes that don't match any protocol";
    let stream_key = StreamKey::new(
        "10.0.0.1:8080".to_string(),
        "10.0.0.2:9090".to_string(),
        TransportLayer::Tcp,
    );

    // Set user override to force gRPC decoder
    registry.set_override(stream_key.clone(), ProtocolId::new(ProtocolId::GRPC));

    let ctx = DecodeContext::new()
        .with_src_addr("10.0.0.1:8080")
        .with_dst_addr("10.0.0.2:9090");

    // Process the stream - should use gRPC decoder due to override
    let result = registry.process_stream(stream_key.clone(), random_data, &ctx);

    // Should succeed (gRPC decoder will just not emit events for invalid data)
    assert!(result.is_ok());
    assert_eq!(registry.active_decoder_count(), 1);
}

#[test]
fn test_multiple_streams_separate_decoders() {
    let mut registry = DecoderRegistry::new();
    registry.register_factory(Box::new(GrpcDecoderFactory));
    registry.register_factory(Box::new(ZmqDecoderFactory));

    // Stream 1: gRPC
    let grpc_preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
    let stream1 = StreamKey::new(
        "192.168.1.1:12345".to_string(),
        "192.168.1.2:50051".to_string(),
        TransportLayer::Tcp,
    );
    let ctx1 = DecodeContext::new()
        .with_src_addr("192.168.1.1:12345")
        .with_dst_addr("192.168.1.2:50051");

    // Stream 2: ZMTP
    let zmtp_greeting = [
        0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0x7F, 0x03, 0x00,
        b'N', b'U', b'L', b'L', 0, 0, 0, 0, 0, 0,
        0x00,
        0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let stream2 = StreamKey::new(
        "10.0.0.1:5555".to_string(),
        "10.0.0.2:5555".to_string(),
        TransportLayer::Tcp,
    );
    let ctx2 = DecodeContext::new()
        .with_src_addr("10.0.0.1:5555")
        .with_dst_addr("10.0.0.2:5555");

    // Process both streams
    let result1 = registry.process_stream(stream1, grpc_preface, &ctx1);
    let result2 = registry.process_stream(stream2, &zmtp_greeting, &ctx2);

    assert!(result1.is_ok());
    assert!(result2.is_ok());

    // Should have two separate decoder instances
    assert_eq!(registry.active_decoder_count(), 2);
}

#[test]
fn test_same_stream_reuses_decoder() {
    let mut registry = DecoderRegistry::new();
    registry.register_factory(Box::new(GrpcDecoderFactory));

    let stream_key = StreamKey::new(
        "192.168.1.1:12345".to_string(),
        "192.168.1.2:50051".to_string(),
        TransportLayer::Tcp,
    );

    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:12345")
        .with_dst_addr("192.168.1.2:50051");

    // Process same stream multiple times
    let data1 = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
    let data2 = b"some more data";
    let data3 = b"even more data";

    let _ = registry.process_stream(stream_key.clone(), data1, &ctx);
    let _ = registry.process_stream(stream_key.clone(), data2, &ctx);
    let _ = registry.process_stream(stream_key.clone(), data3, &ctx);

    // Should still only have one decoder instance for this stream
    assert_eq!(registry.active_decoder_count(), 1);
}

#[test]
fn test_clear_decoders() {
    let mut registry = DecoderRegistry::new();
    registry.register_factory(Box::new(GrpcDecoderFactory));
    registry.register_factory(Box::new(ZmqDecoderFactory));

    // Create two streams
    let stream1 = StreamKey::new(
        "192.168.1.1:12345".to_string(),
        "192.168.1.2:50051".to_string(),
        TransportLayer::Tcp,
    );
    let stream2 = StreamKey::new(
        "10.0.0.1:5555".to_string(),
        "10.0.0.2:5555".to_string(),
        TransportLayer::Tcp,
    );

    registry.set_override(stream1.clone(), ProtocolId::new(ProtocolId::GRPC));
    registry.set_override(stream2.clone(), ProtocolId::new(ProtocolId::ZMTP));

    let ctx1 = DecodeContext::new()
        .with_src_addr("192.168.1.1:12345")
        .with_dst_addr("192.168.1.2:50051");
    let ctx2 = DecodeContext::new()
        .with_src_addr("10.0.0.1:5555")
        .with_dst_addr("10.0.0.2:5555");

    let _ = registry.process_stream(stream1, b"data1", &ctx1);
    let _ = registry.process_stream(stream2, b"data2", &ctx2);

    assert_eq!(registry.active_decoder_count(), 2);

    // Clear all decoders
    registry.clear_decoders();
    assert_eq!(registry.active_decoder_count(), 0);
}
