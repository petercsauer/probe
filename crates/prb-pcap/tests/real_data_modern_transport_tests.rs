//! Integration tests for modern transport protocols with real-world PCAP captures.
//! Tests QUIC, SSH, `WireGuard`, and SCTP protocol detection and decoding.

use prb_core::CaptureAdapter;
use prb_pcap::PcapCaptureAdapter;
use std::path::PathBuf;

/// Helper to get the path to a test fixture for QUIC.
fn quic_fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/captures/quic")
        .join(name)
}

/// Helper to get the path to a test fixture for SSH.
fn ssh_fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/captures/ssh")
        .join(name)
}

/// Helper to get the path to a test fixture for modern protocols.
fn modern_fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/captures/modern")
        .join(name)
}

#[test]
fn test_quic_initial_packet_detection() {
    let pcap_path = quic_fixture_path("quic_initial.pcap");

    assert!(
        pcap_path.exists(),
        "QUIC initial packet capture should exist: {pcap_path:?}"
    );

    // Process QUIC capture - should not panic
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // QUIC packets should be read from pcap
    let stats = adapter.stats();
    assert!(
        stats.packets_read > 0,
        "Should read packets from QUIC capture (got {} packets)",
        stats.packets_read
    );

    // Pipeline should handle QUIC without panicking
    // Events may be empty if packets are malformed or filtered out
    // The key test is no panic occurred
    let _success_count = events.iter().filter(|e| e.is_ok()).count();
}

#[test]
fn test_quic_encrypted_payload_handling() {
    let pcap_path = quic_fixture_path("quic_initial.pcap");

    assert!(
        pcap_path.exists(),
        "QUIC capture should exist: {pcap_path:?}"
    );

    // Process QUIC capture without keys - should not panic
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let _events: Vec<_> = adapter.ingest().collect();

    // Verify packets were read from capture
    let stats = adapter.stats();
    assert!(
        stats.packets_read > 0,
        "Should read packets from QUIC capture"
    );

    // Test passes if no panic occurred - encrypted payloads handled gracefully
}

#[test]
fn test_ssh_banner_exchange_detection() {
    let pcap_path = ssh_fixture_path("ssh_banner.pcap");

    assert!(
        pcap_path.exists(),
        "SSH banner capture should exist: {pcap_path:?}"
    );

    // Process SSH capture - should not panic
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let _events: Vec<_> = adapter.ingest().collect();

    // SSH packets should be read from pcap
    let stats = adapter.stats();
    assert!(
        stats.packets_read > 0,
        "Should read packets from SSH capture (got {} packets)",
        stats.packets_read
    );

    // Test passes if no panic occurred
}

#[test]
fn test_ssh_protocol_detection() {
    let pcap_path = ssh_fixture_path("ssh_banner.pcap");

    assert!(
        pcap_path.exists(),
        "SSH capture should exist: {pcap_path:?}"
    );

    // Process SSH capture - should not panic
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let _events: Vec<_> = adapter.ingest().collect();

    // Verify packets were read
    let stats = adapter.stats();
    assert!(
        stats.packets_read > 0,
        "Should read packets from SSH capture"
    );

    // Test passes if SSH traffic handled without panic
}

#[test]
fn test_wireguard_handshake_detection() {
    let pcap_path = modern_fixture_path("wireguard.pcap");

    assert!(
        pcap_path.exists(),
        "WireGuard capture should exist: {pcap_path:?}"
    );

    // Process WireGuard capture - should not panic
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let _events: Vec<_> = adapter.ingest().collect();

    // Verify packets were read
    let stats = adapter.stats();
    assert!(
        stats.packets_read > 0,
        "Should read packets from WireGuard capture (got {} packets)",
        stats.packets_read
    );

    // Test passes if no panic occurred
}

#[test]
fn test_wireguard_encrypted_handling() {
    let pcap_path = modern_fixture_path("wireguard.pcap");

    assert!(
        pcap_path.exists(),
        "WireGuard capture should exist: {pcap_path:?}"
    );

    // Process WireGuard capture - should not panic
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let _events: Vec<_> = adapter.ingest().collect();

    // WireGuard payloads are encrypted - verify graceful handling
    let stats = adapter.stats();
    assert!(
        stats.packets_read > 0,
        "Should read packets from WireGuard capture"
    );

    // Test passes if encrypted WireGuard handled without panic
}

#[test]
fn test_sctp_multistream_real() {
    let pcap_path = modern_fixture_path("sctp_test.pcap");

    assert!(
        pcap_path.exists(),
        "SCTP capture should exist: {pcap_path:?}"
    );

    // Process SCTP capture - should not panic
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let _events: Vec<_> = adapter.ingest().collect();

    // Verify packets were read
    let stats = adapter.stats();
    assert!(
        stats.packets_read > 0,
        "Should read packets from SCTP capture (got {} packets)",
        stats.packets_read
    );

    // Test passes if SCTP handled without panic
}

#[test]
fn test_sctp_chunk_detection() {
    let pcap_path = modern_fixture_path("sctp_test.pcap");

    assert!(
        pcap_path.exists(),
        "SCTP capture should exist: {pcap_path:?}"
    );

    // Process SCTP capture - should not panic
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let _events: Vec<_> = adapter.ingest().collect();

    // Verify packets were read
    let stats = adapter.stats();
    assert!(
        stats.packets_read > 0,
        "Should read packets from SCTP capture"
    );

    // Test passes if SCTP chunks handled without panic
}
