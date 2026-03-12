//! Pipeline error handling and edge case tests.

use prb_detect::DecoderRegistry;
use prb_pcap::tls::TlsStreamProcessor;
use prb_pcap::{PcapCaptureAdapter, PipelineCore};
use std::path::PathBuf;

/// Test creating adapter with nonexistent file (construction should succeed, errors occur on use).
#[test]
fn test_adapter_with_nonexistent_file() {
    let _adapter = PcapCaptureAdapter::new(PathBuf::from("/nonexistent/file.pcap"), None);
    // Construction succeeds; errors would occur when processing begins
}

/// Test creating adapter with nonexistent keylog file.
#[test]
fn test_adapter_with_nonexistent_keylog() {
    let _adapter = PcapCaptureAdapter::new(
        PathBuf::from("test.pcap"),
        Some(PathBuf::from("/nonexistent/keylog.txt")),
    );
    // Construction succeeds; keylog errors would occur during pipeline processing
}

/// Test pipeline_core with malformed packet data.
#[test]
fn test_pipeline_core_malformed_packet() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    // Malformed packet: truncated Ethernet frame
    let malformed_packet = vec![0xff; 10]; // Too short to be valid

    let result = core.process_packet(1, 1000000, &malformed_packet, "test.pcap");

    // Should not panic, may produce warnings
    assert_eq!(
        result.events.len(),
        0,
        "Expected no events from malformed packet"
    );
}

/// Test pipeline_core with empty packet data.
#[test]
fn test_pipeline_core_empty_packet() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    let empty_packet = vec![];

    let result = core.process_packet(1, 1000000, &empty_packet, "test.pcap");

    // Should handle gracefully
    assert_eq!(result.events.len(), 0);
}

/// Test pipeline_core with unsupported linktype.
#[test]
fn test_pipeline_core_unsupported_linktype() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    // Valid Ethernet packet but wrong linktype
    let packet = vec![0u8; 64];

    let result = core.process_packet(999, 1000000, &packet, "test.pcap");

    // Should produce warnings about unsupported linktype
    assert!(!result.warnings.is_empty() || result.events.is_empty());
}

/// Test protocol override setter.
#[test]
fn test_protocol_override_setter() {
    let mut adapter = PcapCaptureAdapter::new(PathBuf::from("test.pcap"), None);

    // Set protocol override
    adapter.set_protocol_override("grpc");

    // Protocol override is set (we can't easily verify it's used without a full pipeline run,
    // but we can ensure the setter doesn't panic)
}

/// Test protocol override with various protocol names.
#[test]
fn test_protocol_override_variants() {
    let mut adapter = PcapCaptureAdapter::new(PathBuf::from("test.pcap"), None);

    // Test all valid protocol names
    adapter.set_protocol_override("grpc");
    adapter.set_protocol_override("http2");
    adapter.set_protocol_override("zmtp");
    adapter.set_protocol_override("zmq");
    adapter.set_protocol_override("rtps");
    adapter.set_protocol_override("dds");

    // Setting override shouldn't panic
}

/// Test stats() accessor returns expected default values.
#[test]
fn test_stats_default_values() {
    let adapter = PcapCaptureAdapter::new(PathBuf::from("test.pcap"), None);

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

/// Test creating adapter with custom decoder registry.
#[test]
fn test_adapter_with_custom_registry() {
    let registry = DecoderRegistry::new();
    let adapter = PcapCaptureAdapter::with_registry(PathBuf::from("test.pcap"), None, registry);

    // Should construct without panic
    assert_eq!(adapter.stats().packets_read, 0);
}

/// Test pipeline_core with large timestamp.
#[test]
fn test_pipeline_core_large_timestamp() {
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

    // Process with large timestamp (not MAX to avoid overflow in conversion)
    let result = core.process_packet(1, 1_000_000_000_000, &packet, "test.pcap");

    // Should handle large timestamps gracefully
    assert!(result.events.len() <= 1);
}

/// Test pipeline_core with zero timestamp.
#[test]
fn test_pipeline_core_zero_timestamp() {
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
    builder.write(&mut packet, b"DNS").unwrap();

    // Process with zero timestamp
    let result = core.process_packet(1, 0, &packet, "test.pcap");

    // Should handle gracefully
    assert!(result.events.len() <= 1);
}

/// Test TCP stream with out-of-order segments at edge of buffer.
#[test]
fn test_tcp_reassembly_edge_cases() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    use etherparse::PacketBuilder;

    // Send segment 2 before segment 1 (out of order)
    let builder2 = PacketBuilder::ethernet2([0x00; 6], [0xff; 6])
        .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
        .tcp(12345, 80, 2000, 4096); // SEQ 2000

    let mut packet2 = Vec::new();
    builder2.write(&mut packet2, b"segment2").unwrap();

    core.process_packet(1, 1000000, &packet2, "test.pcap");

    // Now send segment 1
    let builder1 = PacketBuilder::ethernet2([0x00; 6], [0xff; 6])
        .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
        .tcp(12345, 80, 1000, 4096); // SEQ 1000

    let mut packet1 = Vec::new();
    builder1.write(&mut packet1, b"segment1").unwrap();

    let result = core.process_packet(1, 1000001, &packet1, "test.pcap");

    // Should buffer and reassemble correctly
    // (exact behavior depends on implementation, just ensure no panic)
    drop(result);
}

/// Test UDP datagram with maximum allowed payload size.
#[test]
fn test_udp_maximum_payload() {
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

    // Maximum UDP payload (65535 - 20 IP header - 8 UDP header)
    let large_payload = vec![0x42; 65507];
    let mut packet = Vec::new();
    builder.write(&mut packet, &large_payload).unwrap();

    let result = core.process_packet(1, 1000000, &packet, "test.pcap");

    // Should handle without panic
    drop(result);
}

/// Test handling of multiple concurrent TCP streams.
#[test]
fn test_multiple_concurrent_tcp_streams() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    use etherparse::PacketBuilder;

    // Stream 1: port 12345 -> 80
    let builder1 = PacketBuilder::ethernet2([0x00; 6], [0xff; 6])
        .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
        .tcp(12345, 80, 1000, 4096);

    let mut packet1 = Vec::new();
    builder1.write(&mut packet1, b"stream1").unwrap();

    // Stream 2: port 54321 -> 443
    let builder2 = PacketBuilder::ethernet2([0x00; 6], [0xff; 6])
        .ipv4([192, 168, 1, 2], [10, 0, 0, 2], 64)
        .tcp(54321, 443, 2000, 4096);

    let mut packet2 = Vec::new();
    builder2.write(&mut packet2, b"stream2").unwrap();

    // Process both streams interleaved
    core.process_packet(1, 1000000, &packet1, "test.pcap");
    core.process_packet(1, 1000001, &packet2, "test.pcap");
    core.process_packet(1, 1000002, &packet1, "test.pcap");
    core.process_packet(1, 1000003, &packet2, "test.pcap");

    // Should track both streams independently
}

/// Test handling of TCP FIN followed by more data (invalid).
#[test]
fn test_tcp_data_after_fin() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    use etherparse::PacketBuilder;

    // Send FIN
    let builder_fin = PacketBuilder::ethernet2([0x00; 6], [0xff; 6])
        .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
        .tcp(12345, 80, 1000, 4096);

    let mut packet_fin = Vec::new();
    builder_fin.write(&mut packet_fin, b"data").unwrap();

    // Manually set FIN flag in TCP header
    // TCP flags are at offset: 14 (Ethernet) + 20 (IP) + 13 (TCP header offset)
    let flags_offset = 14 + 20 + 13;
    packet_fin[flags_offset] |= 0x01; // Set FIN flag

    core.process_packet(1, 1000000, &packet_fin, "test.pcap");

    // Try to send more data after FIN (invalid)
    let builder_after = PacketBuilder::ethernet2([0x00; 6], [0xff; 6])
        .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
        .tcp(12345, 80, 1004, 4096); // SEQ after previous

    let mut packet_after = Vec::new();
    builder_after.write(&mut packet_after, b"invalid").unwrap();

    let result = core.process_packet(1, 1000001, &packet_after, "test.pcap");

    // Should handle gracefully (may ignore or warn)
    drop(result);
}

/// Test handling of TCP RST.
#[test]
fn test_tcp_rst_flag() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    use etherparse::PacketBuilder;

    // Send RST
    let builder = PacketBuilder::ethernet2([0x00; 6], [0xff; 6])
        .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
        .tcp(12345, 80, 1000, 4096);

    let mut packet = Vec::new();
    builder.write(&mut packet, b"").unwrap();

    // Set RST flag
    let flags_offset = 14 + 20 + 13;
    packet[flags_offset] |= 0x04; // Set RST flag

    let result = core.process_packet(1, 1000000, &packet, "test.pcap");

    // Should handle RST and potentially close stream
    drop(result);
}

/// Test empty origin string.
#[test]
fn test_empty_origin_string() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ethernet2([0x00; 6], [0xff; 6])
        .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
        .udp(12345, 53);

    let mut packet = Vec::new();
    builder.write(&mut packet, b"test").unwrap();

    let result = core.process_packet(1, 1000000, &packet, "");

    // Should handle empty origin
    drop(result);
}

/// Test very long origin string.
#[test]
fn test_long_origin_string() {
    let tls_processor = TlsStreamProcessor::new();
    let registry = DecoderRegistry::new();
    let mut core = PipelineCore::new(tls_processor, registry);

    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ethernet2([0x00; 6], [0xff; 6])
        .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
        .udp(12345, 53);

    let mut packet = Vec::new();
    builder.write(&mut packet, b"test").unwrap();

    let long_origin = "x".repeat(10000);
    let result = core.process_packet(1, 1000000, &packet, &long_origin);

    // Should handle long origin
    drop(result);
}
