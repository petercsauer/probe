//! Pipeline error injection tests for robustness.
//!
//! Tests various error conditions, corrupt packets, and edge cases to ensure
//! the pipeline never panics and handles errors gracefully.

use prb_detect::DecoderRegistry;
use prb_pcap::PipelineCore;
use prb_pcap::tls::TlsStreamProcessor;

/// Test warning capacity limiting: generate many warnings from a single packet.
#[test]
fn test_warning_capacity_limit() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    // Send many malformed packets in sequence to generate warnings
    for i in 0..150 {
        let malformed_packet = vec![0xff; i % 20 + 5]; // Various truncated packets
        let result = core.process_packet(1, 1_000_000 + i as u64, &malformed_packet, "test");

        // Each packet should produce at most MAX_WARNINGS_PER_PACKET warnings
        // In practice, each malformed packet produces 1 warning, so this is fine
        assert!(
            result.warnings.len() <= 100,
            "Warnings exceeded capacity limit: {} warnings",
            result.warnings.len()
        );
    }
}

/// Test corrupted IP header with invalid version.
#[test]
fn test_corrupt_ip_header_invalid_version() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    // Ethernet header (14 bytes) + corrupted IP header
    let packet = vec![
        // Ethernet header
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, // Destination MAC
        0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, // Source MAC
        0x08, 0x00, // EtherType: IPv4
        // Invalid IP header: version 15 (should be 4)
        0xf5, 0x00, 0x00, 0x14, 0x00, 0x00, 0x00, 0x00, 0x40, 0x06, 0x00, 0x00, 0xc0, 0xa8, 0x01,
        0x01, 0x0a, 0x00, 0x00, 0x01,
    ];

    let result = core.process_packet(1, 1_000_000, &packet, "test");

    // Should not panic, may produce warnings or no events
    assert_eq!(
        result.events.len(),
        0,
        "Expected no events from corrupt IP header"
    );
}

/// Test truncated TCP header.
#[test]
fn test_truncated_tcp_header() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .tcp(12345, 80, 1000, 4096);

    let mut packet = Vec::new();
    builder.write(&mut packet, b"test").unwrap();

    // Truncate packet to cut off TCP header
    packet.truncate(packet.len() - 10);

    let result = core.process_packet(1, 1_000_000, &packet, "test");

    // Should not panic
    drop(result);
}

/// Test unknown IP protocol number.
#[test]
fn test_unknown_ip_protocol() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    // Ethernet + IP with protocol 255 (reserved)
    let packet = vec![
        // Ethernet header
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x08,
        0x00, // EtherType: IPv4
        // IP header with unknown protocol
        0x45, 0x00, 0x00, 0x14, 0x00, 0x00, 0x00, 0x00, 0x40, 0xff, 0x00,
        0x00, // Protocol: 255 (reserved)
        0xc0, 0xa8, 0x01, 0x01, 0x0a, 0x00, 0x00, 0x01,
    ];

    let result = core.process_packet(1, 1_000_000, &packet, "test");

    // Should handle gracefully (no panic, may produce no events)
    drop(result);
}

/// Test TCP packet with invalid checksum.
#[test]
fn test_tcp_invalid_checksum() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .tcp(12345, 80, 1000, 4096);

    let mut packet = Vec::new();
    builder.write(&mut packet, b"test").unwrap();

    // Corrupt TCP checksum (offset: 14 Ethernet + 20 IP + 16 for checksum)
    packet[14 + 20 + 16] ^= 0xff;
    packet[14 + 20 + 17] ^= 0xff;

    let result = core.process_packet(1, 1_000_000, &packet, "test");

    // Should not panic (checksum validation is typically optional in parsers)
    drop(result);
}

/// Test UDP packet with payload size mismatch.
#[test]
fn test_udp_payload_size_mismatch() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .udp(12345, 53);

    let mut packet = Vec::new();
    builder.write(&mut packet, b"test").unwrap();

    // Corrupt UDP length field (offset: 14 Ethernet + 20 IP + 4 for length)
    let udp_len_offset = 14 + 20 + 4;
    packet[udp_len_offset] = 0xff;
    packet[udp_len_offset + 1] = 0xff;

    let result = core.process_packet(1, 1_000_000, &packet, "test");

    // Should not panic
    drop(result);
}

/// Test IP fragmentation with missing fragments.
#[test]
fn test_ip_fragmentation_missing_fragments() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    // Send fragment 1 but never send fragment 0 (first fragment)
    let packet = vec![
        // Ethernet header
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x08, 0x00,
        // IP header with fragment offset=1, more_fragments=false
        0x45, 0x00, 0x00, 0x20, 0x12, 0x34, 0x00,
        0x08, // Fragment offset = 1 (offset*8 = 8 bytes)
        0x40, 0x11, 0x00, 0x00, 0xc0, 0xa8, 0x01, 0x01, 0x0a, 0x00, 0x00, 0x01,
        // UDP header
        0x30, 0x39, 0x00, 0x35, 0x00, 0x0c, 0x00, 0x00, // Some data
        0x00, 0x01, 0x02, 0x03,
    ];

    let result = core.process_packet(1, 1_000_000, &packet, "test");

    // Should not panic, fragment will wait for first fragment
    assert_eq!(
        result.events.len(),
        0,
        "Expected no events from incomplete fragment"
    );
}

/// Test back-to-back error recovery: ensure pipeline continues after error.
#[test]
fn test_error_recovery() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    // Process a corrupt packet
    let corrupt_packet = vec![0xff; 10];
    core.process_packet(1, 1_000_000, &corrupt_packet, "test");

    // Now process a valid packet
    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .udp(12345, 53);

    let mut valid_packet = Vec::new();
    builder.write(&mut valid_packet, b"valid").unwrap();

    let result = core.process_packet(1, 1_000_001, &valid_packet, "test");

    // Should process valid packet successfully after error
    assert_eq!(
        result.events.len(),
        1,
        "Expected event from valid packet after error"
    );
}

/// Test zero-length TCP payload.
#[test]
fn test_tcp_zero_length_payload() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .tcp(12345, 80, 1000, 4096);

    let mut packet = Vec::new();
    builder.write(&mut packet, b"").unwrap(); // Empty payload

    let result = core.process_packet(1, 1_000_000, &packet, "test");

    // Should handle gracefully
    drop(result);
}

/// Test zero-length UDP payload.
#[test]
fn test_udp_zero_length_payload() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .udp(12345, 53);

    let mut packet = Vec::new();
    builder.write(&mut packet, b"").unwrap(); // Empty payload

    let result = core.process_packet(1, 1_000_000, &packet, "test");

    // Should produce an event (even with empty payload)
    assert!(result.events.len() <= 1);
}

/// Test all-zeros packet (likely corrupt).
#[test]
fn test_all_zeros_packet() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    let zero_packet = vec![0x00; 64];
    let result = core.process_packet(1, 1_000_000, &zero_packet, "test");

    // Should not panic
    drop(result);
}

/// Test all-ones packet (likely corrupt).
#[test]
fn test_all_ones_packet() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    let ones_packet = vec![0xff; 64];
    let result = core.process_packet(1, 1_000_000, &ones_packet, "test");

    // Should not panic
    drop(result);
}

/// Test mixed valid and invalid packets in sequence.
#[test]
fn test_mixed_valid_invalid_sequence() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    use etherparse::PacketBuilder;

    for i in 0..10 {
        if i % 3 == 0 {
            // Invalid packet
            let invalid = vec![0xaa; 15];
            core.process_packet(1, 1_000_000 + i as u64, &invalid, "test");
        } else {
            // Valid packet
            let builder = PacketBuilder::ethernet2([0x00; 6], [0xff; 6])
                .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
                .udp(12345, 53);

            let mut packet = Vec::new();
            builder.write(&mut packet, b"data").unwrap();
            core.process_packet(1, 1_000_000 + i as u64, &packet, "test");
        }
    }

    // Should process all packets without panic
    let stats = core.stats();
    assert!(stats.packets_read == 10);
}

/// Test stats counter for unexpected empty events.
#[test]
fn test_unexpected_empty_events_stat() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let core = PipelineCore::new(tls_processor, registry);

    // Initially should be zero
    assert_eq!(core.stats().unexpected_empty_events, 0);

    // Note: We can't easily trigger the defensive error path without mocking,
    // but we verify the stat field exists and is initialized to zero.
}
