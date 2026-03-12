//! Integration tests for LiveCaptureAdapter.

use prb_capture::{CaptureConfig, LiveCaptureAdapter};
use prb_core::CaptureAdapter;

#[test]
fn test_adapter_creation() {
    // Test that we can create an adapter
    let config = CaptureConfig::new("lo0");
    let adapter = LiveCaptureAdapter::new(config);
    assert!(adapter.is_ok());
}

#[test]
fn test_adapter_name() {
    // Test the adapter name is correct
    let config = CaptureConfig::new("lo0");
    let adapter = LiveCaptureAdapter::new(config).unwrap();
    assert_eq!(adapter.name(), "live-capture");
}

#[test]
fn test_adapter_not_started_error() {
    // Test that calling ingest() before start() returns an error
    let config = CaptureConfig::new("lo0");
    let mut adapter = LiveCaptureAdapter::new(config).unwrap();

    let mut iter = adapter.ingest();
    let result = iter.next();

    assert!(result.is_some());
    let event = result.unwrap();
    assert!(event.is_err());

    let err_msg = event.unwrap_err().to_string();
    assert!(
        err_msg.contains("not started"),
        "Expected 'not started' error, got: {}",
        err_msg
    );
}

#[test]
fn test_adapter_with_tls_keylog() {
    // Test that we can create an adapter with TLS keylog path
    use tempfile::NamedTempFile;

    let keylog_file = NamedTempFile::new().unwrap();
    let config = CaptureConfig::new("lo0").with_tls_keylog(keylog_file.path());

    let adapter = LiveCaptureAdapter::new(config);
    assert!(adapter.is_ok());
}

#[test]
fn test_adapter_with_filter() {
    // Test that we can create an adapter with BPF filter
    let config = CaptureConfig::new("lo0").with_filter("tcp port 443");

    let adapter = LiveCaptureAdapter::new(config);
    assert!(adapter.is_ok());
}

// Note: We don't test actual packet capture here because it requires:
// 1. Root/CAP_NET_RAW privileges
// 2. A specific network interface with traffic
// 3. Platform-specific behavior
//
// Manual testing can be done with:
// ```
// sudo cargo test --package prb-capture -- --ignored test_live_capture_loopback
// ```

#[test]
#[ignore = "requires root privileges and generates live traffic"]
fn test_live_capture_loopback() {
    // This test is ignored by default. Run with:
    // sudo cargo test --package prb-capture -- --ignored test_live_capture_loopback

    use std::thread;
    use std::time::Duration;

    let config = CaptureConfig::new("lo0").with_filter("icmp");

    let mut adapter = LiveCaptureAdapter::new(config).unwrap();

    // Start capture
    match adapter.start() {
        Ok(_) => {
            println!("Capture started on lo0");

            // Generate some loopback traffic in background
            thread::spawn(|| {
                thread::sleep(Duration::from_millis(100));
                // Ping localhost to generate ICMP traffic
                std::process::Command::new("ping")
                    .args(["-c", "3", "127.0.0.1"])
                    .output()
                    .ok();
            });

            // Collect events for a short time
            let mut count = 0;
            let start = std::time::Instant::now();

            for event in adapter.ingest() {
                match event {
                    Ok(evt) => {
                        println!("Captured event: {:?}", evt);
                        count += 1;
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        break;
                    }
                }

                // Stop after 5 seconds or 10 events
                if count >= 10 || start.elapsed() > Duration::from_secs(5) {
                    break;
                }
            }

            println!("Captured {} events", count);

            // Stop capture
            let stats = adapter.stop().unwrap();
            println!("Capture stats: {:?}", stats);

            // We expect to have captured at least some packets
            assert!(count > 0, "Should have captured at least one packet");
        }
        Err(e) => {
            eprintln!("Failed to start capture: {}", e);
            eprintln!("This test requires root privileges and a working loopback interface");
            panic!("Cannot run test without proper privileges");
        }
    }
}
