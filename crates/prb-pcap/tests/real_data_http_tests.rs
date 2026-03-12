//! Real-world HTTP/1.x and WebSocket capture integration tests.
//!
//! These tests validate the full pipeline with real HTTP/1.x pcap files, focusing on robustness.

use prb_core::{CaptureAdapter, TransportKind};
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
fn test_real_data_http11_basic() {
    // Use http-chunked-gzip.pcap as basic HTTP test since http.cap may not be available
    let capture_path = fixtures_dir().join("http/http-chunked-gzip.pcap");
    if !capture_path.exists() {
        // Try http.cap as fallback
        let fallback_path = fixtures_dir().join("http/http.cap");
        if !fallback_path.exists() {
            return;
        }
    }

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    let stats = adapter.stats();
    assert!(stats.packets_read > 0, "Should read packets");
    assert!(stats.tcp_streams > 0, "Should reassemble streams");
    assert!(!events.is_empty(), "Should produce events");

    // Some events should succeed
    let ok_count = events.iter().filter(|r| r.is_ok()).count();
    assert!(ok_count > 0, "Should have successful events");
}

#[test]
fn test_real_data_http_chunked_gzip() {
    let capture_path = fixtures_dir().join("http/http-chunked-gzip.pcap");
    assert!(capture_path.exists(), "HTTP chunked+gzip fixture required");

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    let stats = adapter.stats();
    assert!(stats.packets_read > 0, "Should read packets");
    assert!(stats.tcp_streams > 0, "Should reassemble streams");
    assert!(!events.is_empty(), "Should produce events");

    // Chunked transfer encoding should not crash the pipeline
    let ok_count = events.iter().filter(|r| r.is_ok()).count();
    assert!(ok_count > 0, "Should handle chunked transfer");
}

#[test]
fn test_real_data_http_large_payload() {
    let capture_path = fixtures_dir().join("http/http_with_jpegs.cap");
    assert!(capture_path.exists(), "HTTP large payload fixture required");

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    let stats = adapter.stats();
    assert!(stats.packets_read > 0, "Should read packets");
    assert!(stats.tcp_streams > 0, "Should reassemble streams");
    assert!(!events.is_empty(), "Should produce events");

    // Large payloads should not crash
    let ok_count = events.iter().filter(|r| r.is_ok()).count();
    assert!(ok_count > 0, "Should handle large payloads");
}

#[test]
fn test_real_data_websocket_handshake() {
    let capture_path = fixtures_dir().join("websocket/websocket.pcap");
    if !capture_path.exists() {
        // WebSocket captures may not be available
        return;
    }

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    let stats = adapter.stats();
    assert!(stats.packets_read > 0, "Should read packets");
    assert!(!events.is_empty(), "Should produce events");

    // Should handle WebSocket upgrade without crashing
    let ok_count = events.iter().filter(|r| r.is_ok()).count();
    assert!(ok_count > 0, "Should handle WebSocket handshake");
}

#[test]
fn test_real_data_http_no_panic_comprehensive() {
    // Process all available HTTP captures - should never panic
    let mut tested = 0;

    let http_dir = fixtures_dir().join("http");
    if http_dir.exists()
        && let Ok(entries) = std::fs::read_dir(http_dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("pcap")
                || path.extension().and_then(|s| s.to_str()) == Some("cap")
                || path.extension().and_then(|s| s.to_str()) == Some("pcapng")
            {
                let mut adapter = PcapCaptureAdapter::new(path.clone(), None);
                let _events: Vec<_> = adapter.ingest().collect();
                tested += 1;
            }
        }
    }

    assert!(
        tested >= 2,
        "Should test at least 2 HTTP captures, tested {}",
        tested
    );
}

#[test]
fn test_real_data_http_pipeline_stages() {
    // Use http_with_jpegs.cap as it's a verified working capture
    let path = fixtures_dir().join("http/http_with_jpegs.cap");
    if !path.exists() {
        return;
    }

    let mut adapter = PcapCaptureAdapter::new(path, None);
    let events: Vec<_> = adapter.ingest().collect();

    let stats = adapter.stats();
    assert!(stats.packets_read > 0);
    assert!(stats.tcp_streams > 0);
    assert!(!events.is_empty());

    // Check for TCP or HTTP events
    let tcp_events = events
        .iter()
        .filter_map(|r| r.as_ref().ok())
        .filter(|e| e.transport == TransportKind::RawTcp)
        .count();

    assert!(tcp_events > 0, "Should produce TCP events");
}

#[test]
fn test_real_data_http_multiple_requests() {
    // Test that multiple HTTP requests on same connection are handled
    // Use http_with_jpegs.cap which has multiple image transfers
    let path = fixtures_dir().join("http/http_with_jpegs.cap");
    if !path.exists() {
        return;
    }

    let mut adapter = PcapCaptureAdapter::new(path, None);
    let events: Vec<_> = adapter.ingest().collect();

    let stats = adapter.stats();
    assert!(stats.packets_read > 0);
    assert!(stats.tcp_streams > 0);

    // Should produce multiple events from the capture
    let ok_count = events.iter().filter(|r| r.is_ok()).count();
    assert!(ok_count >= 2, "Should handle multiple HTTP requests");
}

#[test]
fn test_real_data_websocket_robustness() {
    // Test all WebSocket captures if available
    let websocket_dir = fixtures_dir().join("websocket");
    if !websocket_dir.exists() {
        return;
    }

    if let Ok(entries) = std::fs::read_dir(websocket_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("pcap")
                || path.extension().and_then(|s| s.to_str()) == Some("pcapng")
            {
                let mut adapter = PcapCaptureAdapter::new(path, None);
                let _events: Vec<_> = adapter.ingest().collect();
            }
        }
    }

    // WebSocket fixtures are optional, so don't assert minimum count
    // The test passes if it doesn't panic
}
