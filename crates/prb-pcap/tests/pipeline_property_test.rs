//! Property-based tests for pipeline robustness.
//!
//! Uses proptest to verify that the pipeline never panics with arbitrary input.

use prb_detect::DecoderRegistry;
use prb_pcap::PipelineCore;
use prb_pcap::tls::TlsStreamProcessor;
use proptest::prelude::*;

proptest! {
    /// Property test: pipeline never panics with arbitrary packets.
    ///
    /// This test generates random byte sequences of varying lengths and
    /// verifies the pipeline handles them gracefully without panicking.
    #[test]
    fn pipeline_never_panics_with_arbitrary_packets(
        packets in prop::collection::vec(
            prop::collection::vec(any::<u8>(), 0..2000),
            0..100
        )
    ) {
        let tls_processor = TlsStreamProcessor::new();
        let registry = DecoderRegistry::new();
        let mut core = PipelineCore::new(tls_processor, registry);

        // Process all packets - should never panic regardless of input
        for (idx, packet_data) in packets.iter().enumerate() {
            let _ = core.process_packet(1, idx as u64 * 1000, packet_data, "proptest");
        }

        // Verify stats are reasonable
        let stats = core.stats();
        assert!(stats.packets_read > 0 || packets.is_empty());
    }

    /// Property test: pipeline handles arbitrary linktypes.
    #[test]
    fn pipeline_handles_arbitrary_linktypes(
        linktype in any::<u32>(),
        packet in prop::collection::vec(any::<u8>(), 0..1500)
    ) {
        let tls_processor = TlsStreamProcessor::new();
        let registry = DecoderRegistry::new();
        let mut core = PipelineCore::new(tls_processor, registry);

        // Should not panic with any linktype value
        let _ = core.process_packet(linktype, 1_000_000, &packet, "proptest");
    }

    /// Property test: pipeline handles arbitrary timestamps.
    #[test]
    fn pipeline_handles_arbitrary_timestamps(
        timestamp in any::<u64>(),
        packet in prop::collection::vec(any::<u8>(), 14..1500)
    ) {
        let tls_processor = TlsStreamProcessor::new();
        let registry = DecoderRegistry::new();
        let mut core = PipelineCore::new(tls_processor, registry);

        // Should not panic with any timestamp value (including 0, MAX, etc.)
        let _ = core.process_packet(1, timestamp, &packet, "proptest");
    }

    /// Property test: pipeline handles arbitrary origin strings.
    #[test]
    fn pipeline_handles_arbitrary_origin_strings(
        origin in "\\PC*",
        packet in prop::collection::vec(any::<u8>(), 0..500)
    ) {
        let tls_processor = TlsStreamProcessor::new();
        let registry = DecoderRegistry::new();
        let mut core = PipelineCore::new(tls_processor, registry);

        // Should not panic with any origin string
        let _ = core.process_packet(1, 1_000_000, &packet, &origin);
    }

    /// Property test: warnings are bounded per packet.
    #[test]
    fn warnings_are_bounded(
        packet in prop::collection::vec(any::<u8>(), 0..100)
    ) {
        let tls_processor = TlsStreamProcessor::new();
        let registry = DecoderRegistry::new();
        let mut core = PipelineCore::new(tls_processor, registry);

        let result = core.process_packet(1, 1_000_000, &packet, "proptest");

        // Warnings should never exceed the capacity limit (100)
        assert!(
            result.warnings.len() <= 100,
            "Warnings exceeded limit: {} warnings",
            result.warnings.len()
        );
    }

    /// Property test: multiple packets in sequence never panic.
    #[test]
    fn multiple_packets_never_panic(
        packets in prop::collection::vec(
            (any::<u32>(), any::<u64>(), prop::collection::vec(any::<u8>(), 0..1500)),
            0..50
        )
    ) {
        let tls_processor = TlsStreamProcessor::new();
        let registry = DecoderRegistry::new();
        let mut core = PipelineCore::new(tls_processor, registry);

        for (linktype, timestamp, packet_data) in packets {
            let _ = core.process_packet(linktype, timestamp, &packet_data, "proptest");
        }

        // Should complete without panic - just verify we can access stats
        let _ = core.stats();
    }

    /// Property test: flush_idle never panics with arbitrary timestamps.
    #[test]
    fn flush_idle_never_panics(
        current_time in any::<u64>()
    ) {
        let tls_processor = TlsStreamProcessor::new();
        let registry = DecoderRegistry::new();
        let mut core = PipelineCore::new(tls_processor, registry);

        // Should not panic with any timestamp
        let _ = core.flush_idle(current_time);
    }

    /// Property test: events vector is bounded.
    #[test]
    fn events_are_bounded(
        packet in prop::collection::vec(any::<u8>(), 0..1500)
    ) {
        let tls_processor = TlsStreamProcessor::new();
        let registry = DecoderRegistry::new();
        let mut core = PipelineCore::new(tls_processor, registry);

        let result = core.process_packet(1, 1_000_000, &packet, "proptest");

        // Events should be reasonable (single packet typically produces 0-1 event)
        // Allow up to 10 events for edge cases (fragmentation, multiple protocols, etc.)
        assert!(
            result.events.len() <= 10,
            "Excessive events produced: {} events",
            result.events.len()
        );
    }
}

/// Test that property tests can actually catch issues (meta-test).
#[test]
fn proptest_framework_works() {
    // Simple property: all generated vectors have length within bounds
    proptest!(|(v in prop::collection::vec(any::<u8>(), 0..10))| {
        assert!(v.len() <= 10);
    });
}
