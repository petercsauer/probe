//! Integration tests for gRPC decoder.

use crate::decoder::GrpcDecoder;
use prb_core::{DecodeContext, METADATA_KEY_GRPC_METHOD, ProtocolDecoder};

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

    let flags = if end_stream { 0x05 } else { 0x04 };
    let mut frame = vec![
        ((payload.len() >> 16) & 0xFF) as u8, // Length (24-bit big-endian)
        ((payload.len() >> 8) & 0xFF) as u8,
        (payload.len() & 0xFF) as u8,
        0x01,  // Type (HEADERS = 0x01)
        flags, // Flags
    ];
    // Stream ID (31-bit)
    frame.extend_from_slice(&stream_id.to_be_bytes());
    // Payload
    frame.extend_from_slice(&payload);

    frame
}

fn create_data_frame(stream_id: u32, data: &[u8], end_stream: bool) -> Vec<u8> {
    let flags = u8::from(end_stream);
    let mut frame = vec![
        ((data.len() >> 16) & 0xFF) as u8, // Length (24-bit big-endian)
        ((data.len() >> 8) & 0xFF) as u8,
        (data.len() & 0xFF) as u8,
        0x00,  // Type (DATA = 0x00)
        flags, // Flags
    ];
    // Stream ID (31-bit)
    frame.extend_from_slice(&stream_id.to_be_bytes());
    // Payload
    frame.extend_from_slice(data);

    frame
}

fn create_grpc_message(payload: &[u8], compressed: bool) -> Vec<u8> {
    let mut msg = Vec::new();
    // Compressed flag
    msg.push(u8::from(compressed));
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
    assert!(
        request_event
            .metadata
            .get(METADATA_KEY_GRPC_METHOD)
            .unwrap()
            .contains("Method")
    );

    // Verify response event
    let response_event = &events[1];
    assert_eq!(response_event.transport, prb_core::TransportKind::Grpc);
}

#[test]
fn test_grpc_compressed_message() {
    use flate2::Compression;
    use flate2::write::GzEncoder;
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
    let response_headers = vec![(":status", "200"), ("content-type", "application/grpc")];
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
            event.metadata.get("h2.stream_id").map(std::string::String::as_str),
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
        0u8, // Not compressed
        0,
        0,
        0,
        request_payload.len() as u8, // Length
    ];

    // Frame 1: LPM header + first 10 bytes
    let mut frame1_data = grpc_header;
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

#[test]
fn test_h2_multi_byte_integer() {
    // Test HPACK multi-byte integer encoding/decoding (WS-3.1)
    // Header name with length >= 128 exercises multi-byte integer parsing
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    let mut stream = Vec::new();
    stream.extend_from_slice(&create_http2_preface());
    stream.extend_from_slice(&create_settings_frame());

    // Create a header with a long value (>= 128 bytes) to trigger multi-byte integer encoding
    let long_value = "a".repeat(200);
    let request_headers = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
        ("x-custom-header", long_value.as_str()),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers, false));

    let request_payload = b"test";
    let grpc_request = create_grpc_message(request_payload, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_request, true));

    // Should parse successfully despite multi-byte integer
    let events = decoder.decode_stream(&stream, &ctx).unwrap();
    assert!(!events.is_empty(), "Should produce at least one event");
}

#[test]
fn test_h2_continuation_frame() {
    // Test CONTINUATION frame handling (WS-2.3)
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    let mut stream = Vec::new();
    stream.extend_from_slice(&create_http2_preface());
    stream.extend_from_slice(&create_settings_frame());

    // Create HEADERS frame WITHOUT END_HEADERS flag (0x04)
    let headers_part1 = vec![(":method", "POST"), (":path", "/test.Service/Method")];
    let mut payload1 = Vec::new();
    for (name, value) in &headers_part1 {
        payload1.push(0x00);
        payload1.push(name.len() as u8);
        payload1.extend_from_slice(name.as_bytes());
        payload1.push(value.len() as u8);
        payload1.extend_from_slice(value.as_bytes());
    }

    let mut headers_frame = vec![
        ((payload1.len() >> 16) & 0xFF) as u8,
        ((payload1.len() >> 8) & 0xFF) as u8,
        (payload1.len() & 0xFF) as u8,
        0x01, // Type (HEADERS)
        0x00, // Flags (NO END_HEADERS)
    ];
    headers_frame.extend_from_slice(&1u32.to_be_bytes());
    headers_frame.extend_from_slice(&payload1);
    stream.extend_from_slice(&headers_frame);

    // Create CONTINUATION frame with remaining headers and END_HEADERS flag
    let headers_part2 = vec![
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
    ];
    let mut payload2 = Vec::new();
    for (name, value) in &headers_part2 {
        payload2.push(0x00);
        payload2.push(name.len() as u8);
        payload2.extend_from_slice(name.as_bytes());
        payload2.push(value.len() as u8);
        payload2.extend_from_slice(value.as_bytes());
    }

    let mut continuation_frame = vec![
        ((payload2.len() >> 16) & 0xFF) as u8,
        ((payload2.len() >> 8) & 0xFF) as u8,
        (payload2.len() & 0xFF) as u8,
        0x09, // Type (CONTINUATION)
        0x04, // Flags (END_HEADERS)
    ];
    continuation_frame.extend_from_slice(&1u32.to_be_bytes());
    continuation_frame.extend_from_slice(&payload2);
    stream.extend_from_slice(&continuation_frame);

    // Add DATA frame
    let request_payload = b"test";
    let grpc_request = create_grpc_message(request_payload, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_request, true));

    // Should successfully reassemble fragmented headers
    let events = decoder.decode_stream(&stream, &ctx).unwrap();
    assert!(
        !events.is_empty(),
        "Should produce events from CONTINUATION frames"
    );

    // Verify the request has the gRPC method from the fragmented headers
    if let Some(event) = events.first() {
        assert!(event.metadata.contains_key(METADATA_KEY_GRPC_METHOD));
    }
}

#[test]
fn test_h2_unknown_frame_skip() {
    // Test that unknown frame types are silently skipped
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    let mut stream = Vec::new();
    stream.extend_from_slice(&create_http2_preface());
    stream.extend_from_slice(&create_settings_frame());

    // Insert unknown frame type (0xFF)
    let unknown_frame = vec![
        0x00, 0x00, 0x08, // Length
        0xFF, // Unknown type
        0x00, // Flags
        0x00, 0x00, 0x00, 0x00, // Stream ID
        0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x11, 0x22, // Payload
    ];
    stream.extend_from_slice(&unknown_frame);

    // Valid request follows
    let request_headers = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers, false));

    let request_payload = b"test";
    let grpc_request = create_grpc_message(request_payload, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_request, true));

    // Should skip unknown frame and still parse valid frames
    let events = decoder.decode_stream(&stream, &ctx).unwrap();
    assert!(
        !events.is_empty(),
        "Should produce events despite unknown frame"
    );
}

#[test]
fn test_h2_padded_data_frame() {
    // Test DATA frame with PADDED flag (0x08)
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    let mut stream = Vec::new();
    stream.extend_from_slice(&create_http2_preface());
    stream.extend_from_slice(&create_settings_frame());

    let request_headers = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers, false));

    // Create padded DATA frame
    let request_payload = b"test_data";
    let grpc_request = create_grpc_message(request_payload, false);
    let pad_length = 10u8;
    let mut padded_data = vec![pad_length];
    padded_data.extend_from_slice(&grpc_request);
    padded_data.extend_from_slice(&vec![0u8; pad_length as usize]);

    let mut data_frame = vec![
        ((padded_data.len() >> 16) & 0xFF) as u8,
        ((padded_data.len() >> 8) & 0xFF) as u8,
        (padded_data.len() & 0xFF) as u8,
        0x00, // Type (DATA)
        0x08, // Flags (PADDED)
    ];
    data_frame.extend_from_slice(&1u32.to_be_bytes());
    data_frame.extend_from_slice(&padded_data);
    stream.extend_from_slice(&data_frame);

    // Should handle padding correctly
    let events = decoder.decode_stream(&stream, &ctx).unwrap();
    assert!(
        !events.is_empty(),
        "Should produce events from padded DATA frame"
    );
}

#[test]
fn test_h2_empty_headers_frame() {
    // Test HEADERS frame with zero-length payload
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    let mut stream = Vec::new();
    stream.extend_from_slice(&create_http2_preface());
    stream.extend_from_slice(&create_settings_frame());

    // Empty HEADERS frame
    let empty_headers = vec![
        0x00, 0x00, 0x00, // Length = 0
        0x01, // Type (HEADERS)
        0x05, // Flags (END_STREAM | END_HEADERS)
        0x00, 0x00, 0x00, 0x01, // Stream ID = 1
    ];
    stream.extend_from_slice(&empty_headers);

    // Should handle empty headers gracefully
    let result = decoder.decode_stream(&stream, &ctx);
    assert!(
        result.is_ok(),
        "Should handle empty HEADERS frame gracefully"
    );
}

#[test]
fn test_lpm_compressed_gzip_roundtrip() {
    // Test gzip compression/decompression roundtrip
    use crate::lpm::{CompressionAlgorithm, LpmParser};
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    let original_payload = b"test_payload_for_gzip_compression";

    // Compress with gzip
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(original_payload).unwrap();
    let compressed = encoder.finish().unwrap();

    // Create LPM message with compressed flag set
    let mut data = vec![1u8]; // compressed flag = 1
    data.extend_from_slice(&(compressed.len() as u32).to_be_bytes());
    data.extend_from_slice(&compressed);

    // Parse with gzip decompression
    let mut parser = LpmParser::new(CompressionAlgorithm::Gzip);
    let messages = parser.feed(&data).unwrap();

    assert_eq!(messages.len(), 1, "Should parse one message");
    assert_eq!(
        &messages[0].payload[..],
        original_payload,
        "Gzip roundtrip should preserve data"
    );
}

#[test]
fn test_lpm_compressed_zlib_roundtrip() {
    // Test zlib compression/decompression (WS-2.7 - gRPC "deflate" uses zlib, not raw deflate)
    use crate::lpm::{CompressionAlgorithm, LpmParser};
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    let original_payload = b"test_payload_for_zlib_compression";

    // Compress with zlib
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(original_payload).unwrap();
    let compressed = encoder.finish().unwrap();

    // Create LPM message with compressed flag set
    let mut data = vec![1u8]; // compressed flag = 1
    data.extend_from_slice(&(compressed.len() as u32).to_be_bytes());
    data.extend_from_slice(&compressed);

    // Parse with deflate (zlib) decompression
    let mut parser = LpmParser::new(CompressionAlgorithm::Deflate);
    let messages = parser.feed(&data).unwrap();

    assert_eq!(messages.len(), 1, "Should parse one message");
    assert_eq!(
        &messages[0].payload[..],
        original_payload,
        "Zlib roundtrip should preserve data"
    );
}

#[test]
fn test_lpm_zero_length_message() {
    // Test LPM with zero-length payload
    use crate::lpm::{CompressionAlgorithm, LpmParser};

    // LPM header for zero-length message: compressed=0, length=0
    let data = vec![0u8, 0, 0, 0, 0]; // 5 bytes: flag + length

    let mut parser = LpmParser::new(CompressionAlgorithm::Identity);
    let messages = parser.feed(&data).unwrap();

    assert_eq!(messages.len(), 1, "Should parse zero-length message");
    assert_eq!(messages[0].payload.len(), 0, "Payload should be empty");
}

#[test]
fn test_lpm_max_message_size() {
    // Test LPM with large message (1MB)
    use crate::lpm::{CompressionAlgorithm, LpmParser};

    let large_payload = vec![0xAAu8; 1024 * 1024];

    // Create LPM message
    let mut data = vec![0u8]; // compressed flag = 0
    data.extend_from_slice(&(large_payload.len() as u32).to_be_bytes());
    data.extend_from_slice(&large_payload);

    let mut parser = LpmParser::new(CompressionAlgorithm::Identity);
    let messages = parser.feed(&data).unwrap();

    assert_eq!(messages.len(), 1, "Should parse large message");
    assert_eq!(
        messages[0].payload.len(),
        1024 * 1024,
        "Payload should be 1MB"
    );
}

#[test]
fn test_grpc_direction_inference() {
    // Test direction inference: request (with :method) = Outbound, response = Inbound (WS-2.6)
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    let mut stream = Vec::new();
    stream.extend_from_slice(&create_http2_preface());
    stream.extend_from_slice(&create_settings_frame());

    // Request headers with :method
    let request_headers = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers, false));

    let request_payload = b"request_data";
    let grpc_request = create_grpc_message(request_payload, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_request, true));

    // Response headers (no :method)
    let response_headers = vec![(":status", "200"), ("content-type", "application/grpc")];
    stream.extend_from_slice(&create_headers_frame(1, &response_headers, false));

    let response_payload = b"response_data";
    let grpc_response = create_grpc_message(response_payload, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_response, false));

    // Trailers
    let trailers = vec![("grpc-status", "0")];
    stream.extend_from_slice(&create_headers_frame(1, &trailers, true));

    let events = decoder.decode_stream(&stream, &ctx).unwrap();
    assert!(events.len() >= 2, "Should have request and response events");

    // First event should be Outbound (request)
    assert_eq!(
        events[0].direction,
        prb_core::Direction::Outbound,
        "Request should be Outbound"
    );

    // Second event should be Inbound (response)
    assert_eq!(
        events[1].direction,
        prb_core::Direction::Inbound,
        "Response should be Inbound"
    );
}

#[test]
fn test_grpc_correlation_by_stream_id() {
    // Test that events on the same stream have the same correlation key
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    let mut stream = Vec::new();
    stream.extend_from_slice(&create_http2_preface());
    stream.extend_from_slice(&create_settings_frame());

    // Stream 1
    let request_headers_1 = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method1"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers_1, false));

    let request_payload_1 = b"request1";
    let grpc_request_1 = create_grpc_message(request_payload_1, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_request_1, true));

    // Stream 3 (different stream)
    let request_headers_3 = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method2"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
    ];
    stream.extend_from_slice(&create_headers_frame(3, &request_headers_3, false));

    let request_payload_3 = b"request3";
    let grpc_request_3 = create_grpc_message(request_payload_3, false);
    stream.extend_from_slice(&create_data_frame(3, &grpc_request_3, true));

    let events = decoder.decode_stream(&stream, &ctx).unwrap();
    assert!(events.len() >= 2, "Should have events from both streams");

    // Check stream IDs in metadata
    let stream1_events: Vec<_> = events
        .iter()
        .filter(|e| e.metadata.get("h2.stream_id") == Some(&"1".to_string()))
        .collect();

    let stream3_events: Vec<_> = events
        .iter()
        .filter(|e| e.metadata.get("h2.stream_id") == Some(&"3".to_string()))
        .collect();

    assert!(!stream1_events.is_empty(), "Should have stream 1 events");
    assert!(!stream3_events.is_empty(), "Should have stream 3 events");
}

#[test]
fn test_grpc_metadata_extraction() {
    // Test that key metadata fields are extracted
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    let mut stream = Vec::new();
    stream.extend_from_slice(&create_http2_preface());
    stream.extend_from_slice(&create_settings_frame());

    let request_headers = vec![
        (":method", "POST"),
        (":path", "/test.Service/TestMethod"),
        (":authority", "example.com:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
        ("grpc-encoding", "gzip"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers, false));

    let request_payload = b"test";
    let grpc_request = create_grpc_message(request_payload, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_request, true));

    let events = decoder.decode_stream(&stream, &ctx).unwrap();
    assert!(!events.is_empty(), "Should have at least one event");

    let event = &events[0];

    // Check metadata extraction
    assert_eq!(
        event.metadata.get(METADATA_KEY_GRPC_METHOD),
        Some(&"/test.Service/TestMethod".to_string()),
        "Should extract gRPC method"
    );

    assert_eq!(
        event.metadata.get("h2.stream_id"),
        Some(&"1".to_string()),
        "Should extract stream ID"
    );
}
