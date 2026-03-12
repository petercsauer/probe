//! Integration tests for `CaptureEngine`.

use prb_capture::{CaptureConfig, CaptureEngine};

#[test]
fn test_capture_engine_creation() {
    // Test that we can create a capture engine
    let config = CaptureConfig::new("lo0");
    let engine = CaptureEngine::new(config);

    // Should have no receiver before starting
    assert!(engine.receiver().is_none());
}

#[test]
fn test_capture_engine_with_filter_config() {
    // Test creating engine with filter configuration
    let config = CaptureConfig::new("eth0").with_filter("tcp port 443");
    let engine = CaptureEngine::new(config);

    assert!(engine.receiver().is_none());
}

#[test]
fn test_capture_engine_with_custom_snaplen() {
    // Test creating engine with custom snaplen
    let config = CaptureConfig::new("wlan0")
        .with_snaplen(1500)
        .with_promisc(true);
    let _engine = CaptureEngine::new(config);

    // Should succeed without starting
}

#[test]
fn test_capture_engine_stats_before_start() {
    // Test that we can get stats before starting
    let config = CaptureConfig::new("lo0");
    let engine = CaptureEngine::new(config);

    let stats = engine.stats();

    // All counters should be zero initially
    assert_eq!(stats.packets_received, 0);
    assert_eq!(stats.bytes_received, 0);
    assert_eq!(stats.packets_dropped_kernel, 0);
    assert_eq!(stats.packets_dropped_channel, 0);
    assert_eq!(stats.total_drops(), 0);
}

#[test]
fn test_capture_engine_stop_without_start() {
    // Test that stopping an engine that hasn't been started is safe
    let config = CaptureConfig::new("lo0");
    let mut engine = CaptureEngine::new(config);

    let result = engine.stop();

    // Should succeed and return stats
    assert!(result.is_ok());
    let stats = result.unwrap();
    assert_eq!(stats.packets_received, 0);
}

#[test]
fn test_capture_engine_receiver_before_start() {
    // Test that receiver is None before start
    let config = CaptureConfig::new("lo0");
    let engine = CaptureEngine::new(config);

    assert!(engine.receiver().is_none());
}

#[test]
fn test_capture_engine_start_without_privileges() {
    // Test that starting without privileges returns proper error
    // This will fail on systems without pcap privileges
    let config = CaptureConfig::new("nonexistent_interface_xyz");
    let mut engine = CaptureEngine::new(config);

    let result = engine.start();

    // Should fail (either privilege error or interface not found)
    assert!(result.is_err());
}

#[test]
fn test_capture_engine_start_invalid_interface() {
    // Test that starting with invalid interface returns error
    let config = CaptureConfig::new("this_interface_does_not_exist_12345");
    let mut engine = CaptureEngine::new(config);

    let result = engine.start();

    // Should return an error
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_msg = err.to_string();

    // Error should mention the interface or privilege issue
    assert!(
        err_msg.contains("interface")
            || err_msg.contains("privileges")
            || err_msg.contains("device"),
        "error should mention interface or privileges: {}",
        err_msg
    );
}

#[test]
fn test_capture_engine_double_stop() {
    // Test that calling stop multiple times is safe
    let config = CaptureConfig::new("lo0");
    let mut engine = CaptureEngine::new(config);

    let result1 = engine.stop();
    assert!(result1.is_ok());

    let result2 = engine.stop();
    assert!(result2.is_ok());

    // Both should return zero stats
    assert_eq!(result1.unwrap().packets_received, 0);
    assert_eq!(result2.unwrap().packets_received, 0);
}

#[test]
fn test_capture_engine_with_buffer_size() {
    // Test creating engine with custom buffer size
    let config = CaptureConfig::new("lo0").with_buffer_size(4 * 1024 * 1024); // 4MB buffer

    let engine = CaptureEngine::new(config);

    // Should create successfully
    assert!(engine.receiver().is_none());
}

#[test]
fn test_capture_engine_with_promisc_mode() {
    // Test creating engine with promiscuous mode disabled
    let config = CaptureConfig::new("lo0").with_promisc(false);

    let _engine = CaptureEngine::new(config);

    // Should create successfully
}

#[test]
fn test_capture_engine_stats_drop_rate_calculation() {
    // Test the drop rate calculation in stats
    let config = CaptureConfig::new("lo0");
    let engine = CaptureEngine::new(config);

    let stats = engine.stats();

    // With zero packets, drop rate should be 0.0
    assert_eq!(stats.drop_rate(), 0.0);
}

#[test]
fn test_owned_packet_debug_format() {
    // Test the Debug trait implementation for OwnedPacket
    use prb_capture::OwnedPacket;

    let packet = OwnedPacket {
        timestamp_us: 1234567890,
        orig_len: 100,
        data: vec![0x01, 0x02, 0x03],
    };

    let debug_str = format!("{:?}", packet);

    // Should contain the fields
    assert!(debug_str.contains("timestamp_us"));
    assert!(debug_str.contains("orig_len"));
    assert!(debug_str.contains("data"));
}

#[test]
fn test_owned_packet_large_data() {
    // Test with large packet data
    use prb_capture::OwnedPacket;

    let large_data = vec![0xAB; 9000]; // Jumbo frame size
    let packet = OwnedPacket {
        timestamp_us: 9999999999,
        orig_len: 9000,
        data: large_data.clone(),
    };

    assert_eq!(packet.data.len(), 9000);
    assert_eq!(packet.orig_len, 9000);
    assert!(packet.data.iter().all(|&b| b == 0xAB));
}

#[test]
fn test_owned_packet_empty_data() {
    // Test with empty packet data
    use prb_capture::OwnedPacket;

    let packet = OwnedPacket {
        timestamp_us: 0,
        orig_len: 0,
        data: Vec::new(),
    };

    assert_eq!(packet.data.len(), 0);
    assert_eq!(packet.orig_len, 0);
}

#[test]
fn test_owned_packet_truncated() {
    // Test packet with orig_len > data.len() (truncated)
    use prb_capture::OwnedPacket;

    let packet = OwnedPacket {
        timestamp_us: 1000000,
        orig_len: 1500,        // Original frame was 1500 bytes
        data: vec![0xFF; 128], // But we only captured 128 bytes (snaplen)
    };

    assert_eq!(packet.data.len(), 128);
    assert_eq!(packet.orig_len, 1500);
    assert!(packet.orig_len > packet.data.len() as u32);
}

// Additional tests for error conditions

#[test]
fn test_capture_engine_start_with_invalid_filter() {
    // Test that invalid BPF filter is caught
    let config = CaptureConfig::new("lo0").with_filter("this is not a valid bpf filter!!!");

    let mut engine = CaptureEngine::new(config);

    // Try to start - should fail due to filter compilation error or privilege error
    let result = engine.start();

    // Either privilege error or filter error is acceptable
    if let Err(e) = result {
        let err_msg = e.to_string();
        // Error should mention either filter, privileges, or device
        assert!(
            err_msg.contains("filter")
                || err_msg.contains("privileges")
                || err_msg.contains("device"),
            "error should mention filter or privileges: {}",
            err_msg
        );
    }
}

#[test]
fn test_capture_engine_multiple_instances() {
    // Test creating multiple engine instances
    let config1 = CaptureConfig::new("lo0");
    let config2 = CaptureConfig::new("eth0");
    let config3 = CaptureConfig::new("wlan0");

    let _engine1 = CaptureEngine::new(config1);
    let _engine2 = CaptureEngine::new(config2);
    let _engine3 = CaptureEngine::new(config3);

    // All should create successfully without starting
}

#[test]
fn test_capture_engine_config_variations() {
    // Test various configuration combinations
    let configs = vec![
        CaptureConfig::new("lo0"),
        CaptureConfig::new("eth0").with_filter("tcp"),
        CaptureConfig::new("wlan0")
            .with_snaplen(256)
            .with_promisc(true),
        CaptureConfig::new("any")
            .with_filter("udp port 53")
            .with_buffer_size(8 * 1024 * 1024),
    ];

    for config in configs {
        let engine = CaptureEngine::new(config);
        assert!(engine.receiver().is_none());
    }
}
