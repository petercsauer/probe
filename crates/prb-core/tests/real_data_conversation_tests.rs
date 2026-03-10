//! Real-world capture tests for conversation reconstruction.
//!
//! Tests the conversation engine end-to-end with real protocol captures.

use prb_core::{
    CaptureAdapter, ConversationEngine, DebugEvent, TransportKind,
};
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

fn collect_ok_events(adapter: &mut PcapCaptureAdapter) -> Vec<DebugEvent> {
    adapter
        .ingest()
        .filter_map(|r| r.ok())
        .collect()
}

#[test]
fn test_http_conversation_reconstruction() {
    // Load HTTP capture → full pipeline → conversation engine
    let capture_path = fixtures_dir().join("http/http-chunked-gzip.pcap");
    assert!(
        capture_path.exists(),
        "HTTP capture required: {:?}",
        capture_path
    );

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);

    assert!(!events.is_empty(), "Should produce events from HTTP capture");

    // Note: ConversationEngine requires registered strategies
    // For this test, we're validating that events are produced and parseable
    // Actual conversation correlation requires protocol-specific strategies
    let engine = ConversationEngine::new();
    let result = engine.build_conversations(&events);

    // Even without strategies, should not crash
    assert!(result.is_ok(), "Conversation building should not fail");
}

#[test]
fn test_dns_conversation_reconstruction() {
    // Load DNS capture → pipeline → conversations
    let capture_path = fixtures_dir().join("dns/dns.pcap");
    assert!(
        capture_path.exists(),
        "DNS capture required: {:?}",
        capture_path
    );

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);

    assert!(!events.is_empty(), "Should produce events from DNS capture");

    // DNS events should have UDP transport
    let udp_events = events
        .iter()
        .filter(|e| e.transport == TransportKind::RawUdp)
        .count();
    assert!(udp_events > 0, "Should have UDP events for DNS");

    let engine = ConversationEngine::new();
    let result = engine.build_conversations(&events);
    assert!(result.is_ok(), "DNS conversation building should succeed");

    let conv_set = result.unwrap();
    // Without DNS correlation strategy, events go to fallback grouping
    assert!(
        !conv_set.conversations.is_empty(),
        "Should produce fallback conversations"
    );
}

#[test]
fn test_grpc_conversation_reconstruction() {
    // Load gRPC capture → pipeline → conversations
    let capture_path = fixtures_dir().join("grpc/grpc_person_search.pcapng");
    if !capture_path.exists() {
        // Skip if gRPC fixtures not downloaded yet
        return;
    }

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);

    assert!(!events.is_empty(), "Should produce events from gRPC capture");

    let engine = ConversationEngine::new();
    let result = engine.build_conversations(&events);
    assert!(result.is_ok(), "gRPC conversation building should succeed");
}

#[test]
fn test_multi_protocol_conversations() {
    // Load mixed-traffic capture (DNS has other traffic in some files)
    let capture_path = fixtures_dir().join("tcp/dns-remoteshell.pcap");
    if !capture_path.exists() {
        return;
    }

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);

    assert!(
        !events.is_empty(),
        "Should produce events from mixed capture"
    );

    // Should have multiple transport types
    let mut transports = std::collections::HashSet::new();
    for event in &events {
        transports.insert(event.transport);
    }

    assert!(
        transports.len() > 1,
        "Mixed capture should have multiple protocols"
    );

    let engine = ConversationEngine::new();
    let result = engine.build_conversations(&events);
    assert!(
        result.is_ok(),
        "Multi-protocol conversation building should succeed"
    );

    let conv_set = result.unwrap();
    // Conversations should be grouped by protocol
    let by_tcp = conv_set.by_protocol(TransportKind::RawTcp);
    let by_udp = conv_set.by_protocol(TransportKind::RawUdp);

    // Should have some TCP conversations (remote shell)
    // Should have some UDP conversations (DNS)
    assert!(
        by_tcp.len() + by_udp.len() > 0,
        "Should have conversations for different protocols"
    );
}

#[test]
fn test_conversation_timing_validity() {
    // Verify that reconstructed conversations have valid timing
    let capture_path = fixtures_dir().join("http/http-chunked-gzip.pcap");
    assert!(capture_path.exists());

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);
    assert!(!events.is_empty());

    let engine = ConversationEngine::new();
    let conv_set = engine.build_conversations(&events).unwrap();

    for conv in &conv_set.conversations {
        // Start time should be <= end time
        assert!(
            conv.metrics.start_time <= conv.metrics.end_time,
            "Conversation start time must be <= end time"
        );

        // Duration is unsigned, so it's always non-negative
        // Just verify it exists and can be accessed
        let _ = conv.metrics.duration_ns;
    }
}

#[test]
fn test_conversation_event_association() {
    // Verify that events are correctly associated with conversations
    let capture_path = fixtures_dir().join("dns/dns.pcap");
    assert!(capture_path.exists());

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);
    assert!(!events.is_empty());

    let engine = ConversationEngine::new();
    let conv_set = engine.build_conversations(&events).unwrap();

    // Each event should be in exactly one conversation
    let mut event_count = 0;
    for conv in &conv_set.conversations {
        event_count += conv.event_ids.len();
    }

    assert_eq!(
        event_count,
        events.len(),
        "All events should be assigned to conversations"
    );

    // Test lookup: each event should be findable via the index
    for event in &events {
        let found = conv_set.for_event(event.id);
        assert!(
            found.is_some(),
            "Each event should be findable in a conversation"
        );
    }
}

#[test]
fn test_smb_conversation_reconstruction() {
    // Test SMB/enterprise protocol conversation reconstruction
    let capture_path = fixtures_dir().join("smb/smb2-peter.pcap");
    assert!(
        capture_path.exists(),
        "SMB capture required: {:?}",
        capture_path
    );

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);

    assert!(!events.is_empty(), "Should produce events from SMB capture");

    let engine = ConversationEngine::new();
    let result = engine.build_conversations(&events);
    assert!(result.is_ok(), "SMB conversation building should succeed");

    let conv_set = result.unwrap();
    assert!(
        !conv_set.conversations.is_empty(),
        "Should produce conversations from SMB traffic"
    );
}

#[test]
fn test_conversation_stats_aggregation() {
    // Verify conversation statistics are computed correctly
    let capture_path = fixtures_dir().join("http/http_with_jpegs.cap");
    assert!(capture_path.exists());

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);
    assert!(!events.is_empty());

    let engine = ConversationEngine::new();
    let conv_set = engine.build_conversations(&events).unwrap();

    let stats = conv_set.stats();

    // Should have protocol counts
    assert!(
        stats.total > 0,
        "Should have at least one conversation"
    );

    // Should match conversation count
    assert_eq!(
        stats.total,
        conv_set.conversations.len(),
        "Stats total should match actual count"
    );
}

#[test]
fn test_sorted_conversations_by_time() {
    // Verify conversations can be sorted by start time
    let capture_path = fixtures_dir().join("dns/dns.pcap");
    assert!(capture_path.exists());

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);
    assert!(!events.is_empty());

    let engine = ConversationEngine::new();
    let conv_set = engine.build_conversations(&events).unwrap();

    let sorted = conv_set.sorted_by_time();

    // Verify sorted order
    for window in sorted.windows(2) {
        assert!(
            window[0].metrics.start_time <= window[1].metrics.start_time,
            "Conversations should be sorted by start time"
        );
    }
}
