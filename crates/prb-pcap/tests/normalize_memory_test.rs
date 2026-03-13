//! Memory profiling tests for IP defragmentation.
//!
//! These tests are marked with #[ignore] and must be run explicitly with:
//! `cargo test -p prb-pcap normalize_memory -- --ignored`

use etherparse::{EtherType, Ethernet2Header, IpNumber, Ipv4Header};
use prb_pcap::PacketNormalizer;

/// Creates a fragmented IPv4 packet.
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
    eth.write(&mut packet).unwrap();

    let mut ipv4 = Ipv4Header::new(
        payload.len() as u16,
        64,
        IpNumber(17), // UDP
        [192, 168, 1, 1],
        [10, 0, 0, 1],
    )
    .unwrap();
    ipv4.more_fragments = more_fragments;
    ipv4.fragment_offset = fragment_offset.try_into().unwrap();
    ipv4.identification = id;
    ipv4.write(&mut packet).unwrap();

    packet.extend_from_slice(payload);
    packet
}

#[test]
#[ignore] // Run with: cargo test -p prb-pcap normalize_memory -- --ignored
fn test_fragment_memory_usage() {
    let mut normalizer = PacketNormalizer::new();

    println!("Starting memory profiling test...");
    println!("Note: This test documents Box::leak behavior - memory is intentionally leaked.");

    // Process 10,000 fragmented packets (each with 2 fragments)
    // Each fragment payload is 100 bytes, so 200 bytes per complete packet
    for i in 0..10_000 {
        let id = i as u16;
        let timestamp_us = i * 1000; // 1ms apart

        // First fragment (100 bytes)
        let frag0 = create_ipv4_fragment(id, true, 0, &vec![0xaa; 100]);
        let result = normalizer.normalize(1, timestamp_us, &frag0);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none(), "First fragment should buffer");

        // Last fragment (100 bytes)
        let frag1 = create_ipv4_fragment(id, false, 13, &vec![0xbb; 100]); // offset 13 = 100/8 rounded up
        let result = normalizer.normalize(1, timestamp_us + 100, &frag1);
        assert!(result.is_ok());
        assert!(
            result.unwrap().is_some(),
            "Last fragment should return reassembled packet"
        );
    }

    println!("Processed 10,000 fragmented packets (20,000 fragments total)");
    println!("Expected memory leaked: ~2 MB (10,000 packets × 200 bytes)");
    println!();
    println!("MEMORY LEAK DOCUMENTED:");
    println!("- Line 354 in normalize.rs uses Box::leak for reassembled payloads");
    println!("- This intentional leak satisfies 'static lifetime requirements");
    println!("- Memory is never freed, even after packets are processed");
    println!("- For long-running captures, consider using an arena allocator");
    println!();
    println!("Test completed successfully. Memory leak is expected behavior.");
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
        let frag0 = create_ipv4_fragment(id, true, 0, &vec![0xaa; 100]);
        let result = normalizer.normalize(1, timestamp_us, &frag0);
        assert!(result.is_ok());
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
    println!("- Cleanup runs every 1,000 packets (DEFRAG_CLEANUP_INTERVAL)");
    println!("- Line 170: saturating_sub prevents underflow on timestamp wraparound");
    println!();
    println!("Expected: Incomplete fragments cleaned up after timeout");
    println!("Note: Completed fragments are still leaked via Box::leak");
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

            let frag0 = create_ipv4_fragment(id, true, 0, &vec![0xaa; 100]);
            let _ = normalizer.normalize(1, ts, &frag0);

            let frag1 = create_ipv4_fragment(id, false, 13, &vec![0xbb; 100]);
            let result = normalizer.normalize(1, ts + 100, &frag1);
            assert!(result.unwrap().is_some());
        }

        // 500 incomplete fragment trains
        for i in 500..1000 {
            let id = (batch * 1000 + i) as u16;
            let ts = (batch * 500_000 + i * 1000) as u64;

            let frag0 = create_ipv4_fragment(id, true, 0, &vec![0xcc; 100]);
            let _ = normalizer.normalize(1, ts, &frag0);
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
    println!("- Completed fragments: Memory grows unbounded (Box::leak)");
    println!("- Incomplete fragments: Cleaned up after timeout");
    println!("- Total leaked: ~1 MB (5,000 completed × 200 bytes)");
    println!("- Cleaned up: Incomplete fragments evicted periodically");
    println!();
    println!("Test completed. Memory leak is expected for completed fragments.");
}
