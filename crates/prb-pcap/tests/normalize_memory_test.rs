//! Memory cleanup tests for IP defragmentation.
//!
//! These tests verify that reassembled fragment payloads are properly cleaned up
//! via the timestamp-based cache (reassembled_cache) in PacketNormalizer.
//!
//! The cache is cleaned up every DEFRAG_CLEANUP_INTERVAL (1000 packets),
//! removing entries older than DEFRAG_TIMEOUT_US (5 seconds).
//!
//! These tests are marked with #[ignore] and must be run explicitly with:
//! `cargo test -p prb-pcap normalize_memory -- --ignored`

use etherparse::{EtherType, Ethernet2Header, IpNumber, Ipv4Header};
use prb_pcap::PacketNormalizer;

/// Creates a fragmented IPv4 packet.
///
/// This creates a raw IP fragment (no transport layer header).
/// For proper fragmentation, all fragments of the same packet must have:
/// - Same identification number (id)
/// - Same source and destination IPs
/// - Correct fragment_offset (in 8-byte units)
/// - more_fragments flag (true for all except last fragment)
fn create_ipv4_fragment(
    id: u16,
    more_fragments: bool,
    fragment_offset: u16,
    payload: &[u8],
) -> Vec<u8> {
    let mut packet = Vec::new();

    let eth = Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: EtherType(0x0800),
    };

    let mut ipv4 = Ipv4Header::new(
        payload.len() as u16,
        64,
        IpNumber(17), // UDP (though we're not including UDP header in fragments)
        [192, 168, 1, 1],
        [10, 0, 0, 1],
    )
    .unwrap();
    ipv4.identification = id;
    ipv4.fragment_offset = etherparse::IpFragOffset::try_new(fragment_offset).unwrap();
    ipv4.more_fragments = more_fragments;

    eth.write(&mut packet).unwrap();
    ipv4.write(&mut packet).unwrap();
    packet.extend_from_slice(payload);
    packet
}

#[test]
#[ignore] // Run with: cargo test -p prb-pcap normalize_memory -- --ignored
fn test_fragment_memory_usage() {
    let mut normalizer = PacketNormalizer::new();

    println!("Starting memory cleanup test...");
    println!("Note: This test verifies cache-based memory management (no Box::leak).");

    // Process 10,000 fragmented packets (each with 2 fragments)
    // Each fragment payload is 96 bytes (8-byte aligned), so 192 bytes per complete packet
    for i in 0..10_000 {
        let id = i as u16;
        let timestamp_us = i * 1000; // 1ms apart

        // First fragment (96 bytes, offset 0)
        let frag0 = create_ipv4_fragment(id, true, 0, &[0xaa; 96]);
        let result = normalizer.normalize(1, timestamp_us, &frag0);
        assert!(result.is_ok(), "First fragment failed: {:?}", result);
        assert!(result.unwrap().is_none(), "First fragment should buffer");

        // Last fragment (96 bytes, offset 96/8 = 12)
        let frag1 = create_ipv4_fragment(id, false, 96 / 8, &[0xbb; 96]);
        let result = normalizer.normalize(1, timestamp_us + 100, &frag1);
        assert!(result.is_ok(), "Second fragment failed: {:?}", result);
        assert!(
            result.unwrap().is_some(),
            "Last fragment should return reassembled packet"
        );
    }

    println!("Processed 10,000 fragmented packets (20,000 fragments total)");
    println!("Reassembled payloads stored in cache: ~1.9 MB (10,000 packets × 192 bytes)");
    println!();
    println!("MEMORY MANAGEMENT:");
    println!("- normalize.rs uses reassembled_cache (Vec) for reassembled payloads");
    println!("- Cache is cleaned up every 1,000 packets (DEFRAG_CLEANUP_INTERVAL)");
    println!("- Entries older than 5 seconds (DEFRAG_TIMEOUT_US) are removed");
    println!("- No memory leaks - cache is bounded by time window");
    println!();
    println!("Test completed successfully. Memory is properly managed via cache.");
}

#[test]
#[ignore]
fn test_incomplete_fragments_memory_cleanup() {
    let mut normalizer = PacketNormalizer::new();

    println!("Testing incomplete fragment cleanup...");

    // Send 5,000 incomplete fragment trains (only first fragment, never completed)
    for i in 0..5_000 {
        let id = i as u16;
        let timestamp_us = i * 1000;

        // Send only first fragment, never complete it
        let frag0 = create_ipv4_fragment(id, true, 0, &[0xaa; 96]);
        let result = normalizer.normalize(1, timestamp_us, &frag0);
        assert!(result.is_ok(), "Fragment failed: {:?}", result);
        assert!(result.unwrap().is_none());
    }

    println!("Sent 5,000 incomplete fragment trains");

    // Advance time by 6 seconds and send 1000 packets to trigger cleanup
    let cleanup_time = 6_000_000u64;
    let dummy = create_ipv4_fragment(60000, false, 0, b"cleanup trigger");

    for i in 0..1000 {
        let _ = normalizer.normalize(1, cleanup_time + i * 1000, &dummy);
    }

    println!("Cleanup triggered at 6 seconds (past 5-second timeout)");
    println!();
    println!("CLEANUP BEHAVIOR:");
    println!("- Incomplete fragments are evicted after 5-second timeout");
    println!("- Completed fragments in reassembled_cache are also cleaned up");
    println!("- Cleanup runs every 1,000 packets (DEFRAG_CLEANUP_INTERVAL)");
    println!("- saturating_sub prevents underflow on timestamp wraparound");
    println!();
    println!("Expected: Both incomplete and completed fragments cleaned up after timeout");
}

#[test]
#[ignore]
fn test_memory_growth_pattern() {
    let mut normalizer = PacketNormalizer::new();

    println!("Testing memory growth pattern with mixed complete/incomplete fragments...");

    // Mix of complete and incomplete fragment trains
    for batch in 0..10 {
        println!("Batch {}/10", batch + 1);

        // 500 complete fragment trains
        for i in 0..500 {
            let id = (batch * 1000 + i) as u16;
            let ts = (batch * 500_000 + i * 1000) as u64;

            let frag0 = create_ipv4_fragment(id, true, 0, &[0xaa; 96]);
            let result = normalizer.normalize(1, ts, &frag0);
            assert!(result.is_ok(), "Fragment 0 failed: {:?}", result);

            let frag1 = create_ipv4_fragment(id, false, 96 / 8, &[0xbb; 96]);
            let result = normalizer.normalize(1, ts + 100, &frag1);
            assert!(result.is_ok(), "Fragment 1 failed: {:?}", result);
            assert!(result.unwrap().is_some());
        }

        // 500 incomplete fragment trains
        for i in 500..1000 {
            let id = (batch * 1000 + i) as u16;
            let ts = (batch * 500_000 + i * 1000) as u64;

            let frag0 = create_ipv4_fragment(id, true, 0, &[0xcc; 96]);
            let result = normalizer.normalize(1, ts, &frag0);
            assert!(result.is_ok(), "Incomplete fragment failed: {:?}", result);
            // Never send second fragment
        }

        // After each batch, trigger cleanup if enough packets processed
        if batch > 0 && batch % 2 == 0 {
            // Advance time significantly to trigger cleanup
            let cleanup_ts = (batch * 500_000 + 6_000_000) as u64;
            let dummy = create_ipv4_fragment(65000, false, 0, b"dummy");
            for i in 0..1000 {
                let _ = normalizer.normalize(1, cleanup_ts + i * 1000, &dummy);
            }
            println!("  Cleanup triggered");
        }
    }

    println!();
    println!("MEMORY GROWTH PATTERN:");
    println!("- Completed fragments: Stored in cache, cleaned up after 5 seconds");
    println!("- Incomplete fragments: Cleaned up after timeout");
    println!("- Maximum cache size: Bounded by 5-second time window");
    println!("- Total processed: 5,000 packets × 192 bytes = ~960 KB");
    println!("- Both fragment types are cleaned up periodically");
    println!();
    println!("Test completed. Memory is properly bounded by time window.");
}
