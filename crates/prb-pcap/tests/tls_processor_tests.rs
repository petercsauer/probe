//! Tests for TLS stream processor coverage gaps.

use prb_pcap::tcp::{ReassembledStream, StreamDirection};
use prb_pcap::tls::{TlsKeyLog, TlsStreamProcessor};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;

#[test]
fn test_tls_processor_default() {
    // Test Default trait implementation
    let processor = TlsStreamProcessor::default();

    let stream = create_test_stream();
    let result = processor.decrypt_stream(stream);

    assert!(result.is_ok());
    let decrypted = result.unwrap();
    assert!(decrypted.encrypted); // No keys, should remain encrypted
}

#[test]
fn test_tls_processor_keylog_mut() {
    // Test keylog_mut() method
    let mut processor = TlsStreamProcessor::new();

    // Get mutable reference and add a key
    let keylog = processor.keylog_mut();
    keylog.insert(
        vec![0x01, 0x02, 0x03],
        prb_pcap::tls::keylog::KeyMaterial::MasterSecret(vec![0x04, 0x05, 0x06]),
    );

    // Verify the key was added by attempting decryption
    // (won't actually decrypt since we don't have valid TLS data, but tests the code path)
    let stream = create_test_stream();
    let result = processor.decrypt_stream(stream);
    assert!(result.is_ok());
}

#[test]
fn test_tls_processor_keylog_mut_with_shared_arc() {
    // Test keylog_mut() when Arc has multiple references (triggers clone)
    let keylog = Arc::new(TlsKeyLog::new());
    let mut processor1 = TlsStreamProcessor::with_keylog_ref(Arc::clone(&keylog));
    let _processor2 = TlsStreamProcessor::with_keylog_ref(Arc::clone(&keylog));

    // This should trigger Arc::make_mut to clone the keylog
    let keylog_mut = processor1.keylog_mut();
    keylog_mut.insert(
        vec![0x01, 0x02, 0x03],
        prb_pcap::tls::keylog::KeyMaterial::MasterSecret(vec![0x04, 0x05, 0x06]),
    );

    // Original keylog should remain unchanged
    assert_eq!(keylog.len(), 0);

    // processor1's keylog should have the new key
    let stream = create_test_stream();
    let result = processor1.decrypt_stream(stream);
    assert!(result.is_ok());
}

#[test]
fn test_tls_decrypt_stream_no_handshake() {
    // Test decrypt_stream with non-TLS data
    let processor = TlsStreamProcessor::new();

    let stream = ReassembledStream {
        src_ip: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        src_port: 12345,
        dst_ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
        dst_port: 443,
        direction: StreamDirection::ClientToServer,
        data: vec![0x00, 0x01, 0x02, 0x03], // Not TLS data
        is_complete: true,
        missing_ranges: vec![],
        timestamp_us: 1000,
    };

    let result = processor.decrypt_stream(stream);
    assert!(result.is_ok());

    let decrypted = result.unwrap();
    assert!(decrypted.encrypted); // No valid TLS handshake, should remain encrypted
    assert_eq!(decrypted.data, vec![0x00, 0x01, 0x02, 0x03]);
}

#[test]
fn test_tls_decrypt_stream_with_tls_header_no_keys() {
    // Test decrypt_stream with TLS-like data but no keys
    let processor = TlsStreamProcessor::new();

    let stream = ReassembledStream {
        src_ip: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        src_port: 12345,
        dst_ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
        dst_port: 443,
        direction: StreamDirection::ClientToServer,
        // TLS-like header (incomplete/invalid)
        data: vec![0x16, 0x03, 0x03, 0x00, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05],
        is_complete: true,
        missing_ranges: vec![],
        timestamp_us: 1000,
    };

    let result = processor.decrypt_stream(stream);
    assert!(result.is_ok());

    let decrypted = result.unwrap();
    assert!(decrypted.encrypted); // No keys, should remain encrypted
}

#[test]
fn test_tls_decrypt_stream_server_to_client() {
    // Test decrypt_stream with ServerToClient direction
    let processor = TlsStreamProcessor::new();

    let stream = ReassembledStream {
        src_ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
        src_port: 443,
        dst_ip: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        dst_port: 12345,
        direction: StreamDirection::ServerToClient,
        data: vec![0x16, 0x03, 0x03, 0x00, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05],
        is_complete: true,
        missing_ranges: vec![],
        timestamp_us: 1000,
    };

    let result = processor.decrypt_stream(stream);
    assert!(result.is_ok());

    let decrypted = result.unwrap();
    assert_eq!(decrypted.direction, StreamDirection::ServerToClient);
    assert!(decrypted.encrypted);
}

#[test]
fn test_tls_decrypt_stream_incomplete() {
    // Test decrypt_stream with incomplete stream
    let processor = TlsStreamProcessor::new();

    #[allow(clippy::single_range_in_vec_init)]
    let stream = ReassembledStream {
        src_ip: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        src_port: 12345,
        dst_ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
        dst_port: 443,
        direction: StreamDirection::ClientToServer,
        data: vec![0x16, 0x03, 0x03, 0x00, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05],
        is_complete: false, // Stream not complete
        missing_ranges: vec![100..200_u64],
        timestamp_us: 1000,
    };

    let result = processor.decrypt_stream(stream);
    assert!(result.is_ok());

    let decrypted = result.unwrap();
    assert!(!decrypted.is_complete); // Should preserve incomplete status
}

#[test]
fn test_decrypted_stream_fields() {
    // Test that all DecryptedStream fields are properly populated
    let processor = TlsStreamProcessor::new();

    let src_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
    let dst_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
    let timestamp_us = 123456789;

    let stream = ReassembledStream {
        src_ip,
        src_port: 12345,
        dst_ip,
        dst_port: 443,
        direction: StreamDirection::ClientToServer,
        data: vec![0x01, 0x02, 0x03],
        is_complete: true,
        missing_ranges: vec![],
        timestamp_us,
    };

    let result = processor.decrypt_stream(stream);
    assert!(result.is_ok());

    let decrypted = result.unwrap();
    assert_eq!(decrypted.src_ip, src_ip);
    assert_eq!(decrypted.src_port, 12345);
    assert_eq!(decrypted.dst_ip, dst_ip);
    assert_eq!(decrypted.dst_port, 443);
    assert_eq!(decrypted.timestamp_us, timestamp_us);
    assert!(decrypted.is_complete);
}

/// Helper function to create a test stream.
fn create_test_stream() -> ReassembledStream {
    ReassembledStream {
        src_ip: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        src_port: 12345,
        dst_ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
        dst_port: 443,
        direction: StreamDirection::ClientToServer,
        data: vec![0x16, 0x03, 0x03, 0x00, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05],
        is_complete: true,
        missing_ranges: vec![],
        timestamp_us: 1000,
    }
}
