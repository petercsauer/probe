//! Real-world HTTP/1.x decode integration tests.
//!
//! These tests validate that HTTP payloads from real captures can be processed
//! through the decode pipeline without panicking. HTTP/1.x is text-based, not
//! protobuf, so these tests focus on graceful handling of non-protobuf data.

use prb_core::{CaptureAdapter, Payload};
use prb_decode::wire_format::decode_wire_format;
use prb_pcap::PcapCaptureAdapter;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/captures")
}

#[test]
fn test_real_data_http_payload_decode_robustness() {
    // Test that HTTP payloads don't crash the wire format decoder
    // Use http-chunked-gzip.pcap as it's a verified working capture
    let capture_path = fixtures_dir().join("http/http-chunked-gzip.pcap");
    if !capture_path.exists() {
        return;
    }

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    let mut payloads_tested = 0;
    for event_result in events.iter().filter_map(|r| r.as_ref().ok()) {
        if let Payload::Raw { raw: data } = &event_result.payload {
            // Try to decode as protobuf - most HTTP data won't be valid protobuf
            // but the decoder should not panic
            let _ = decode_wire_format(data);
            payloads_tested += 1;
        }
    }

    // Should have at least attempted to decode some payloads
    assert!(
        payloads_tested > 0,
        "Should test at least one payload from HTTP capture"
    );
}

#[test]
fn test_real_data_http_chunked_decode_robustness() {
    let capture_path = fixtures_dir().join("http/http-chunked-gzip.pcap");
    if !capture_path.exists() {
        return;
    }

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    let mut payloads_tested = 0;
    for event_result in events.iter().filter_map(|r| r.as_ref().ok()) {
        if let Payload::Raw { raw: data } = &event_result.payload {
            // HTTP chunked encoding is not protobuf, but should not crash decoder
            let _ = decode_wire_format(data);
            payloads_tested += 1;
        }
    }

    assert!(
        payloads_tested > 0,
        "Should test payloads from chunked HTTP capture"
    );
}

#[test]
fn test_real_data_http_large_payload_decode() {
    let capture_path = fixtures_dir().join("http/http_with_jpegs.cap");
    if !capture_path.exists() {
        return;
    }

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    let mut payloads_tested = 0;
    let mut large_payloads = 0;

    for event_result in events.iter().filter_map(|r| r.as_ref().ok()) {
        if let Payload::Raw { raw: data } = &event_result.payload {
            if data.len() > 1000 {
                large_payloads += 1;
            }
            // Large JPEG data is not protobuf, but should not crash decoder
            let _ = decode_wire_format(data);
            payloads_tested += 1;
        }
    }

    assert!(
        payloads_tested > 0,
        "Should test payloads from large payload capture"
    );
    assert!(
        large_payloads > 0,
        "Should encounter at least one large payload"
    );
}

#[test]
fn test_real_data_websocket_decode_robustness() {
    let capture_path = fixtures_dir().join("websocket/websocket.pcap");
    if !capture_path.exists() {
        // WebSocket captures may not be available
        return;
    }

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    for event_result in events.iter().filter_map(|r| r.as_ref().ok()) {
        if let Payload::Raw { raw: data } = &event_result.payload {
            // WebSocket frames are not protobuf, but should not crash decoder
            let _ = decode_wire_format(data);
        }
    }

    // Test passes if it doesn't panic
}

#[test]
fn test_real_data_http_decode_error_handling() {
    // Verify that decoding HTTP text data returns errors, not panics
    let http_get_request = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";

    let result = decode_wire_format(http_get_request);

    // HTTP text is not valid protobuf, so should return an error
    // The important thing is it doesn't panic
    assert!(result.is_err(), "HTTP text should not be valid protobuf");
}

#[test]
fn test_real_data_http_response_decode() {
    // Test decoding HTTP response headers
    let http_response =
        b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 13\r\n\r\nHello, world!";

    let result = decode_wire_format(http_response);

    // HTTP response is not valid protobuf
    assert!(
        result.is_err(),
        "HTTP response should not be valid protobuf"
    );
}

#[test]
fn test_real_data_all_http_captures_no_panic() {
    // Comprehensive test: decode all payloads from all HTTP captures
    let http_captures = vec!["http/http-chunked-gzip.pcap", "http/http_with_jpegs.cap"];

    let mut total_payloads = 0;

    for capture in http_captures {
        let path = fixtures_dir().join(capture);
        if !path.exists() {
            continue;
        }

        let mut adapter = PcapCaptureAdapter::new(path, None);
        let events: Vec<_> = adapter.ingest().collect();

        for event_result in events.iter().filter_map(|r| r.as_ref().ok()) {
            if let Payload::Raw { raw: data } = &event_result.payload {
                // Decode should never panic, even on non-protobuf data
                let _ = decode_wire_format(data);
                total_payloads += 1;
            }
        }
    }

    assert!(
        total_payloads > 0,
        "Should test payloads from at least one HTTP capture"
    );
}
