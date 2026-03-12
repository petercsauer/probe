//! Unit tests for CaptureStats.

use prb_capture::CaptureStats;
use std::time::Duration;

#[test]
fn test_total_drops_calculation() {
    let stats = CaptureStats {
        packets_received: 1000,
        packets_dropped_kernel: 10,
        packets_dropped_channel: 5,
        bytes_received: 1500000,
        capture_duration: Duration::from_secs(10),
        packets_per_second: 100.0,
        bytes_per_second: 150000.0,
    };

    assert_eq!(stats.total_drops(), 15);
}

#[test]
fn test_drop_rate_with_drops() {
    let stats = CaptureStats {
        packets_received: 1000,
        packets_dropped_kernel: 10,
        packets_dropped_channel: 10,
        bytes_received: 1500000,
        capture_duration: Duration::from_secs(10),
        packets_per_second: 100.0,
        bytes_per_second: 150000.0,
    };

    let drop_rate = stats.drop_rate();
    assert!(
        (drop_rate - 0.02).abs() < 0.001,
        "expected ~0.02, got {}",
        drop_rate
    );

    let drop_pct = stats.drop_percentage();
    assert!(
        (drop_pct - 2.0).abs() < 0.1,
        "expected ~2.0%, got {}",
        drop_pct
    );
}

#[test]
fn test_drop_rate_with_no_packets() {
    let stats = CaptureStats {
        packets_received: 0,
        packets_dropped_kernel: 0,
        packets_dropped_channel: 0,
        bytes_received: 0,
        capture_duration: Duration::from_secs(10),
        packets_per_second: 0.0,
        bytes_per_second: 0.0,
    };

    assert_eq!(
        stats.drop_rate(),
        0.0,
        "drop rate should be 0 with no packets"
    );
    assert_eq!(stats.drop_percentage(), 0.0, "drop percentage should be 0");
}

#[test]
fn test_drop_rate_with_all_drops() {
    let stats = CaptureStats {
        packets_received: 100,
        packets_dropped_kernel: 50,
        packets_dropped_channel: 50,
        bytes_received: 150000,
        capture_duration: Duration::from_secs(1),
        packets_per_second: 100.0,
        bytes_per_second: 150000.0,
    };

    assert_eq!(stats.drop_rate(), 1.0, "drop rate should be 1.0");
    assert_eq!(
        stats.drop_percentage(),
        100.0,
        "drop percentage should be 100%"
    );
}

#[test]
fn test_stats_display_format() {
    let stats = CaptureStats {
        packets_received: 1500,
        packets_dropped_kernel: 10,
        packets_dropped_channel: 5,
        bytes_received: 2250000,
        capture_duration: Duration::from_secs(15),
        packets_per_second: 100.0,
        bytes_per_second: 150000.0,
    };

    let display = stats.to_string();

    // Check that key information is present
    assert!(display.contains("1500"), "should show packets received");
    assert!(display.contains("15"), "should show total drops");
    assert!(display.contains("kernel: 10"), "should show kernel drops");
    assert!(display.contains("channel: 5"), "should show channel drops");
    assert!(display.contains("pps"), "should show packets per second");
}

#[test]
fn test_stats_clone() {
    let stats = CaptureStats {
        packets_received: 1000,
        packets_dropped_kernel: 10,
        packets_dropped_channel: 5,
        bytes_received: 1500000,
        capture_duration: Duration::from_secs(10),
        packets_per_second: 100.0,
        bytes_per_second: 150000.0,
    };

    let cloned = stats.clone();
    assert_eq!(cloned.packets_received, stats.packets_received);
    assert_eq!(cloned.packets_dropped_kernel, stats.packets_dropped_kernel);
    assert_eq!(
        cloned.packets_dropped_channel,
        stats.packets_dropped_channel
    );
    assert_eq!(cloned.bytes_received, stats.bytes_received);
}
