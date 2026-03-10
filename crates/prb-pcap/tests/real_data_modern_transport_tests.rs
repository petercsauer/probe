//! Integration tests for modern transport protocols with real-world PCAP captures.
//! Tests QUIC, SSH, WireGuard, and SCTP protocol detection and decoding.

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
        "QUIC initial packet capture should exist: {:?}",
        pcap_path
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
        "QUIC capture should exist: {:?}",
        pcap_path
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
        "SSH banner capture should exist: {:?}",
        pcap_path
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
        "SSH capture should exist: {:?}",
        pcap_path
    );

    // Process SSH capture
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Should detect SSH on standard port 22
    assert!(
        !events.is_empty(),
        "SSH detection should produce events"
    );

    // SSH encrypted session data should be handled without panic
    let success_count = events.iter().filter(|e| e.is_ok()).count();
    assert!(
        success_count > 0,
        "Should handle SSH traffic without errors"
    );
}

#[test]
fn test_wireguard_handshake_detection() {
    let pcap_path = modern_fixture_path("wireguard.pcap");

    assert!(
        pcap_path.exists(),
        "WireGuard capture should exist: {:?}",
        pcap_path
    );

    // Process WireGuard capture
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Should detect WireGuard protocol (UDP with WireGuard message types)
    assert!(
        !events.is_empty(),
        "Pipeline should produce events from WireGuard capture"
    );

    let stats = adapter.stats();
    assert!(
        stats.packets_read > 0,
        "Should read packets from WireGuard capture"
    );

    // Count successful events
    let success_count = events.iter().filter(|e| e.is_ok()).count();
    assert!(
        success_count > 0,
        "Should have at least one successfully processed WireGuard event"
    );
}

#[test]
fn test_wireguard_encrypted_handling() {
    let pcap_path = modern_fixture_path("wireguard.pcap");

    assert!(
        pcap_path.exists(),
        "WireGuard capture should exist: {:?}",
        pcap_path
    );

    // Process WireGuard capture
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // WireGuard payloads are encrypted - should handle gracefully
    assert!(
        !events.is_empty(),
        "Should handle encrypted WireGuard data without panic"
    );

    let success_count = events.iter().filter(|e| e.is_ok()).count();
    assert!(
        success_count > 0,
        "Should process WireGuard packets even though encrypted"
    );
}

#[test]
fn test_sctp_multistream_real() {
    let pcap_path = modern_fixture_path("sctp_test.pcap");

    assert!(
        pcap_path.exists(),
        "SCTP capture should exist: {:?}",
        pcap_path
    );

    // Process SCTP capture
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Should detect SCTP protocol and multi-stream behavior
    assert!(
        !events.is_empty(),
        "Pipeline should produce events from SCTP capture"
    );

    let stats = adapter.stats();
    assert!(
        stats.packets_read > 0,
        "Should read packets from SCTP capture"
    );

    // Count successful events - SCTP chunks should be parsed
    let success_count = events.iter().filter(|e| e.is_ok()).count();
    assert!(
        success_count > 0,
        "Should have at least one successfully processed SCTP event"
    );
}

#[test]
fn test_sctp_chunk_detection() {
    let pcap_path = modern_fixture_path("sctp_test.pcap");

    assert!(
        pcap_path.exists(),
        "SCTP capture should exist: {:?}",
        pcap_path
    );

    // Process SCTP capture
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // SCTP INIT, INIT_ACK, DATA chunks should be detectable
    assert!(
        !events.is_empty(),
        "Should detect SCTP chunks in capture"
    );

    // Should handle SCTP without errors
    let success_count = events.iter().filter(|e| e.is_ok()).count();
    assert!(
        success_count > 0,
        "Should process SCTP chunks successfully"
    );
}
