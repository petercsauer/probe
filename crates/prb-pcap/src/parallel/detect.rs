//! Protocol detection from decrypted stream data.
//!
//! This module provides simple heuristics to detect protocols from stream data:
//! - HTTP/2: connection preface or valid frame headers
//! - ZMTP: greeting signature (0xFF at byte 0, 0x7F at byte 9)
//! - RTPS: "RTPS" magic bytes at start
//! - Port-based fallback for common gRPC ports

use crate::tls::DecryptedStream;

/// Detected protocol type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedProtocol {
    /// gRPC over HTTP/2.
    Grpc,
    /// `ZeroMQ` ZMTP.
    Zmtp,
    /// DDS-RTPS.
    Rtps,
}

/// HTTP/2 connection preface (24 bytes).
const H2_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// Detects protocol from a decrypted stream.
///
/// Detection logic:
/// 1. ZMTP greeting signature (0xFF at byte 0, 0x7F at byte 9) - checked first as it's very distinct
/// 2. RTPS magic bytes
/// 3. HTTP/2 connection preface (client-side)
/// 4. HTTP/2 frame header (server-side)
/// 5. Port-based fallback (50051, 443, 8443, 9090 → gRPC)
///
/// Returns `None` if protocol cannot be determined.
#[must_use] 
pub fn detect_protocol(stream: &DecryptedStream) -> Option<DetectedProtocol> {
    let data = &stream.data;

    // Need at least some bytes to detect
    if data.is_empty() {
        return port_based_detection(stream);
    }

    // Check ZMTP greeting signature FIRST (most distinct pattern)
    // Greeting: 0xFF (signature start), padding[1-8], 0x7F (signature end at byte 9)
    if data.len() >= 10 && data[0] == 0xFF && data[9] == 0x7F {
        return Some(DetectedProtocol::Zmtp);
    }

    // Check RTPS magic bytes (also very distinct)
    if data.len() >= 4 && &data[0..4] == b"RTPS" {
        return Some(DetectedProtocol::Rtps);
    }

    // Check HTTP/2 connection preface (24 bytes)
    if data.len() >= H2_PREFACE.len() && data.starts_with(H2_PREFACE) {
        return Some(DetectedProtocol::Grpc);
    }

    // Check HTTP/2 frame header (server-side or mid-stream)
    // Frame format: 3-byte length + 1-byte type + 1-byte flags + 4-byte stream ID
    // Be more strict: check reserved bit is 0 and stream ID is valid
    if data.len() >= 9 {
        let frame_type = data[3];
        // Valid HTTP/2 frame types: 0x00-0x09
        if frame_type <= 0x09 {
            // Check that length field is reasonable (< 16MB default max)
            let length = u32::from_be_bytes([0, data[0], data[1], data[2]]);
            // Check reserved bit (bit 31 of stream ID) is 0
            let reserved_bit = data[5] & 0x80;
            if length <= 16_777_216 && reserved_bit == 0 {
                // Likely HTTP/2
                return Some(DetectedProtocol::Grpc);
            }
        }
    }

    // Fallback to port-based detection
    port_based_detection(stream)
}

/// Port-based protocol detection fallback.
fn port_based_detection(stream: &DecryptedStream) -> Option<DetectedProtocol> {
    // Common gRPC ports
    const GRPC_PORTS: &[u16] = &[50051, 443, 8443, 9090];

    if GRPC_PORTS.contains(&stream.dst_port) || GRPC_PORTS.contains(&stream.src_port) {
        return Some(DetectedProtocol::Grpc);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tcp::StreamDirection;
    use std::net::{IpAddr, Ipv4Addr};

    fn make_stream(data: Vec<u8>, dst_port: u16) -> DecryptedStream {
        DecryptedStream {
            src_ip: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            src_port: 12345,
            dst_ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            dst_port,
            direction: StreamDirection::ClientToServer,
            data,
            encrypted: false,
            is_complete: true,
            timestamp_us: 1000,
        }
    }

    #[test]
    fn test_detect_http2_preface() {
        let data = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n".to_vec();
        let stream = make_stream(data, 8080);
        assert_eq!(detect_protocol(&stream), Some(DetectedProtocol::Grpc));
    }

    #[test]
    fn test_detect_http2_frames() {
        // HTTP/2 SETTINGS frame: length=0x000000, type=0x04, flags=0x00, stream_id=0x00000000
        let data = vec![0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00];
        let stream = make_stream(data, 8080);
        assert_eq!(detect_protocol(&stream), Some(DetectedProtocol::Grpc));
    }

    #[test]
    fn test_detect_zmtp_greeting() {
        // ZMTP greeting: 0xFF, padding[1-8], 0x7F at byte 9, then version info
        let mut data = vec![0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7F];
        data.extend_from_slice(&[3, 0]); // version 3.0
        let stream = make_stream(data, 5555);
        assert_eq!(detect_protocol(&stream), Some(DetectedProtocol::Zmtp));
    }

    #[test]
    fn test_detect_rtps_magic() {
        let data = b"RTPS\x02\x03\x01\x0f".to_vec(); // RTPS 2.3 header
        let stream = make_stream(data, 7400);
        assert_eq!(detect_protocol(&stream), Some(DetectedProtocol::Rtps));
    }

    #[test]
    fn test_detect_port_fallback() {
        let data = vec![0x00; 32]; // Random data
        let stream = make_stream(data, 50051);
        assert_eq!(detect_protocol(&stream), Some(DetectedProtocol::Grpc));
    }

    #[test]
    fn test_detect_unknown() {
        let data = vec![0x42; 32]; // Random data, non-gRPC port
        let stream = make_stream(data, 12345);
        assert_eq!(detect_protocol(&stream), None);
    }
}
