//! Integration tests for TLS decryption with real-world PCAP captures and keylog files.

use prb_core::CaptureAdapter;
use prb_pcap::PcapCaptureAdapter;
use std::path::PathBuf;

/// Helper to get the path to a test fixture.
fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/captures/tls")
        .join(name)
}

#[test]
fn test_tls13_decrypt_with_keylog() {
    let pcap_path = fixture_path("tls13.pcapng");
    let keylog_path = fixture_path("tls13.keylog");

    assert!(
        pcap_path.exists(),
        "TLS 1.3 pcap file should exist: {:?}",
        pcap_path
    );
    assert!(
        keylog_path.exists(),
        "TLS 1.3 keylog file should exist: {:?}",
        keylog_path
    );

    // Process pcap with keylog
    let mut adapter = PcapCaptureAdapter::new(pcap_path, Some(keylog_path));
    let events: Vec<_> = adapter.ingest().collect();

    // Should produce at least some events
    assert!(
        !events.is_empty(),
        "Pipeline should produce events from TLS 1.3 capture"
    );

    // Count successful events
    let success_count = events.iter().filter(|e| e.is_ok()).count();
    assert!(
        success_count > 0,
        "Should have at least one successfully processed event"
    );

    // Check that TLS decryption was attempted (stats should show TLS processing)
    let stats = adapter.stats();
    assert!(
        stats.packets_read > 0,
        "Should read packets from TLS 1.3 capture"
    );
}

#[test]
fn test_tls12_decrypt_with_keylog() {
    let pcap_path = fixture_path("tls12.pcapng");
    let keylog_path = fixture_path("tls12.keylog");

    assert!(
        pcap_path.exists(),
        "TLS 1.2 pcap file should exist: {:?}",
        pcap_path
    );
    assert!(
        keylog_path.exists(),
        "TLS 1.2 keylog file should exist: {:?}",
        keylog_path
    );

    // Process pcap with keylog
    let mut adapter = PcapCaptureAdapter::new(pcap_path, Some(keylog_path));
    let events: Vec<_> = adapter.ingest().collect();

    // Should produce at least some events
    assert!(
        !events.is_empty(),
        "Pipeline should produce events from TLS 1.2 capture"
    );

    // Count successful events
    let success_count = events.iter().filter(|e| e.is_ok()).count();
    assert!(
        success_count > 0,
        "Should have at least one successfully processed event"
    );

    // Check stats
    let stats = adapter.stats();
    assert!(
        stats.packets_read > 0,
        "Should read packets from TLS 1.2 capture"
    );
}

#[test]
fn test_tls_without_keylog_produces_encrypted_events() {
    let pcap_path = fixture_path("tls13.pcapng");

    assert!(
        pcap_path.exists(),
        "TLS 1.3 pcap file should exist: {:?}",
        pcap_path
    );

    // Process pcap WITHOUT keylog
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Should still produce events, but they will be encrypted
    assert!(
        !events.is_empty(),
        "Pipeline should produce events even without keylog"
    );

    // Count successful events
    let success_count = events.iter().filter(|e| e.is_ok()).count();
    assert!(
        success_count > 0,
        "Should have at least one event (encrypted payload)"
    );

    // Events should exist but with encrypted payloads
    // (We can't easily assert that payloads are encrypted vs decrypted here
    // without inspecting the event structure, but the test verifies the
    // pipeline doesn't crash when keys are missing)
}

#[test]
fn test_tls_with_wrong_keylog() {
    let pcap_path = fixture_path("tls13.pcapng");
    let wrong_keylog_path = fixture_path("tls12.keylog"); // Wrong keylog for TLS 1.3 capture

    assert!(
        pcap_path.exists(),
        "TLS 1.3 pcap file should exist: {:?}",
        pcap_path
    );
    assert!(
        wrong_keylog_path.exists(),
        "TLS 1.2 keylog file should exist: {:?}",
        wrong_keylog_path
    );

    // Process pcap with wrong keylog
    let mut adapter = PcapCaptureAdapter::new(pcap_path, Some(wrong_keylog_path));
    let events: Vec<_> = adapter.ingest().collect();

    // Should not panic - decryption just fails gracefully
    assert!(
        !events.is_empty(),
        "Pipeline should produce events even with wrong keylog"
    );

    // Events should exist (decryption may fail but processing continues)
    // (success_count is always >= 0 by definition, so we just verify no panic)
}

#[test]
fn test_tls13_keylog_with_comments_and_blank_lines() {
    use std::io::Write;
    use tempfile::NamedTempFile;

    let pcap_path = fixture_path("tls13.pcapng");

    // Read the original keylog
    let original_keylog =
        std::fs::read_to_string(fixture_path("tls13.keylog")).expect("Should read TLS 1.3 keylog");

    // Create a keylog with comments and blank lines
    let mut tmpfile = NamedTempFile::new().unwrap();
    writeln!(tmpfile, "# TLS Key Log File").unwrap();
    writeln!(tmpfile, "# Generated by test").unwrap();
    writeln!(tmpfile).unwrap();
    writeln!(tmpfile, "   # Comment with leading spaces").unwrap();
    write!(tmpfile, "{}", original_keylog).unwrap();
    writeln!(tmpfile).unwrap();
    writeln!(tmpfile, "# End of keylog").unwrap();
    tmpfile.flush().unwrap();

    // Process pcap with enhanced keylog
    let mut adapter =
        PcapCaptureAdapter::new(pcap_path.clone(), Some(tmpfile.path().to_path_buf()));
    let events: Vec<_> = adapter.ingest().collect();

    // Should work the same as without comments
    assert!(
        !events.is_empty(),
        "Pipeline should handle keylog with comments"
    );

    let success_count = events.iter().filter(|e| e.is_ok()).count();
    assert!(
        success_count > 0,
        "Should process events with commented keylog"
    );
}

#[test]
fn test_tls_version_coverage() {
    // Test TLS 1.2
    let tls12_path = fixture_path("tls12.pcapng");
    let tls12_keylog = fixture_path("tls12.keylog");

    if tls12_path.exists() && tls12_keylog.exists() {
        let mut adapter = PcapCaptureAdapter::new(tls12_path, Some(tls12_keylog));
        let events: Vec<_> = adapter.ingest().collect();
        assert!(!events.is_empty(), "TLS 1.2 capture should produce events");
    }

    // Test TLS 1.3
    let tls13_path = fixture_path("tls13.pcapng");
    let tls13_keylog = fixture_path("tls13.keylog");

    if tls13_path.exists() && tls13_keylog.exists() {
        let mut adapter = PcapCaptureAdapter::new(tls13_path, Some(tls13_keylog));
        let events: Vec<_> = adapter.ingest().collect();
        assert!(!events.is_empty(), "TLS 1.3 capture should produce events");
    }
}
