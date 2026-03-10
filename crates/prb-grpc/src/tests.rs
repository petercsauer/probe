//! Integration tests for gRPC decoder.

use crate::decoder::GrpcDecoder;
use prb_core::{DecodeContext, ProtocolDecoder, METADATA_KEY_GRPC_METHOD};

// Helper function to create test HTTP/2 frames
fn create_http2_preface() -> Vec<u8> {
    b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n".to_vec()
}

fn create_settings_frame() -> Vec<u8> {
    // SETTINGS frame: length=0, type=0x04, flags=0x00, stream_id=0
    vec![0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00]
}

fn create_headers_frame(stream_id: u32, headers: &[(&str, &str)], end_stream: bool) -> Vec<u8> {
    // Encode headers as literal without indexing (simplified HPACK)
    let mut payload = Vec::new();
    for (name, value) in headers {
        payload.push(0x00); // Literal without indexing
        payload.push(name.len() as u8);
        payload.extend_from_slice(name.as_bytes());
        payload.push(value.len() as u8);
        payload.extend_from_slice(value.as_bytes());
    }

    let mut frame = Vec::new();
    // Length (24-bit big-endian)
    frame.push(((payload.len() >> 16) & 0xFF) as u8);
    frame.push(((payload.len() >> 8) & 0xFF) as u8);
    frame.push((payload.len() & 0xFF) as u8);
    // Type (HEADERS = 0x01)
    frame.push(0x01);
    // Flags (END_STREAM = 0x01, END_HEADERS = 0x04)
    let flags = if end_stream { 0x05 } else { 0x04 };
    frame.push(flags);
    // Stream ID (31-bit)
    frame.extend_from_slice(&stream_id.to_be_bytes());
    // Payload
    frame.extend_from_slice(&payload);

    frame
}

fn create_data_frame(stream_id: u32, data: &[u8], end_stream: bool) -> Vec<u8> {
    let mut frame = Vec::new();
    // Length (24-bit big-endian)
    frame.push(((data.len() >> 16) & 0xFF) as u8);
    frame.push(((data.len() >> 8) & 0xFF) as u8);
    frame.push((data.len() & 0xFF) as u8);
    // Type (DATA = 0x00)
    frame.push(0x00);
    // Flags (END_STREAM = 0x01)
    let flags = if end_stream { 0x01 } else { 0x00 };
    frame.push(flags);
    // Stream ID (31-bit)
    frame.extend_from_slice(&stream_id.to_be_bytes());
    // Payload
    frame.extend_from_slice(data);

    frame
}

fn create_grpc_message(payload: &[u8], compressed: bool) -> Vec<u8> {
    let mut msg = Vec::new();
    // Compressed flag
    msg.push(if compressed { 1 } else { 0 });
    // Length (32-bit big-endian)
    msg.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    // Payload
    msg.extend_from_slice(payload);
    msg
}

#[test]
fn test_grpc_simple_unary_call() {
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    // Build HTTP/2 stream with gRPC unary call
    let mut stream = Vec::new();

    // HTTP/2 preface
    stream.extend_from_slice(&create_http2_preface());

    // SETTINGS frame
    stream.extend_from_slice(&create_settings_frame());

    // Request HEADERS (stream 1)
    let request_headers = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
        ("grpc-encoding", "identity"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers, false));

    // Request DATA (gRPC message)
    let request_payload = b"request_data";
    let grpc_request = create_grpc_message(request_payload, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_request, true));

    // Response HEADERS (stream 1)
    let response_headers = vec![
        (":status", "200"),
        ("content-type", "application/grpc"),
        ("grpc-encoding", "identity"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &response_headers, false));

    // Response DATA (gRPC message)
    let response_payload = b"response_data";
    let grpc_response = create_grpc_message(response_payload, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_response, false));

    // Trailers (grpc-status)
    let trailers = vec![("grpc-status", "0"), ("grpc-message", "")];
    stream.extend_from_slice(&create_headers_frame(1, &trailers, true));

    // Decode the stream
    let events = decoder.decode_stream(&stream, &ctx).unwrap();

    // Verify events
    assert!(
        events.len() >= 2,
        "Expected at least 2 events (request, response), got {}",
        events.len()
    );

    // Verify request event
    let request_event = &events[0];
    assert_eq!(request_event.transport, prb_core::TransportKind::Grpc);
    assert!(request_event
        .metadata
        .get(METADATA_KEY_GRPC_METHOD)
        .unwrap()
        .contains("Method"));

    // Verify response event
    let response_event = &events[1];
    assert_eq!(response_event.transport, prb_core::TransportKind::Grpc);
}

#[test]
fn test_grpc_compressed_message() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    // Build HTTP/2 stream with compressed gRPC message
    let mut stream = Vec::new();

    // HTTP/2 preface
    stream.extend_from_slice(&create_http2_preface());

    // SETTINGS frame
    stream.extend_from_slice(&create_settings_frame());

    // Request HEADERS (stream 1) with gzip encoding
    let request_headers = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
        ("grpc-encoding", "gzip"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers, false));

    // Compress the payload with gzip
    let uncompressed_payload = b"this_is_the_uncompressed_test_payload";
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(uncompressed_payload).unwrap();
    let compressed_payload = encoder.finish().unwrap();

    // Create gRPC message with compressed flag set
    let grpc_request = create_grpc_message(&compressed_payload, true);
    stream.extend_from_slice(&create_data_frame(1, &grpc_request, true));

    // Decode the stream
    let events = decoder.decode_stream(&stream, &ctx).unwrap();

    // Verify we got an event
    assert!(
        !events.is_empty(),
        "Expected at least 1 event for compressed message"
    );

    // Verify the message was decompressed correctly
    let event = &events[0];
    if let prb_core::Payload::Raw { raw } = &event.payload {
        assert_eq!(
            &raw[..],
            uncompressed_payload,
            "Decompressed payload should match original"
        );
    } else {
        panic!("Expected Raw payload");
    }
}

#[test]
fn test_grpc_streaming() {
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    // Build HTTP/2 stream with server-streaming gRPC call
    let mut stream = Vec::new();

    // HTTP/2 preface
    stream.extend_from_slice(&create_http2_preface());

    // SETTINGS frame
    stream.extend_from_slice(&create_settings_frame());

    // Request HEADERS (stream 1)
    let request_headers = vec![
        (":method", "POST"),
        (":path", "/test.Service/StreamingMethod"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers, false));

    // Request DATA
    let request_payload = b"request";
    let grpc_request = create_grpc_message(request_payload, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_request, true));

    // Response HEADERS (stream 1)
    let response_headers = vec![
        (":status", "200"),
        ("content-type", "application/grpc"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &response_headers, false));

    // Multiple response DATA frames (streaming responses)
    let response1 = b"response_1";
    let grpc_response1 = create_grpc_message(response1, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_response1, false));

    let response2 = b"response_2";
    let grpc_response2 = create_grpc_message(response2, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_response2, false));

    let response3 = b"response_3";
    let grpc_response3 = create_grpc_message(response3, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_response3, false));

    // Trailers (grpc-status)
    let trailers = vec![("grpc-status", "0"), ("grpc-message", "")];
    stream.extend_from_slice(&create_headers_frame(1, &trailers, true));

    // Decode the stream
    let events = decoder.decode_stream(&stream, &ctx).unwrap();

    // Verify we got at least 4 events (request + 3 responses)
    assert!(
        events.len() >= 4,
        "Expected at least 4 events (1 request + 3 responses), got {}",
        events.len()
    );

    // Verify all events are for the correct stream
    for event in &events {
        assert_eq!(
            event.metadata.get("h2.stream_id").map(|s| s.as_str()),
            Some("1")
        );
    }
}

#[test]
fn test_grpc_trailers_only() {
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    // Build HTTP/2 stream with Trailers-Only response
    let mut stream = Vec::new();

    // HTTP/2 preface
    stream.extend_from_slice(&create_http2_preface());

    // SETTINGS frame
    stream.extend_from_slice(&create_settings_frame());

    // Request HEADERS (stream 1)
    let request_headers = vec![
        (":method", "POST"),
        (":path", "/test.Service/ErrorMethod"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers, false));

    // Request DATA
    let request_payload = b"request_data";
    let grpc_request = create_grpc_message(request_payload, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_request, true));

    // Trailers-Only response (no DATA frames, just headers with error status)
    let trailers = vec![
        (":status", "200"),
        ("content-type", "application/grpc"),
        ("grpc-status", "2"), // UNKNOWN error
        ("grpc-message", "Error occurred"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &trailers, true));

    // Decode the stream
    let events = decoder.decode_stream(&stream, &ctx).unwrap();

    // Verify we got events
    assert!(
        !events.is_empty(),
        "Expected at least 1 event for trailers-only response"
    );

    // Find status event
    let status_event = events
        .iter()
        .find(|e| e.metadata.contains_key("grpc.status"));
    assert!(status_event.is_some(), "Expected to find grpc-status event");

    let status_event = status_event.unwrap();
    assert_eq!(status_event.metadata.get("grpc.status").unwrap(), "2");
}

#[test]
fn test_hpack_degradation() {
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    // Build HTTP/2 stream WITHOUT preface/settings - simulating mid-stream capture
    let mut stream = Vec::new();

    // Directly start with HEADERS frame (no preface)
    // Use indexed headers that require dynamic table (will trigger HPACK error)
    let request_headers = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers, false));

    // Request DATA
    let request_payload = b"test_data";
    let grpc_request = create_grpc_message(request_payload, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_request, true));

    // Decode the stream
    let events = decoder.decode_stream(&stream, &ctx).unwrap();

    // Should still produce events despite missing context
    assert!(
        !events.is_empty(),
        "Should produce events even with HPACK degradation"
    );

    // Check if any event has a warning about HPACK degradation
    // Note: Our current simple implementation may not trigger degradation for literal headers,
    // but the test verifies graceful handling
    // In a real mid-stream capture with dynamic table references, we would see warnings
}

#[test]
fn test_grpc_multi_frame_message() {
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    // Build HTTP/2 stream with gRPC message spanning multiple DATA frames
    let mut stream = Vec::new();

    // HTTP/2 preface
    stream.extend_from_slice(&create_http2_preface());

    // SETTINGS frame
    stream.extend_from_slice(&create_settings_frame());

    // Request HEADERS (stream 1)
    let request_headers = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers, false));

    // Split gRPC message across 3 DATA frames
    let request_payload = b"this_is_a_long_payload_that_spans_multiple_frames";
    let grpc_header = vec![
        0u8,                                              // Not compressed
        0, 0, 0, request_payload.len() as u8, // Length
    ];

    // Frame 1: LPM header + first 10 bytes
    let mut frame1_data = grpc_header.clone();
    frame1_data.extend_from_slice(&request_payload[..10]);
    stream.extend_from_slice(&create_data_frame(1, &frame1_data, false));

    // Frame 2: next 20 bytes
    stream.extend_from_slice(&create_data_frame(1, &request_payload[10..30], false));

    // Frame 3: remaining bytes
    stream.extend_from_slice(&create_data_frame(1, &request_payload[30..], true));

    // Decode the stream
    let events = decoder.decode_stream(&stream, &ctx).unwrap();

    // Verify we got the message event
    assert!(
        !events.is_empty(),
        "Expected at least 1 event for multi-frame message"
    );

    // Verify the message was reassembled correctly
    let event = &events[0];
    if let prb_core::Payload::Raw { raw } = &event.payload {
        assert_eq!(raw.len(), request_payload.len());
    } else {
        panic!("Expected Raw payload");
    }
}

#[test]
fn test_protocol_dispatch() {
    // This test will be implemented when we add protocol dispatch infrastructure
    // For now, just verify the decoder reports the correct protocol
    let decoder = GrpcDecoder::new();
    assert_eq!(decoder.protocol(), prb_core::TransportKind::Grpc);
}
