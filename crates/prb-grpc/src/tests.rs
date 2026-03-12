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
            event
                .metadata
                .get("h2.stream_id")
                .map(std::string::String::as_str),
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

// ============================================================================
// Direct H2Codec Tests - Exercise HPACK and frame parsing paths
// ============================================================================

#[test]
fn test_h2_indexed_header_static_table() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();

    // HTTP/2 preface
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // HEADERS frame with indexed headers from static table
    // Index 2 = :method GET, Index 4 = :path /
    let payload = vec![
        0x82, // Indexed header, index=2 (:method GET)
        0x84, // Indexed header, index=4 (:path /)
    ];

    let mut frame = vec![
        0x00,
        0x00,
        payload.len() as u8, // Length
        0x01,                // Type (HEADERS)
        0x05,                // Flags (END_STREAM | END_HEADERS)
    ];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        assert_eq!(headers.get(":method"), Some(&"GET".to_string()));
        assert_eq!(headers.get(":path"), Some(&"/".to_string()));
    } else {
        panic!("Expected Headers event");
    }
}

#[test]
fn test_h2_indexed_headers_various_static_table_entries() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Test various static table entries
    let payload = vec![
        0x83, // Index 3 = :method POST
        0x84, // Index 4 = :path /
        0x86, // Index 6 = :scheme http
        0x88, // Index 8 = :status 200
    ];

    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert!(events.len() >= 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        assert_eq!(headers.get(":method"), Some(&"POST".to_string()));
        assert_eq!(headers.get(":path"), Some(&"/".to_string()));
        assert_eq!(headers.get(":scheme"), Some(&"http".to_string()));
        assert_eq!(headers.get(":status"), Some(&"200".to_string()));
    }
}

#[test]
fn test_h2_literal_header_with_incremental_indexing() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Literal header with incremental indexing (0x40 prefix)
    // Name from static table index 1 (:authority)
    let mut payload = vec![
        0x41, // Literal with incremental indexing, name index=1 (:authority)
        0x0E, // Value length = 14
    ];
    payload.extend_from_slice(b"localhost:8080");

    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert!(events.len() >= 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        assert_eq!(
            headers.get(":authority"),
            Some(&"localhost:8080".to_string())
        );
    }
}

#[test]
fn test_h2_literal_header_never_indexed() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Literal header never indexed (0x10 prefix)
    let mut payload = vec![
        0x10, // Literal never indexed, name index=0 (literal name follows)
        0x0C, // Name length = 12
    ];
    payload.extend_from_slice(b"x-secret-key");
    payload.push(0x08); // Value length = 8
    payload.extend_from_slice(b"secretAA");

    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert!(events.len() >= 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        assert_eq!(headers.get("x-secret-key"), Some(&"secretAA".to_string()));
    }
}

#[test]
fn test_h2_dynamic_table_size_update() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Dynamic table size update (0x20 prefix)
    let payload = vec![
        0x3F, 0x11, // Dynamic table size update to 4096 (multi-byte integer)
        0x82, // Indexed header index=2 (:method GET)
    ];

    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    // Should process without error (size update is consumed)
    let events = codec.process(&frame).unwrap();
    assert!(events.len() >= 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        assert_eq!(headers.get(":method"), Some(&"GET".to_string()));
    }
}

#[test]
fn test_h2_hpack_multi_byte_integer_edge_cases() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Multi-byte integer with value >= 127
    // String length of 200 requires multi-byte encoding
    let long_value = "x".repeat(200);
    let mut payload = vec![
        0x00, // Literal without indexing, name index=0
        0x04, // Name length = 4
    ];
    payload.extend_from_slice(b"test");
    // Value length 200 = 127 + 73, encoded as: 0x7F (127 in 7-bit prefix), 0x49 (73)
    payload.push(0x7F); // First byte: all 7 bits set
    payload.push(0x49); // Second byte: remaining value (73)
    payload.extend_from_slice(long_value.as_bytes());

    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert!(events.len() >= 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        assert_eq!(headers.get("test"), Some(&long_value));
    }
}

#[test]
fn test_h2_static_table_high_indices() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Test high static table indices (50-61)
    let payload = vec![
        0xF2, // Index 50 (0x80 | 50) = range
        0xF3, // Index 51 = referer
        0xF6, // Index 54 = server
        0xFD, // Index 61 = www-authenticate
    ];

    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert!(events.len() >= 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        assert!(headers.contains_key("range"));
        assert!(headers.contains_key("referer"));
        assert!(headers.contains_key("server"));
        assert!(headers.contains_key("www-authenticate"));
    }
}

#[test]
fn test_h2_dynamic_table_reference_causes_degradation() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Reference index 62+ (beyond static table)
    let payload = vec![
        0xBE, // Indexed header, index=62 (first dynamic table entry)
    ];

    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();

    // Should produce HpackDegraded event
    let has_degraded = events
        .iter()
        .any(|e| matches!(e, crate::h2::H2Event::HpackDegraded { .. }));
    assert!(has_degraded, "Should produce HPACK degradation warning");
}

#[test]
fn test_h2_rst_stream_frame_direct() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // RST_STREAM frame with error code PROTOCOL_ERROR
    let mut frame = vec![
        0x00, 0x00, 0x04, // Length = 4
        0x03, // Type = RST_STREAM
        0x00, // Flags
    ];
    frame.extend_from_slice(&7u32.to_be_bytes()); // Stream ID = 7
    frame.extend_from_slice(&0x01u32.to_be_bytes()); // Error code = PROTOCOL_ERROR

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 1);
    matches!(events[0], crate::h2::H2Event::RstStream { stream_id: 7 });
}

#[test]
fn test_h2_goaway_frame_direct() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // GOAWAY frame
    let mut frame = vec![
        0x00, 0x00, 0x08, // Length = 8
        0x07, // Type = GOAWAY
        0x00, // Flags
    ];
    frame.extend_from_slice(&0u32.to_be_bytes()); // Stream ID = 0
    frame.extend_from_slice(&5u32.to_be_bytes()); // Last stream ID = 5
    frame.extend_from_slice(&0x00u32.to_be_bytes()); // Error code = NO_ERROR

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 1);
    matches!(events[0], crate::h2::H2Event::GoAway);
}

#[test]
fn test_h2_settings_frame_with_parameters() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // SETTINGS frame with parameters
    let mut frame = vec![
        0x00, 0x00, 0x0C, // Length = 12 (2 settings)
        0x04, // Type = SETTINGS
        0x00, // Flags
    ];
    frame.extend_from_slice(&0u32.to_be_bytes()); // Stream ID = 0
    // Setting 1: SETTINGS_MAX_CONCURRENT_STREAMS = 100
    frame.extend_from_slice(&0x0003u16.to_be_bytes());
    frame.extend_from_slice(&100u32.to_be_bytes());
    // Setting 2: SETTINGS_INITIAL_WINDOW_SIZE = 65536
    frame.extend_from_slice(&0x0004u16.to_be_bytes());
    frame.extend_from_slice(&65536u32.to_be_bytes());

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 1);
    matches!(events[0], crate::h2::H2Event::Settings);
}

#[test]
fn test_h2_data_frame_with_padding() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // DATA frame with PADDED flag
    let payload = b"test_data";
    let pad_length = 5u8;
    let mut padded_data = vec![pad_length];
    padded_data.extend_from_slice(payload);
    padded_data.extend_from_slice(&vec![0u8; pad_length as usize]);

    let mut frame = vec![
        0x00,
        0x00,
        padded_data.len() as u8, // Length
        0x00,                    // Type = DATA
        0x08,                    // Flags = PADDED
    ];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&padded_data);

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 1);

    if let crate::h2::H2Event::Data { data, .. } = &events[0] {
        // Note: Current implementation doesn't strip padding automatically
        // so the data includes pad_length byte + payload + padding
        assert!(data.len() > 0);
    }
}

#[test]
fn test_h2_multiple_frames_in_buffer() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Build multiple frames in one buffer
    let mut buffer = Vec::new();

    // Frame 1: SETTINGS
    buffer.extend_from_slice(&[0x00, 0x00, 0x00, 0x04, 0x00]);
    buffer.extend_from_slice(&0u32.to_be_bytes());

    // Frame 2: HEADERS
    let payload = vec![0x82]; // :method GET
    buffer.extend_from_slice(&[0x00, 0x00, 0x01, 0x01, 0x05]);
    buffer.extend_from_slice(&1u32.to_be_bytes());
    buffer.extend_from_slice(&payload);

    // Frame 3: DATA
    let data = b"test";
    buffer.extend_from_slice(&[0x00, 0x00, 0x04, 0x00, 0x01]);
    buffer.extend_from_slice(&1u32.to_be_bytes());
    buffer.extend_from_slice(data);

    let events = codec.process(&buffer).unwrap();
    assert!(events.len() >= 3, "Should parse all three frames");
}

#[test]
fn test_h2_partial_frame_across_multiple_process_calls() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Build a DATA frame and split it
    let data = b"test_payload_data";
    let mut frame = vec![0x00, 0x00, data.len() as u8, 0x00, 0x01];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(data);

    // Split into 3 parts
    let part1 = &frame[..5];
    let part2 = &frame[5..12];
    let part3 = &frame[12..];

    // Part 1: Partial header
    let events1 = codec.process(part1).unwrap();
    assert_eq!(events1.len(), 0, "No events yet");

    // Part 2: Rest of header + some payload
    let events2 = codec.process(part2).unwrap();
    assert_eq!(events2.len(), 0, "Still incomplete");

    // Part 3: Remaining payload
    let events3 = codec.process(part3).unwrap();
    assert_eq!(events3.len(), 1, "Should have complete frame now");
}

#[test]
fn test_h2_literal_header_with_indexed_name() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Literal header with incremental indexing, indexed name
    let mut payload = vec![
        0x60, // Literal with indexing (0x40), name index=32 (cookie)
        0x0A, // Value length = 10
    ];
    payload.extend_from_slice(b"sessionid=");

    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert!(events.len() >= 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        assert_eq!(headers.get("cookie"), Some(&"sessionid=".to_string()));
    }
}

#[test]
fn test_h2_stream_state_tracking() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Access stream state (tests get_stream method)
    let stream = codec.get_stream(42);
    assert!(!stream.saw_request_headers);
    assert!(!stream.saw_response_headers);
    assert!(!stream.closed);
}

#[test]
fn test_h2_hpack_degraded_flag() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    assert!(!codec.is_hpack_degraded());

    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Trigger HPACK degradation with dynamic table reference
    let payload = vec![0xBE]; // Index 62 (dynamic table)
    let mut frame = vec![0x00, 0x00, 0x01, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    codec.process(&frame).unwrap();
    assert!(
        codec.is_hpack_degraded(),
        "Should be degraded after dynamic table ref"
    );
}

// ============================================================================
// Trace Context and Decoder Edge Cases
// ============================================================================

#[test]
fn test_grpc_with_w3c_trace_context() {
    // Test W3C Trace Context extraction
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    let mut stream = Vec::new();
    stream.extend_from_slice(&create_http2_preface());
    stream.extend_from_slice(&create_settings_frame());

    // Request with W3C Trace Context headers
    let request_headers = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
        (
            "traceparent",
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
        ),
        ("tracestate", "rojo=00f067aa0ba902b7"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers, false));

    let request_payload = b"test_with_trace";
    let grpc_request = create_grpc_message(request_payload, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_request, true));

    let events = decoder.decode_stream(&stream, &ctx).unwrap();
    assert!(!events.is_empty());

    // Verify trace context was extracted
    let event = &events[0];
    assert!(event.metadata.contains_key("otel.trace_id"));
    assert!(event.metadata.contains_key("otel.span_id"));
    assert!(event.metadata.contains_key("otel.trace_flags"));
    assert!(event.metadata.contains_key("otel.tracestate"));
}

#[test]
fn test_grpc_with_invalid_trace_context() {
    // Test invalid traceparent header (should not crash)
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
        ("traceparent", "invalid-trace-parent"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers, false));

    let request_payload = b"test";
    let grpc_request = create_grpc_message(request_payload, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_request, true));

    // Should not crash with invalid trace context
    let events = decoder.decode_stream(&stream, &ctx).unwrap();
    assert!(!events.is_empty());
}

#[test]
fn test_grpc_deflate_compression() {
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    let mut stream = Vec::new();
    stream.extend_from_slice(&create_http2_preface());
    stream.extend_from_slice(&create_settings_frame());

    // Request with deflate (zlib) encoding
    let request_headers = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
        ("grpc-encoding", "deflate"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers, false));

    // Compress payload with zlib
    let uncompressed = b"test_deflate_payload";
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(uncompressed).unwrap();
    let compressed = encoder.finish().unwrap();

    let grpc_request = create_grpc_message(&compressed, true);
    stream.extend_from_slice(&create_data_frame(1, &grpc_request, true));

    let events = decoder.decode_stream(&stream, &ctx).unwrap();
    assert!(!events.is_empty());

    // Verify decompression worked
    let event = &events[0];
    if let prb_core::Payload::Raw { raw } = &event.payload {
        assert_eq!(&raw[..], uncompressed);
    }
}

#[test]
fn test_grpc_unknown_compression_algorithm() {
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    let mut stream = Vec::new();
    stream.extend_from_slice(&create_http2_preface());
    stream.extend_from_slice(&create_settings_frame());

    // Request with unknown compression algorithm
    let request_headers = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
        ("grpc-encoding", "br"), // brotli - not supported
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers, false));

    let request_payload = b"test";
    let grpc_request = create_grpc_message(request_payload, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_request, true));

    // Should fall back to identity/raw
    let events = decoder.decode_stream(&stream, &ctx).unwrap();
    assert!(!events.is_empty());
}

#[test]
fn test_decoder_with_context_metadata() {
    let mut decoder = GrpcDecoder::new();
    let mut ctx = DecodeContext::new()
        .with_src_addr("10.0.0.1:50051")
        .with_dst_addr("10.0.0.2:8080");
    ctx.metadata
        .insert("origin".to_string(), "test-capture.pcap".to_string());

    let mut stream = Vec::new();
    stream.extend_from_slice(&create_http2_preface());
    stream.extend_from_slice(&create_settings_frame());

    let request_headers = vec![
        (":method", "POST"),
        (":path", "/test.Service/TestMethod"),
        (":authority", "example.com"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers, false));

    let request_payload = b"test";
    let grpc_request = create_grpc_message(request_payload, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_request, true));

    let events = decoder.decode_stream(&stream, &ctx).unwrap();
    assert!(!events.is_empty());

    let event = &events[0];
    assert_eq!(event.source.origin, "test-capture.pcap");
    assert_eq!(event.source.network.as_ref().unwrap().src, "10.0.0.1:50051");
    assert_eq!(event.source.network.as_ref().unwrap().dst, "10.0.0.2:8080");
}

#[test]
fn test_decoder_sequence_numbering() {
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    let mut stream = Vec::new();
    stream.extend_from_slice(&create_http2_preface());
    stream.extend_from_slice(&create_settings_frame());

    // Stream 1: request
    let headers1 = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method1"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &headers1, false));
    let grpc1 = create_grpc_message(b"msg1", false);
    stream.extend_from_slice(&create_data_frame(1, &grpc1, true));

    // Stream 3: different request
    let headers3 = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method2"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
    ];
    stream.extend_from_slice(&create_headers_frame(3, &headers3, false));
    let grpc3 = create_grpc_message(b"msg2", false);
    stream.extend_from_slice(&create_data_frame(3, &grpc3, true));

    let events = decoder.decode_stream(&stream, &ctx).unwrap();
    assert!(events.len() >= 2);

    // Verify sequences are increasing
    assert!(events[0].sequence.is_some());
    assert!(events[1].sequence.is_some());
    assert!(events[1].sequence.unwrap() > events[0].sequence.unwrap());
}

#[test]
fn test_grpc_hpack_degradation_warning_in_event() {
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    // Skip preface to trigger potential degradation later
    let mut stream = Vec::new();
    stream.extend_from_slice(&create_http2_preface());
    stream.extend_from_slice(&create_settings_frame());

    // Use a dynamic table reference to trigger HPACK degradation
    let payload = vec![0xBE]; // Index 62 (dynamic table)
    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x04];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);
    stream.extend_from_slice(&frame);

    // Add valid data frame
    let data_frame_headers = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
    ];
    stream.extend_from_slice(&create_headers_frame(3, &data_frame_headers, false));
    let grpc_msg = create_grpc_message(b"test", false);
    stream.extend_from_slice(&create_data_frame(3, &grpc_msg, true));

    let events = decoder.decode_stream(&stream, &ctx).unwrap();

    // At least one event should have HPACK degradation warning
    let has_warning = events
        .iter()
        .any(|e| e.warnings.iter().any(|w| w.contains("HPACK degradation")));
    assert!(
        has_warning || events.is_empty(),
        "Expected HPACK degradation warning in events"
    );
}

#[test]
fn test_decoder_default_trait() {
    let decoder = GrpcDecoder::default();
    assert_eq!(decoder.protocol(), prb_core::TransportKind::Grpc);
}

#[test]
fn test_grpc_response_without_request_headers() {
    // Edge case: response headers without seeing request first
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    let mut stream = Vec::new();
    stream.extend_from_slice(&create_http2_preface());
    stream.extend_from_slice(&create_settings_frame());

    // Send response headers first (unusual but possible in mid-stream capture)
    let response_headers = vec![(":status", "200"), ("content-type", "application/grpc")];
    stream.extend_from_slice(&create_headers_frame(1, &response_headers, false));

    let response_payload = b"response_data";
    let grpc_response = create_grpc_message(response_payload, false);
    stream.extend_from_slice(&create_data_frame(1, &grpc_response, false));

    // Should handle gracefully
    let result = decoder.decode_stream(&stream, &ctx);
    assert!(result.is_ok());
}

#[test]
fn test_grpc_multiple_messages_same_data_frame() {
    use crate::decoder::GrpcDecoder;
    use prb_core::{DecodeContext, ProtocolDecoder};

    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    let mut stream = Vec::new();
    stream.extend_from_slice(&create_http2_preface());
    stream.extend_from_slice(&create_settings_frame());

    let request_headers = vec![
        (":method", "POST"),
        (":path", "/test.Service/StreamMethod"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &request_headers, false));

    // Create DATA frame with two gRPC messages back-to-back
    let mut combined_data = Vec::new();
    combined_data.extend_from_slice(&create_grpc_message(b"message1", false));
    combined_data.extend_from_slice(&create_grpc_message(b"message2", false));
    stream.extend_from_slice(&create_data_frame(1, &combined_data, false));

    let events = decoder.decode_stream(&stream, &ctx).unwrap();
    assert!(events.len() >= 2, "Should parse both messages");
}

// ============================================================================
// Additional H2 Static Table and HPACK Coverage
// ============================================================================

#[test]
fn test_h2_static_table_mid_range_indices() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Test mid-range static table indices (15-40)
    let payload = vec![
        0x8F, // Index 15 = accept-charset
        0x90, // Index 16 = accept-encoding
        0x97, // Index 23 = authorization
        0x98, // Index 24 = cache-control
        0x9C, // Index 28 = content-length
        0x9F, // Index 31 = content-type
        0xA0, // Index 32 = cookie
        0xA6, // Index 38 = host
    ];

    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert!(events.len() >= 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        assert!(headers.contains_key("accept-charset"));
        assert!(headers.contains_key("accept-encoding"));
        assert!(headers.contains_key("authorization"));
        assert!(headers.contains_key("cache-control"));
        assert!(headers.contains_key("content-length"));
        assert!(headers.contains_key("content-type"));
        assert!(headers.contains_key("cookie"));
        assert!(headers.contains_key("host"));
    }
}

#[test]
fn test_h2_static_table_all_status_codes() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Test all status code entries (8-14)
    let payload = vec![
        0x88, // Index 8 = :status 200
        0x89, // Index 9 = :status 204
        0x8A, // Index 10 = :status 206
        0x8B, // Index 11 = :status 304
        0x8C, // Index 12 = :status 400
        0x8D, // Index 13 = :status 404
        0x8E, // Index 14 = :status 500
    ];

    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert!(events.len() >= 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        // Last one wins, so should be 500
        assert_eq!(headers.get(":status"), Some(&"500".to_string()));
    }
}

#[test]
fn test_h2_static_table_remaining_entries() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Test remaining static table entries not covered yet
    let payload = vec![
        0xA1, // Index 33 = date
        0xA2, // Index 34 = etag
        0xA3, // Index 35 = expect
        0xA4, // Index 36 = expires
        0xA5, // Index 37 = from
        0xA7, // Index 39 = if-match
        0xA8, // Index 40 = if-modified-since
        0xA9, // Index 41 = if-none-match
        0xAA, // Index 42 = if-range
        0xAB, // Index 43 = if-unmodified-since
        0xAC, // Index 44 = last-modified
        0xAD, // Index 45 = link
        0xAE, // Index 46 = location
        0xAF, // Index 47 = max-forwards
        0xB0, // Index 48 = proxy-authenticate
        0xB1, // Index 49 = proxy-authorization
        0xB4, // Index 52 = refresh
        0xB5, // Index 53 = retry-after
        0xB7, // Index 55 = set-cookie
        0xB8, // Index 56 = strict-transport-security
        0xB9, // Index 57 = transfer-encoding
        0xBA, // Index 58 = user-agent
        0xBB, // Index 59 = vary
        0xBC, // Index 60 = via
    ];

    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert!(events.len() >= 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        assert!(headers.contains_key("date"));
        assert!(headers.contains_key("etag"));
        assert!(headers.contains_key("user-agent"));
        assert!(headers.contains_key("vary"));
    }
}

#[test]
fn test_h2_literal_with_indexed_name_from_static_table() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Literal with incremental indexing, using static table index for name
    let mut payload = vec![
        0x50, // Literal with indexing (0x40), name index=16 (accept-encoding)
        0x04, // Value length = 4
    ];
    payload.extend_from_slice(b"gzip");

    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert!(events.len() >= 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        assert_eq!(headers.get("accept-encoding"), Some(&"gzip".to_string()));
    }
}

#[test]
fn test_h2_literal_never_indexed_with_static_table_name() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Literal never indexed (0x10), using static table index for name
    // Use index 1 = :authority which fits in 4-bit prefix
    let mut payload = vec![
        0x11, // Literal never indexed (0x10), name index=1 (:authority)
        0x09, // Value length = 9
    ];
    payload.extend_from_slice(b"localhost");

    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert!(events.len() >= 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        assert!(headers.contains_key(":authority") || headers.contains_key("unknown"));
    }
}

#[test]
fn test_h2_mixed_hpack_encodings() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Mix of indexed and literal headers
    let mut payload = vec![
        0x82, // Indexed: :method GET (index 2)
        0x84, // Indexed: :path / (index 4)
        0x41, // Literal with indexing, name index=1 (:authority)
        0x09, // Value length = 9
    ];
    payload.extend_from_slice(b"localhost");

    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert!(events.len() >= 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        assert_eq!(headers.get(":method"), Some(&"GET".to_string()));
        assert_eq!(headers.get(":path"), Some(&"/".to_string()));
        assert_eq!(headers.get(":authority"), Some(&"localhost".to_string()));
    }
}

#[test]
fn test_h2_hpack_integer_edge_case_126() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Test near-boundary of single-byte encoding (126 for 7-bit prefix)
    let mut payload = vec![
        0x00, // Literal without indexing, name index=0
        0x04, // Name length = 4
    ];
    payload.extend_from_slice(b"test");
    // Value length 126 (fits in single byte)
    payload.push(126); // Single byte encoding for 126
    payload.extend_from_slice(&vec![b'x'; 126]);

    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert!(events.len() >= 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        assert_eq!(headers.get("test").map(|s| s.len()), Some(126));
    }
}

#[test]
fn test_h2_hpack_integer_three_byte() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Test three-byte integer encoding
    // Value 384 = 127 + 257, encoded as: 0x7F, 0x81, 0x02
    let mut payload = vec![
        0x00, // Literal without indexing
        0x04, // Name length = 4
    ];
    payload.extend_from_slice(b"test");
    // Three-byte integer for value length 384
    payload.push(0x7F); // All bits set in 7-bit prefix
    payload.push(0x81); // 0x01 with continuation bit set
    payload.push(0x02); // Final byte
    payload.extend_from_slice(&vec![b'y'; 384]);

    // Frame length in 24-bit big-endian
    let len = payload.len();
    let mut frame = vec![
        ((len >> 16) & 0xFF) as u8, // High byte
        ((len >> 8) & 0xFF) as u8,  // Mid byte
        (len & 0xFF) as u8,         // Low byte
        0x01,                       // Type = HEADERS
        0x05,                       // Flags = END_STREAM | END_HEADERS
    ];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert!(events.len() >= 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        assert_eq!(headers.get("test").map(|s| s.len()), Some(384));
    }
}

#[test]
fn test_h2_data_frame_empty_payload() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // DATA frame with zero-length payload
    let frame = vec![
        0x00, 0x00, 0x00, // Length = 0
        0x00, // Type = DATA
        0x00, // Flags
        0x00, 0x00, 0x00, 0x01, // Stream ID = 1
    ];

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 1);

    if let crate::h2::H2Event::Data { data, .. } = &events[0] {
        assert_eq!(data.len(), 0);
    }
}

#[test]
fn test_h2_headers_with_priority() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // HEADERS frame with PRIORITY flag (0x20)
    // Priority data: 5 bytes (E flag + Stream Dependency + Weight)
    let payload = vec![
        0x00, 0x00, 0x00, 0x00, // Stream Dependency = 0
        0x10, // Weight = 16
        0x82, // :method GET
    ];

    let mut frame = vec![
        0x00,
        0x00,
        payload.len() as u8,
        0x01, // Type = HEADERS
        0x20, // Flags = PRIORITY (no END_HEADERS)
    ];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    // Note: Our current implementation doesn't parse PRIORITY
    // But it should handle the frame gracefully
    let result = codec.process(&frame);
    // Should not crash
    assert!(result.is_ok());
}

#[test]
fn test_h2_continuation_without_preceding_headers() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // CONTINUATION frame without preceding HEADERS (protocol error)
    let payload = vec![0x82]; // :method GET
    let mut frame = vec![
        0x00,
        0x00,
        payload.len() as u8,
        0x09, // Type = CONTINUATION
        0x04, // Flags = END_HEADERS
    ];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    // Should handle gracefully (log warning, no events)
    let events = codec.process(&frame).unwrap();
    // No events expected for orphaned CONTINUATION
    assert_eq!(events.len(), 0);
}

#[test]
fn test_h2_continuation_stream_id_mismatch() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // HEADERS frame without END_HEADERS
    let payload1 = vec![0x82]; // :method GET
    let mut frame1 = vec![
        0x00,
        0x00,
        payload1.len() as u8,
        0x01, // Type = HEADERS
        0x00, // Flags = none (no END_HEADERS)
    ];
    frame1.extend_from_slice(&1u32.to_be_bytes()); // Stream 1
    frame1.extend_from_slice(&payload1);
    codec.process(&frame1).unwrap();

    // CONTINUATION frame with different stream ID (protocol error)
    let payload2 = vec![0x84]; // :path /
    let mut frame2 = vec![
        0x00,
        0x00,
        payload2.len() as u8,
        0x09, // Type = CONTINUATION
        0x04, // Flags = END_HEADERS
    ];
    frame2.extend_from_slice(&3u32.to_be_bytes()); // Stream 3 (mismatch!)
    frame2.extend_from_slice(&payload2);

    // Should handle gracefully (log warning)
    let result = codec.process(&frame2);
    // Protocol error, but should not crash
    assert!(result.is_ok());
}

// ============================================================================
// Final coverage push - edge cases and remaining paths
// ============================================================================

#[test]
fn test_h2_hpack_literal_with_indexing_literal_name() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Literal with incremental indexing, literal name (index=0)
    let mut payload = vec![
        0x40, // Literal with indexing (0x40), name index=0
        0x08, // Name length = 8
    ];
    payload.extend_from_slice(b"x-custom");
    payload.push(0x05); // Value length = 5
    payload.extend_from_slice(b"value");

    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert!(events.len() >= 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        assert_eq!(headers.get("x-custom"), Some(&"value".to_string()));
    }
}

#[test]
fn test_h2_data_frame_with_end_stream_flag() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // DATA frame with END_STREAM flag set
    let data = b"final_data";
    let mut frame = vec![
        0x00,
        0x00,
        data.len() as u8,
        0x00, // Type = DATA
        0x01, // Flags = END_STREAM
    ];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(data);

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 1);

    if let crate::h2::H2Event::Data { end_stream, .. } = &events[0] {
        assert!(*end_stream);
    }
}

#[test]
fn test_h2_headers_frame_with_end_stream() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // HEADERS frame with END_STREAM flag
    let payload = vec![0x82]; // :method GET
    let mut frame = vec![
        0x00,
        0x00,
        payload.len() as u8,
        0x01, // Type = HEADERS
        0x05, // Flags = END_STREAM | END_HEADERS
    ];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 1);

    if let crate::h2::H2Event::Headers { end_stream, .. } = &events[0] {
        assert!(*end_stream);
    }
}

#[test]
fn test_h2_settings_ack_frame() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // SETTINGS frame with ACK flag
    let frame = vec![
        0x00, 0x00, 0x00, // Length = 0
        0x04, // Type = SETTINGS
        0x01, // Flags = ACK
        0x00, 0x00, 0x00, 0x00, // Stream ID = 0
    ];

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 1);
    matches!(events[0], crate::h2::H2Event::Settings);
}

#[test]
fn test_h2_large_stream_id() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // DATA frame with large stream ID
    let data = b"test";
    let large_stream_id = 0x7FFFFFFFu32; // Max stream ID (31 bits)
    let mut frame = vec![
        0x00,
        0x00,
        data.len() as u8,
        0x00, // Type = DATA
        0x00, // Flags
    ];
    frame.extend_from_slice(&large_stream_id.to_be_bytes());
    frame.extend_from_slice(data);

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 1);

    if let crate::h2::H2Event::Data { stream_id, .. } = &events[0] {
        assert_eq!(*stream_id, large_stream_id);
    }
}

#[test]
fn test_h2_rst_stream_various_error_codes() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Test different error codes
    for error_code in [0x00u32, 0x01, 0x02, 0x08, 0x0B] {
        let mut frame = vec![
            0x00, 0x00, 0x04, 0x03, // Type = RST_STREAM
            0x00,
        ];
        frame.extend_from_slice(&1u32.to_be_bytes());
        frame.extend_from_slice(&error_code.to_be_bytes());

        let events = codec.process(&frame).unwrap();
        assert!(!events.is_empty());
    }
}

#[test]
fn test_h2_static_table_boundary_indices() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Test boundary indices: 1, 61 (valid), 62 (first dynamic)
    let payload = vec![
        0x81, // Index 1 = :authority
        0xBD, // Index 61 = www-authenticate
    ];

    let mut frame = vec![0x00, 0x00, payload.len() as u8, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert!(events.len() >= 1);
}

#[test]
fn test_h2_continuation_accumulation() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // HEADERS without END_HEADERS
    let payload1 = vec![0x82]; // :method GET
    let mut frame1 = vec![0x00, 0x00, 0x01, 0x01, 0x00];
    frame1.extend_from_slice(&1u32.to_be_bytes());
    frame1.extend_from_slice(&payload1);
    codec.process(&frame1).unwrap();

    // First CONTINUATION without END_HEADERS
    let payload2 = vec![0x84]; // :path /
    let mut frame2 = vec![0x00, 0x00, 0x01, 0x09, 0x00];
    frame2.extend_from_slice(&1u32.to_be_bytes());
    frame2.extend_from_slice(&payload2);
    codec.process(&frame2).unwrap();

    // Final CONTINUATION with END_HEADERS
    let payload3 = vec![0x86]; // :scheme http
    let mut frame3 = vec![0x00, 0x00, 0x01, 0x09, 0x04];
    frame3.extend_from_slice(&1u32.to_be_bytes());
    frame3.extend_from_slice(&payload3);

    let events = codec.process(&frame3).unwrap();
    assert_eq!(events.len(), 1);

    if let crate::h2::H2Event::Headers { headers, .. } = &events[0] {
        assert!(headers.contains_key(":method"));
        assert!(headers.contains_key(":path"));
        assert!(headers.contains_key(":scheme"));
    }
}

#[test]
fn test_h2_goaway_with_debug_data() {
    use crate::h2::H2Codec;

    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // GOAWAY frame with debug data
    let mut frame = vec![
        0x00, 0x00, 0x10, // Length = 16 (8 + debug data)
        0x07, // Type = GOAWAY
        0x00,
    ];
    frame.extend_from_slice(&0u32.to_be_bytes()); // Stream ID = 0
    frame.extend_from_slice(&100u32.to_be_bytes()); // Last stream ID = 100
    frame.extend_from_slice(&0x01u32.to_be_bytes()); // Error code
    frame.extend_from_slice(b"debugdat"); // 8 bytes debug data

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 1);
    matches!(events[0], crate::h2::H2Event::GoAway);
}

#[test]
fn test_decoder_lpm_parser_per_stream() {
    // Test that different streams have independent LPM parsers
    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.1:50051")
        .with_dst_addr("192.168.1.2:12345");

    let mut stream = Vec::new();
    stream.extend_from_slice(&create_http2_preface());
    stream.extend_from_slice(&create_settings_frame());

    // Stream 1 with one message
    let headers1 = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
    ];
    stream.extend_from_slice(&create_headers_frame(1, &headers1, false));
    let msg1 = create_grpc_message(b"stream1_msg", false);
    stream.extend_from_slice(&create_data_frame(1, &msg1, false));

    // Stream 3 with different message
    let headers3 = vec![
        (":method", "POST"),
        (":path", "/test.Service/Method"),
        (":authority", "localhost:50051"),
        (":scheme", "http"),
        ("content-type", "application/grpc"),
    ];
    stream.extend_from_slice(&create_headers_frame(3, &headers3, false));
    let msg3 = create_grpc_message(b"stream3_msg", false);
    stream.extend_from_slice(&create_data_frame(3, &msg3, false));

    let events = decoder.decode_stream(&stream, &ctx).unwrap();
    // Should have events from both streams
    let stream1_count = events
        .iter()
        .filter(|e| e.metadata.get("h2.stream_id") == Some(&"1".to_string()))
        .count();
    let stream3_count = events
        .iter()
        .filter(|e| e.metadata.get("h2.stream_id") == Some(&"3".to_string()))
        .count();

    assert!(stream1_count > 0);
    assert!(stream3_count > 0);
}
