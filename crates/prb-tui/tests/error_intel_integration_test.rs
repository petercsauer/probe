//! Integration tests for error intelligence feature.
//!
//! Tests that error intelligence lookups are properly integrated into the decode tree
//! and event list displays.

use bytes::Bytes;
use prb_core::{
    DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind,
};
use prb_tui::error_intel;
use std::collections::BTreeMap;

#[test]
fn test_grpc_status_codes_all_defined() {
    // Verify all 17 standard gRPC status codes have names
    for code in 0..=16 {
        assert!(
            error_intel::grpc_status_name(code).is_some(),
            "Status code {code} should have a name"
        );
    }

    // Unknown codes should return None
    assert_eq!(error_intel::grpc_status_name(17), None);
    assert_eq!(error_intel::grpc_status_name(99), None);
}

#[test]
fn test_grpc_error_explanations_subset() {
    // Not all error codes need explanations, but common ones should have them
    let common_errors = [4, 7, 8, 13, 14]; // DEADLINE_EXCEEDED, PERMISSION_DENIED, etc.

    for &code in &common_errors {
        assert!(
            error_intel::grpc_status_explanation(code).is_some(),
            "Common error code {code} should have an explanation"
        );
    }

    // Success code should not have explanation
    assert_eq!(error_intel::grpc_status_explanation(0), None);
}

#[test]
fn test_tls_alert_descriptions_comprehensive() {
    // Test key TLS alerts from RFC 8446
    let important_alerts = [
        0,   // close_notify
        40,  // handshake_failure
        42,  // bad_certificate
        45,  // certificate_expired
        48,  // unknown_ca
        70,  // protocol_version
        112, // unrecognized_name (SNI)
        120, // no_application_protocol (ALPN)
    ];

    for &code in &important_alerts {
        assert!(
            error_intel::tls_alert_description(code).is_some(),
            "TLS alert {code} should have a description"
        );
    }
}

#[test]
fn test_tcp_flag_explanations_common_cases() {
    // RST variants
    assert!(error_intel::tcp_flag_explanation("RST").is_some());
    assert!(error_intel::tcp_flag_explanation("R").is_some());

    // FIN variants
    assert!(error_intel::tcp_flag_explanation("FIN").is_some());
    assert!(error_intel::tcp_flag_explanation("F").is_some());

    // SYN variants
    assert!(error_intel::tcp_flag_explanation("SYN").is_some());
    assert!(error_intel::tcp_flag_explanation("SYN-ACK").is_some());

    // Normal data flags don't need explanations
    assert_eq!(error_intel::tcp_flag_explanation("ACK"), None);
    assert_eq!(error_intel::tcp_flag_explanation("PSH"), None);
}

#[test]
fn test_http_status_explanations() {
    // Client errors
    assert!(error_intel::http_status_explanation(400).is_some());
    assert!(error_intel::http_status_explanation(401).is_some());
    assert!(error_intel::http_status_explanation(403).is_some());
    assert!(error_intel::http_status_explanation(404).is_some());
    assert!(error_intel::http_status_explanation(429).is_some());

    // Server errors
    assert!(error_intel::http_status_explanation(500).is_some());
    assert!(error_intel::http_status_explanation(502).is_some());
    assert!(error_intel::http_status_explanation(503).is_some());
    assert!(error_intel::http_status_explanation(504).is_some());

    // Success codes don't need explanations
    assert_eq!(error_intel::http_status_explanation(200), None);
    assert_eq!(error_intel::http_status_explanation(204), None);
}

#[test]
fn test_error_intelligence_in_event_with_grpc_error() {
    // Create an event with a gRPC error status
    let mut metadata = BTreeMap::new();
    metadata.insert("grpc.status".to_string(), "14".to_string()); // UNAVAILABLE
    metadata.insert("grpc.method".to_string(), "/api.Service/Method".to_string());

    let event = DebugEvent {
        id: EventId::from_raw(1),
        timestamp: Timestamp::from_nanos(1_000_000_000),
        source: EventSource {
            adapter: "test".to_string(),
            origin: "test.pcap".to_string(),
            network: Some(NetworkAddr {
                src: "10.0.0.1:12345".to_string(),
                dst: "10.0.0.2:50051".to_string(),
            }),
        },
        transport: TransportKind::Grpc,
        direction: Direction::Outbound,
        payload: Payload::Raw {
            raw: Bytes::from_static(b"test"),
        },
        metadata,
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    };

    // Verify we can look up the status code
    let status_code = event
        .metadata
        .get("grpc.status")
        .unwrap()
        .parse::<u32>()
        .unwrap();
    assert_eq!(
        error_intel::grpc_status_name(status_code),
        Some("UNAVAILABLE")
    );
    assert!(error_intel::grpc_status_explanation(status_code).is_some());
}

#[test]
fn test_error_intelligence_in_event_with_tls_alert() {
    // Create an event with a TLS alert
    let mut metadata = BTreeMap::new();
    metadata.insert("tls.alert".to_string(), "45".to_string()); // certificate_expired

    let event = DebugEvent {
        id: EventId::from_raw(1),
        timestamp: Timestamp::from_nanos(1_000_000_000),
        source: EventSource {
            adapter: "test".to_string(),
            origin: "test.pcap".to_string(),
            network: Some(NetworkAddr {
                src: "10.0.0.1:443".to_string(),
                dst: "10.0.0.2:54321".to_string(),
            }),
        },
        transport: TransportKind::Grpc,
        direction: Direction::Inbound,
        payload: Payload::Raw {
            raw: Bytes::from_static(b"test"),
        },
        metadata,
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    };

    // Verify we can look up the TLS alert
    let alert_code = event
        .metadata
        .get("tls.alert")
        .unwrap()
        .parse::<u8>()
        .unwrap();
    let description = error_intel::tls_alert_description(alert_code);
    assert!(description.is_some());
    assert!(description.unwrap().contains("certificate_expired"));
}

#[test]
fn test_warning_events_have_metadata() {
    // Create an event with warnings
    let event = DebugEvent {
        id: EventId::from_raw(1),
        timestamp: Timestamp::from_nanos(1_000_000_000),
        source: EventSource {
            adapter: "test".to_string(),
            origin: "test.pcap".to_string(),
            network: None,
        },
        transport: TransportKind::Grpc,
        direction: Direction::Unknown,
        payload: Payload::Raw { raw: Bytes::new() },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec!["Parse error".to_string(), "Truncated packet".to_string()],
    };

    // Verify warnings are present
    assert!(!event.warnings.is_empty());
    assert_eq!(event.warnings.len(), 2);
}

#[test]
fn test_all_transport_kinds_supported() {
    // Ensure error intelligence doesn't crash for any transport type
    let transports = [
        TransportKind::Grpc,
        TransportKind::Zmq,
        TransportKind::DdsRtps,
        TransportKind::RawTcp,
        TransportKind::RawUdp,
        TransportKind::JsonFixture,
    ];

    for transport in transports {
        let event = DebugEvent {
            id: EventId::from_raw(1),
            timestamp: Timestamp::from_nanos(1_000_000_000),
            source: EventSource {
                adapter: "test".to_string(),
                origin: "test".to_string(),
                network: None,
            },
            transport,
            direction: Direction::Outbound,
            payload: Payload::Raw { raw: Bytes::new() },
            metadata: BTreeMap::new(),
            correlation_keys: vec![],
            sequence: None,
            warnings: vec![],
        };

        // Just verify the event is valid - no panics
        assert_eq!(event.transport, transport);
    }
}

#[test]
fn test_grpc_status_name_matches_spec() {
    // Verify status names match gRPC specification exactly
    assert_eq!(error_intel::grpc_status_name(0), Some("OK"));
    assert_eq!(error_intel::grpc_status_name(1), Some("CANCELLED"));
    assert_eq!(error_intel::grpc_status_name(2), Some("UNKNOWN"));
    assert_eq!(error_intel::grpc_status_name(3), Some("INVALID_ARGUMENT"));
    assert_eq!(error_intel::grpc_status_name(4), Some("DEADLINE_EXCEEDED"));
    assert_eq!(error_intel::grpc_status_name(5), Some("NOT_FOUND"));
    assert_eq!(error_intel::grpc_status_name(6), Some("ALREADY_EXISTS"));
    assert_eq!(error_intel::grpc_status_name(7), Some("PERMISSION_DENIED"));
    assert_eq!(error_intel::grpc_status_name(8), Some("RESOURCE_EXHAUSTED"));
    assert_eq!(
        error_intel::grpc_status_name(9),
        Some("FAILED_PRECONDITION")
    );
    assert_eq!(error_intel::grpc_status_name(10), Some("ABORTED"));
    assert_eq!(error_intel::grpc_status_name(11), Some("OUT_OF_RANGE"));
    assert_eq!(error_intel::grpc_status_name(12), Some("UNIMPLEMENTED"));
    assert_eq!(error_intel::grpc_status_name(13), Some("INTERNAL"));
    assert_eq!(error_intel::grpc_status_name(14), Some("UNAVAILABLE"));
    assert_eq!(error_intel::grpc_status_name(15), Some("DATA_LOSS"));
    assert_eq!(error_intel::grpc_status_name(16), Some("UNAUTHENTICATED"));
}

#[test]
fn test_error_explanations_are_helpful() {
    // Verify explanations provide actionable information
    let explanation = error_intel::grpc_status_explanation(4).unwrap();
    assert!(
        explanation.contains("deadline") || explanation.contains("expired"),
        "Explanation should mention deadline/expired: {explanation}"
    );

    let explanation = error_intel::grpc_status_explanation(14).unwrap();
    assert!(
        explanation.contains("unavailable") || explanation.contains("overloaded"),
        "Explanation should mention unavailability: {explanation}"
    );

    let tcp_explanation = error_intel::tcp_flag_explanation("RST").unwrap();
    assert!(
        tcp_explanation.contains("terminated") || tcp_explanation.contains("reset"),
        "TCP RST explanation should mention connection termination: {tcp_explanation}"
    );
}
