//! Real-world HTTP/2 and protocol capture integration tests.
//!
//! These tests validate the full pipeline with real pcap files, focusing on robustness.

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
fn test_real_data_http2_h2c_cleartext() {
    let capture_path = fixtures_dir().join("http2/http2-h2c.pcap");
    assert!(capture_path.exists(), "HTTP/2 h2c fixture required");

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
fn test_real_data_tcp_robustness() {
    let captures = vec![
        "tcp/dns-remoteshell.pcap",
        "tcp/tcp-ecn-sample.pcap",
        "tcp/200722_tcp_anon.pcapng",
    ];

    for capture in captures {
        let path = fixtures_dir().join(capture);
        if !path.exists() {
            continue;
        }

        let mut adapter = PcapCaptureAdapter::new(path.clone(), None);
        let events: Vec<_> = adapter.ingest().collect();

        let stats = adapter.stats();
        assert!(
            stats.packets_read > 0,
            "Capture {capture} should have packets"
        );
        assert!(
            !events.is_empty(),
            "Capture {capture} should produce events"
        );
    }
}

#[test]
fn test_real_data_ipv6() {
    let path = fixtures_dir().join("ip/v6.pcap");
    if !path.exists() {
        return;
    }

    let mut adapter = PcapCaptureAdapter::new(path, None);
    let events: Vec<_> = adapter.ingest().collect();

    let stats = adapter.stats();
    assert!(stats.packets_read > 0, "Should read IPv6 packets");
    assert!(!events.is_empty(), "Should handle IPv6");
}

#[test]
fn test_real_data_tls_without_keys() {
    let captures = vec!["tls/tls12.pcapng", "tls/tls13.pcapng"];

    for capture in captures {
        let path = fixtures_dir().join(capture);
        if !path.exists() {
            continue;
        }

        let mut adapter = PcapCaptureAdapter::new(path.clone(), None);
        let events: Vec<_> = adapter.ingest().collect();

        let stats = adapter.stats();
        assert!(
            stats.packets_read > 0,
            "Capture {capture} should have packets"
        );
        assert!(
            !events.is_empty(),
            "TLS without keys should still produce events"
        );
    }
}

#[test]
fn test_real_data_no_panic_comprehensive() {
    // Process all available captures - should never panic
    let mut tested = 0;

    for protocol_dir in &["tcp", "tls", "http2", "ip"] {
        let dir = fixtures_dir().join(protocol_dir);
        if !dir.exists() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("pcap")
                    || path.extension().and_then(|s| s.to_str()) == Some("pcapng")
                {
                    let mut adapter = PcapCaptureAdapter::new(path, None);
                    let _events: Vec<_> = adapter.ingest().collect();
                    tested += 1;
                }
            }
        }
    }

    assert!(
        tested >= 5,
        "Should test at least 5 real captures, tested {tested}"
    );
}

#[test]
fn test_real_data_pipeline_stages() {
    let path = fixtures_dir().join("http2/http2-h2c.pcap");
    assert!(path.exists(), "HTTP/2 fixture required");

    let mut adapter = PcapCaptureAdapter::new(path, None);
    let events: Vec<_> = adapter.ingest().collect();

    let stats = adapter.stats();
    assert!(stats.packets_read > 0);
    assert!(stats.tcp_streams > 0);
    assert!(!events.is_empty());

    // Check for TCP or Grpc events
    let tcp_or_grpc = events
        .iter()
        .filter_map(|r| r.as_ref().ok())
        .filter(|e| e.transport == TransportKind::RawTcp || e.transport == TransportKind::Grpc)
        .count();

    assert!(tcp_or_grpc > 0, "Should produce TCP or gRPC events");
}
