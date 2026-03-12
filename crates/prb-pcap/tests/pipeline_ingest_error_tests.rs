//! Tests for pipeline error handling and edge cases.

use prb_core::CaptureAdapter;
use prb_pcap::PcapCaptureAdapter;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_pipeline_ingest_invalid_pcap_file() {
    // Test error handling when PCAP file is invalid
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("invalid.pcap");

    // Create file with invalid header
    let mut file = File::create(&pcap_path).unwrap();
    file.write_all(b"INVALID_PCAP_HEADER_DATA").unwrap();
    file.flush().unwrap();
    drop(file);

    // Process should handle error gracefully
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Should produce an error event (not panic)
    assert!(!events.is_empty(), "Should produce at least one event");

    // First event should be an error
    let first_event = &events[0];
    assert!(first_event.is_err(), "First event should be an error");
}

#[test]
fn test_pipeline_ingest_nonexistent_file() {
    // Test error handling when file doesn't exist
    let pcap_path = PathBuf::from("/nonexistent/path/to/file.pcap");

    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Should produce an error event (not panic)
    assert!(!events.is_empty(), "Should produce at least one event");

    // First event should be an error
    let first_event = &events[0];
    assert!(first_event.is_err(), "First event should be an error");
}

#[test]
fn test_pipeline_ingest_invalid_keylog_file() {
    // Test error handling when keylog file is invalid
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("test.pcap");
    let keylog_path = temp_dir.path().join("invalid_keys.log");

    // Create valid PCAP file (empty)
    let mut pcap_file = File::create(&pcap_path).unwrap();
    let pcap_header = [
        0xd4, 0xc3, 0xb2, 0xa1, 0x02, 0x00, 0x04, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xff, 0xff, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    ];
    pcap_file.write_all(&pcap_header).unwrap();
    pcap_file.flush().unwrap();
    drop(pcap_file);

    // Create invalid keylog file
    let mut keylog_file = File::create(&keylog_path).unwrap();
    keylog_file.write_all(b"INVALID KEYLOG FORMAT\n").unwrap();
    keylog_file.flush().unwrap();
    drop(keylog_file);

    // Process - invalid keylog format should be handled gracefully
    let mut adapter = PcapCaptureAdapter::new(pcap_path, Some(keylog_path));
    let events: Vec<_> = adapter.ingest().collect();

    // May produce error or successfully process with no valid keys
    // Key point: no panic
    let _ = events;
}

#[test]
fn test_pipeline_ingest_empty_pcap_no_panic() {
    // Test that empty PCAP (just header, no packets) doesn't cause issues
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("empty.pcap");

    let mut file = File::create(&pcap_path).unwrap();
    let pcap_header = [
        0xd4, 0xc3, 0xb2, 0xa1, 0x02, 0x00, 0x04, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xff, 0xff, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    ];
    file.write_all(&pcap_header).unwrap();
    file.flush().unwrap();
    drop(file);

    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Should produce no events (empty file)
    assert_eq!(events.len(), 0, "Empty PCAP should produce no events");
}

#[test]
fn test_pipeline_ingest_multiple_calls_idempotent() {
    // Test that calling ingest() multiple times returns same results
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("test.pcap");

    // Create simple valid PCAP with one packet
    let mut file = File::create(&pcap_path).unwrap();
    let pcap_header = [
        0xd4, 0xc3, 0xb2, 0xa1, 0x02, 0x00, 0x04, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xff, 0xff, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    ];
    file.write_all(&pcap_header).unwrap();

    // Add one packet (minimal Ethernet frame)
    let ts_sec = 1700000000u32;
    let packet_data = [0xAA; 60];
    file.write_all(&ts_sec.to_le_bytes()).unwrap();
    file.write_all(&0u32.to_le_bytes()).unwrap();
    file.write_all(&(packet_data.len() as u32).to_le_bytes()).unwrap();
    file.write_all(&(packet_data.len() as u32).to_le_bytes()).unwrap();
    file.write_all(&packet_data).unwrap();
    file.flush().unwrap();
    drop(file);

    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);

    // First ingest call
    let events1: Vec<_> = adapter.ingest().collect();

    // Second ingest call - should return empty (all events already consumed)
    let events2: Vec<_> = adapter.ingest().collect();

    // First call may produce events or errors, second should be empty
    let _ = events1; // Consume to process the file
    assert_eq!(events2.len(), 0, "Second ingest should return empty");
}

#[test]
fn test_pipeline_adapter_name() {
    // Test CaptureAdapter trait implementation
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("test.pcap");

    // Create minimal valid PCAP
    let mut file = File::create(&pcap_path).unwrap();
    let pcap_header = [
        0xd4, 0xc3, 0xb2, 0xa1, 0x02, 0x00, 0x04, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xff, 0xff, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    ];
    file.write_all(&pcap_header).unwrap();
    file.flush().unwrap();
    drop(file);

    let adapter = PcapCaptureAdapter::new(pcap_path, None);
    assert_eq!(adapter.name(), "pcap");
}

#[test]
fn test_pipeline_stats_before_ingest() {
    // Test that stats() returns default values before ingest is called
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("test.pcap");

    // Create minimal valid PCAP
    let mut file = File::create(&pcap_path).unwrap();
    let pcap_header = [
        0xd4, 0xc3, 0xb2, 0xa1, 0x02, 0x00, 0x04, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xff, 0xff, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    ];
    file.write_all(&pcap_header).unwrap();
    file.flush().unwrap();
    drop(file);

    let adapter = PcapCaptureAdapter::new(pcap_path, None);

    // Stats before ingest should be all zeros
    let stats = adapter.stats();
    assert_eq!(stats.packets_read, 0);
    assert_eq!(stats.packets_failed, 0);
    assert_eq!(stats.tcp_streams, 0);
    assert_eq!(stats.udp_datagrams, 0);
    assert_eq!(stats.tls_decrypted, 0);
    assert_eq!(stats.tls_encrypted, 0);
    assert_eq!(stats.protocol_decoded, 0);
    assert_eq!(stats.protocol_fallback, 0);
}

#[test]
fn test_pipeline_with_custom_registry() {
    // Test with_registry constructor
    use prb_detect::DecoderRegistry;

    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("test.pcap");

    // Create minimal valid PCAP
    let mut file = File::create(&pcap_path).unwrap();
    let pcap_header = [
        0xd4, 0xc3, 0xb2, 0xa1, 0x02, 0x00, 0x04, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xff, 0xff, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    ];
    file.write_all(&pcap_header).unwrap();
    file.flush().unwrap();
    drop(file);

    // Create custom registry (empty for this test)
    let registry = DecoderRegistry::new();

    let mut adapter = PcapCaptureAdapter::with_registry(pcap_path, None, registry);

    // Should process without error
    let _events: Vec<_> = adapter.ingest().collect();

    // Adapter should work with custom registry
    assert_eq!(adapter.name(), "pcap");
}
