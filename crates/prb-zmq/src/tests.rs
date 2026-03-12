//! Tests for ZMTP decoder.

use crate::decoder::ZmqDecoder;
use crate::parser::{ZmtpEvent, ZmtpParser};
use prb_core::{DecodeContext, ProtocolDecoder};
use std::collections::BTreeMap;

/// Helper to build a valid ZMTP 3.0 greeting with NULL mechanism.
fn build_greeting(major: u8, minor: u8, mechanism: &str, as_server: bool) -> Vec<u8> {
    let mut greeting = vec![0u8; 64];
    // Signature: 0xFF + 8 padding bytes + 0x7F
    greeting[0] = 0xFF;
    greeting[9] = 0x7F;
    // Version
    greeting[10] = major;
    greeting[11] = minor;
    // Mechanism (20 bytes, null-padded)
    let mech_bytes = mechanism.as_bytes();
    greeting[12..12 + mech_bytes.len()].copy_from_slice(mech_bytes);
    // as-server flag
    greeting[32] = u8::from(as_server);
    greeting
}

/// Helper to build a READY command frame.
fn build_ready_command(properties: &[(&str, &[u8])]) -> Vec<u8> {
    let mut body = Vec::new();

    // Command name length + name
    body.push(5); // "READY" length
    body.extend_from_slice(b"READY");

    // Properties
    for (name, value) in properties {
        body.push(name.len() as u8);
        body.extend_from_slice(name.as_bytes());
        body.extend_from_slice(&(value.len() as u32).to_be_bytes());
        body.extend_from_slice(value);
    }

    // Build frame: flags (0x04 = short command) + size + body
    let mut frame = Vec::new();
    frame.push(0x04); // Short command flag
    frame.push(body.len() as u8);
    frame.extend_from_slice(&body);
    frame
}

/// Helper to build a message frame.
fn build_message_frame(data: &[u8], has_more: bool) -> Vec<u8> {
    let mut frame = Vec::new();
    let flags = u8::from(has_more); // MORE flag
    frame.push(flags);
    frame.push(data.len() as u8);
    frame.extend_from_slice(data);
    frame
}

/// Helper to build a long message frame (8-byte size).
fn build_long_message_frame(data: &[u8], has_more: bool) -> Vec<u8> {
    let mut frame = Vec::new();
    let flags = if has_more { 0x03 } else { 0x02 }; // LONG flag + optional MORE
    frame.push(flags);
    frame.extend_from_slice(&(data.len() as u64).to_be_bytes());
    frame.extend_from_slice(data);
    frame
}

#[test]
fn test_zmtp_greeting_parse() {
    let mut parser = ZmtpParser::new();
    let greeting = build_greeting(3, 0, "NULL", false);

    let events = parser.feed(&greeting).expect("parse greeting");
    assert_eq!(events.len(), 1);

    match &events[0] {
        ZmtpEvent::Greeting(g) => {
            assert_eq!(g.major_version, 3);
            assert_eq!(g.minor_version, 0);
            assert_eq!(g.mechanism, "NULL");
            assert!(!g.as_server);
        }
        _ => panic!("Expected Greeting event"),
    }
}

#[test]
fn test_zmtp_ready_metadata() {
    let mut parser = ZmtpParser::new();
    let mut data = build_greeting(3, 0, "NULL", false);
    let ready = build_ready_command(&[("Socket-Type", b"PUB"), ("Identity", b"test-pub")]);
    data.extend_from_slice(&ready);

    let events = parser.feed(&data).expect("parse greeting and ready");
    assert_eq!(events.len(), 2);

    match &events[1] {
        ZmtpEvent::Ready(r) => {
            assert_eq!(r.properties.get("Socket-Type").unwrap(), b"PUB");
            assert_eq!(r.properties.get("Identity").unwrap(), b"test-pub");
        }
        _ => panic!("Expected Ready event"),
    }
}

#[test]
fn test_zmtp_single_frame_message() {
    let mut parser = ZmtpParser::new();
    let mut data = build_greeting(3, 0, "NULL", false);
    data.extend_from_slice(&build_ready_command(&[("Socket-Type", b"REQ")]));
    data.extend_from_slice(&build_message_frame(b"hello", false));

    let events = parser.feed(&data).expect("parse");
    assert_eq!(events.len(), 3); // Greeting, Ready, Message

    match &events[2] {
        ZmtpEvent::Message(m) => {
            assert_eq!(m.frames.len(), 1);
            assert_eq!(m.frames[0], b"hello");
        }
        _ => panic!("Expected Message event"),
    }
}

#[test]
fn test_zmtp_multipart_message() {
    let mut parser = ZmtpParser::new();
    let mut data = build_greeting(3, 0, "NULL", false);
    data.extend_from_slice(&build_ready_command(&[("Socket-Type", b"REQ")]));
    data.extend_from_slice(&build_message_frame(b"frame1", true));
    data.extend_from_slice(&build_message_frame(b"frame2", true));
    data.extend_from_slice(&build_message_frame(b"frame3", false));

    let events = parser.feed(&data).expect("parse");
    assert_eq!(events.len(), 3); // Greeting, Ready, Message

    match &events[2] {
        ZmtpEvent::Message(m) => {
            assert_eq!(m.frames.len(), 3);
            assert_eq!(m.frames[0], b"frame1");
            assert_eq!(m.frames[1], b"frame2");
            assert_eq!(m.frames[2], b"frame3");
        }
        _ => panic!("Expected Message event"),
    }
}

#[test]
fn test_zmtp_long_frame() {
    let mut parser = ZmtpParser::new();
    let mut data = build_greeting(3, 0, "NULL", false);
    data.extend_from_slice(&build_ready_command(&[("Socket-Type", b"REQ")]));

    // Create a large payload
    let large_payload = vec![0x42u8; 1000];
    data.extend_from_slice(&build_long_message_frame(&large_payload, false));

    let events = parser.feed(&data).expect("parse");
    assert_eq!(events.len(), 3);

    match &events[2] {
        ZmtpEvent::Message(m) => {
            assert_eq!(m.frames.len(), 1);
            assert_eq!(m.frames[0].len(), 1000);
            assert_eq!(m.frames[0][0], 0x42);
        }
        _ => panic!("Expected Message event"),
    }
}

#[test]
fn test_zmtp_pubsub_topic() {
    let mut decoder = ZmqDecoder::new();
    let mut data = build_greeting(3, 0, "NULL", false);
    data.extend_from_slice(&build_ready_command(&[("Socket-Type", b"PUB")]));
    data.extend_from_slice(&build_message_frame(b"sensor.temp", true));
    data.extend_from_slice(&build_message_frame(b"payload-data", false));

    let ctx = DecodeContext {
        src_addr: Some("10.0.0.1:5555".to_string()),
        dst_addr: Some("10.0.0.2:5556".to_string()),
        metadata: BTreeMap::new(),
        timestamp: None,
    };

    let events = decoder.decode_stream(&data, &ctx).expect("decode");
    assert_eq!(events.len(), 1);

    let event = &events[0];
    assert_eq!(
        event
            .metadata
            .get("zmq.topic")
            .map(std::string::String::as_str),
        Some("sensor.temp")
    );
    assert_eq!(
        event
            .metadata
            .get("zmq.socket_type")
            .map(std::string::String::as_str),
        Some("PUB")
    );
}

#[test]
fn test_zmtp_mid_stream_degraded() {
    let mut parser = ZmtpParser::new();

    // Feed data without greeting - create invalid greeting signature
    // to trigger degraded mode detection (need at least 10 bytes)
    let mut invalid_data = vec![0x00; 10]; // Not 0xFF at byte 0
    invalid_data.extend_from_slice(&build_message_frame(b"data", false));

    // First feed should trigger degraded mode
    let _events = parser.feed(&invalid_data).expect("parse");

    // Parser should enter degraded mode
    assert!(parser.is_degraded());

    // In degraded mode, it should try to parse frames heuristically
}

#[test]
fn test_zmtp_invalid_version() {
    let mut parser = ZmtpParser::new();
    let greeting = build_greeting(2, 0, "NULL", false);

    let result = parser.feed(&greeting);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        crate::error::ZmqError::UnsupportedVersion { major: 2, minor: 0 }
    ));
}

#[test]
fn test_zmtp_incremental_feed() {
    let mut parser = ZmtpParser::new();
    let greeting = build_greeting(3, 0, "NULL", false);

    // Feed in small chunks
    for chunk in greeting.chunks(10) {
        let events = parser.feed(chunk).expect("parse");
        if chunk.len() == 10 && events.is_empty() {
            // Still accumulating
            continue;
        }
    }
}

#[test]
fn test_zmtp_parser_ready_with_multiple_properties() {
    let mut parser = ZmtpParser::new();
    let mut data = build_greeting(3, 1, "NULL", true);
    let ready = build_ready_command(&[
        ("Socket-Type", b"DEALER"),
        ("Identity", b"worker-001"),
        ("Custom-Prop", b"custom-value"),
    ]);
    data.extend_from_slice(&ready);

    let events = parser.feed(&data).expect("parse");
    assert_eq!(events.len(), 2);

    match &events[1] {
        ZmtpEvent::Ready(r) => {
            assert_eq!(r.properties.len(), 3);
            assert_eq!(r.properties.get("Socket-Type").unwrap(), b"DEALER");
            assert_eq!(r.properties.get("Identity").unwrap(), b"worker-001");
            assert_eq!(r.properties.get("Custom-Prop").unwrap(), b"custom-value");
        }
        _ => panic!("Expected Ready event"),
    }
}
