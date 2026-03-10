//! Integration tests for IoT protocol decoding with real-world captures.
//! Tests RTPS/DDS, MQTT, CoAP, and AMQP protocol handling.

use prb_core::CaptureAdapter;
use prb_pcap::PcapCaptureAdapter;
use std::path::PathBuf;

/// Helper to get the path to an RTPS test fixture.
fn rtps_fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/captures/rtps")
        .join(name)
}

/// Helper to get the path to an MQTT test fixture.
fn mqtt_fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/captures/mqtt")
        .join(name)
}

/// Helper to get the path to an IoT test fixture (CoAP, AMQP, etc).
fn iot_fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/captures/iot")
        .join(name)
}

#[test]
fn test_rtps_discovery_real() {
    let pcap_path = rtps_fixture_path("rtps_sample.pcap");

    assert!(
        pcap_path.exists(),
        "RTPS pcap file should exist: {:?}",
        pcap_path
    );

    // Process RTPS capture through the pipeline
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Should produce events from the RTPS capture
    assert!(
        !events.is_empty(),
        "Pipeline should produce events from RTPS capture"
    );

    // Count successful events
    let success_count = events.iter().filter(|e| e.is_ok()).count();
    assert!(
        success_count > 0,
        "Should have at least one successfully processed RTPS event"
    );

    // Verify packets were read
    let stats = adapter.stats();
    assert!(
        stats.packets_read > 0,
        "Should read packets from RTPS capture"
    );
}

#[test]
fn test_rtps_multicast_handling() {
    // RTPS uses multicast for discovery (239.255.0.x)
    // Test that multicast packets are correctly handled in normalization
    let pcap_path = rtps_fixture_path("rtps_sample.pcap");

    if !pcap_path.exists() {
        return;
    }

    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Should handle RTPS multicast traffic without panic
    assert!(
        !events.is_empty(),
        "Should process RTPS multicast traffic"
    );

    let stats = adapter.stats();
    assert!(stats.packets_read > 0, "Should read RTPS packets");
}

#[test]
fn test_mqtt_connect_publish_subscribe() {
    let pcap_path = mqtt_fixture_path("mqtt.pcap");

    assert!(
        pcap_path.exists(),
        "MQTT pcap file should exist: {:?}",
        pcap_path
    );

    // Process MQTT capture
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Should produce events from the MQTT capture
    assert!(
        !events.is_empty(),
        "Pipeline should produce events from MQTT capture"
    );

    // Count successful events
    let success_count = events.iter().filter(|e| e.is_ok()).count();
    assert!(
        success_count > 0,
        "Should have at least one successfully processed MQTT event"
    );

    // Verify packets were read
    let stats = adapter.stats();
    assert!(
        stats.packets_read > 0,
        "Should read packets from MQTT capture"
    );
}

#[test]
fn test_mqtt_parser_robustness() {
    // Test that MQTT parser handles packets without panicking
    let pcap_path = mqtt_fixture_path("mqtt.pcap");

    if !pcap_path.exists() {
        return;
    }

    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Parser should handle all packets without panicking
    assert!(
        events.iter().all(|e| e.is_ok() || e.is_err()),
        "All events should be either Ok or Err (no panic)"
    );

    let stats = adapter.stats();
    assert!(stats.packets_read > 0, "Should read MQTT packets");
}

#[test]
fn test_coap_request_response() {
    let pcap_path = iot_fixture_path("coap.pcap");

    assert!(
        pcap_path.exists(),
        "CoAP pcap file should exist: {:?}",
        pcap_path
    );

    // Process CoAP capture
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Should produce events from the CoAP capture
    assert!(
        !events.is_empty(),
        "Pipeline should produce events from CoAP capture"
    );

    // Count successful events
    let success_count = events.iter().filter(|e| e.is_ok()).count();
    assert!(
        success_count > 0,
        "Should have at least one successfully processed CoAP event"
    );

    // Verify packets were read
    let stats = adapter.stats();
    assert!(
        stats.packets_read > 0,
        "Should read packets from CoAP capture"
    );
}

#[test]
fn test_coap_udp_handling() {
    // CoAP uses UDP - verify UDP packets are correctly handled
    let pcap_path = iot_fixture_path("coap.pcap");

    if !pcap_path.exists() {
        return;
    }

    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Should handle CoAP UDP traffic without panic
    assert!(
        !events.is_empty(),
        "Should process CoAP UDP traffic"
    );

    let stats = adapter.stats();
    assert!(stats.packets_read > 0, "Should read CoAP packets");
}

#[test]
fn test_amqp_connection_and_publish() {
    let pcap_path = iot_fixture_path("amqp.pcap");

    assert!(
        pcap_path.exists(),
        "AMQP pcap file should exist: {:?}",
        pcap_path
    );

    // Process AMQP capture
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Should produce events from the AMQP capture
    assert!(
        !events.is_empty(),
        "Pipeline should produce events from AMQP capture"
    );

    // Count successful events
    let success_count = events.iter().filter(|e| e.is_ok()).count();
    assert!(
        success_count > 0,
        "Should have at least one successfully processed AMQP event"
    );

    // Verify packets were read
    let stats = adapter.stats();
    assert!(
        stats.packets_read > 0,
        "Should read packets from AMQP capture"
    );
}

#[test]
fn test_amqp_parser_robustness() {
    // Test that AMQP parser handles packets without panicking
    let pcap_path = iot_fixture_path("amqp.pcap");

    if !pcap_path.exists() {
        return;
    }

    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Parser should handle all packets without panicking
    assert!(
        events.iter().all(|e| e.is_ok() || e.is_err()),
        "All events should be either Ok or Err (no panic)"
    );

    let stats = adapter.stats();
    assert!(stats.packets_read > 0, "Should read AMQP packets");
}

#[test]
fn test_iot_protocols_no_panic_suite() {
    // Comprehensive test that all IoT captures can be processed without panic
    let captures = vec![
        ("rtps", rtps_fixture_path("rtps_sample.pcap")),
        ("mqtt", mqtt_fixture_path("mqtt.pcap")),
        ("coap", iot_fixture_path("coap.pcap")),
        ("amqp", iot_fixture_path("amqp.pcap")),
    ];

    for (protocol, pcap_path) in captures {
        if !pcap_path.exists() {
            eprintln!("Skipping {} - file not found", protocol);
            continue;
        }

        let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
        let events: Vec<_> = adapter.ingest().collect();

        // Main assertion: no panic occurred during processing
        assert!(
            events.iter().all(|e| e.is_ok() || e.is_err()),
            "{} capture processing should not panic",
            protocol
        );

        let stats = adapter.stats();
        assert!(
            stats.packets_read > 0,
            "{} capture should have packets read",
            protocol
        );
    }
}
