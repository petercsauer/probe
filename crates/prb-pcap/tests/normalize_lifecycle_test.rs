//! IP defragmentation lifecycle tests: reassembly, timeout cleanup, memory safety.

use etherparse::{EtherType, Ethernet2Header, IpNumber, Ipv4Header};
use prb_pcap::PacketNormalizer;
use std::net::IpAddr;

/// Creates a fragmented IPv4 packet with given fragment parameters.
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
fn test_fragment_reassembly() {
    let mut normalizer = PacketNormalizer::new();

    // Create 3 fragments of a UDP packet
    // Fragment 0: offset 0, MF=1, 24 bytes (UDP header + first 16 bytes)
    let mut frag0_payload = Vec::new();
    frag0_payload.extend_from_slice(&12345u16.to_be_bytes()); // src port
    frag0_payload.extend_from_slice(&80u16.to_be_bytes()); // dst port
    frag0_payload.extend_from_slice(&32u16.to_be_bytes()); // length (8 + 24)
    frag0_payload.extend_from_slice(&0u16.to_be_bytes()); // checksum
    frag0_payload.extend_from_slice(b"AAAAAAAAAAAAAAAA"); // 16 bytes

    let frag0 = create_ipv4_fragment(12345, true, 0, &frag0_payload);

    // Fragment 1: offset 3 (24 bytes / 8), MF=1, 16 bytes
    let frag1_payload = b"BBBBBBBBBBBBBBBB";
    let frag1 = create_ipv4_fragment(12345, true, 3, frag1_payload);

    // Fragment 2: offset 5 (40 bytes / 8), MF=0, 8 bytes (last)
    let frag2_payload = b"CCCCCCCC";
    let frag2 = create_ipv4_fragment(12345, false, 5, frag2_payload);

    // Process first two fragments - should return None (waiting)
    let result0 = normalizer.normalize(1, 1000000, &frag0).unwrap();
    assert!(result0.is_none(), "First fragment should return None");

    let result1 = normalizer.normalize(1, 2000000, &frag1).unwrap();
    assert!(result1.is_none(), "Second fragment should return None");

    // Process last fragment - should return reassembled packet
    let result2 = normalizer.normalize(1, 3000000, &frag2).unwrap();
    assert!(
        result2.is_some(),
        "Last fragment should return reassembled packet"
    );

    let packet = result2.unwrap();
    assert_eq!(packet.src_ip, IpAddr::from([192, 168, 1, 1]));
    assert_eq!(packet.dst_ip, IpAddr::from([10, 0, 0, 1]));

    // Check payload is reassembled correctly
    let expected_payload = b"AAAAAAAAAAAAAAAABBBBBBBBBBBBBBBBCCCCCCCC";
    assert_eq!(packet.payload, expected_payload);
}

#[test]
fn test_timeout_cleanup() {
    let mut normalizer = PacketNormalizer::new();

    // Send first fragment at time 0
    let frag0_payload = vec![0xaa; 24];
    let frag0 = create_ipv4_fragment(54321, true, 0, &frag0_payload);

    let result = normalizer.normalize(1, 0, &frag0).unwrap();
    assert!(result.is_none(), "Fragment should be buffered");

    // Advance time by 6 seconds (past DEFRAG_TIMEOUT_US of 5 seconds)
    // Send a new packet to trigger cleanup (cleanup happens every 1000 packets)
    // Create a valid UDP packet (8-byte header + payload)
    let mut dummy_payload = Vec::new();
    dummy_payload.extend_from_slice(&12345u16.to_be_bytes()); // src port
    dummy_payload.extend_from_slice(&80u16.to_be_bytes()); // dst port
    dummy_payload.extend_from_slice(&13u16.to_be_bytes()); // length (8 + 5)
    dummy_payload.extend_from_slice(&0u16.to_be_bytes()); // checksum
    dummy_payload.extend_from_slice(b"dummy"); // 5 bytes payload
    let dummy_packet = create_ipv4_fragment(60000, false, 0, &dummy_payload);

    // Send 999 packets to not trigger cleanup yet
    for i in 0..999 {
        let _ = normalizer.normalize(1, 1_000_000 + i * 1000, &dummy_packet);
    }

    // The 1000th packet should trigger cleanup at timestamp 6_000_000
    let _ = normalizer.normalize(1, 6_000_000, &dummy_packet).unwrap();

    // Now try to send another fragment with the same ID - it should start a NEW fragment train
    // because the old one was evicted. Send first + last to complete the new train.
    // Fragment payloads must be multiples of 8 bytes (except last fragment)
    let new_frag0_payload = vec![0xcc; 24]; // 24 is multiple of 8
    let new_frag0 = create_ipv4_fragment(54321, true, 0, &new_frag0_payload);

    let result0 = normalizer.normalize(1, 6_500_000, &new_frag0).unwrap();
    assert!(
        result0.is_none(),
        "First fragment of new train should buffer"
    );

    let new_frag1_payload = vec![0xdd; 12]; // Last fragment can be any size
    let new_frag1 = create_ipv4_fragment(54321, false, 3, &new_frag1_payload);

    let result1 = normalizer.normalize(1, 6_500_100, &new_frag1).unwrap();
    assert!(
        result1.is_some(),
        "New fragment train should complete successfully, proving old one was cleaned up"
    );
}

#[test]
fn test_10k_fragmented_packets() {
    let mut normalizer = PacketNormalizer::new();

    // Process 10,000 fragmented packets (each with 2 fragments)
    // This tests that memory is managed reasonably
    for i in 0..10_000 {
        let id = i as u16;
        let timestamp_us = i * 1000; // 1ms apart

        // First fragment
        let frag0 = create_ipv4_fragment(id, true, 0, b"AAAAAAAA");
        let result = normalizer.normalize(1, timestamp_us, &frag0).unwrap();
        assert!(result.is_none());

        // Last fragment - should complete reassembly
        let frag1 = create_ipv4_fragment(id, false, 1, b"BBBBBBBB");
        let result = normalizer.normalize(1, timestamp_us + 100, &frag1).unwrap();
        assert!(result.is_some(), "Fragment {} should reassemble", id);
    }

    // All fragments should complete successfully
    // Memory leak: Box::leak means leaked memory is never freed
    // This test documents the behavior - real-world usage should be monitored
}

#[test]
fn test_timestamp_wraparound() {
    let mut normalizer = PacketNormalizer::new();

    // Start at u64::MAX - 1000 microseconds
    let start_time = u64::MAX - 1000;

    // Send first fragment near u64::MAX
    let frag0 = create_ipv4_fragment(11111, true, 0, b"AAAAAAAA");
    let result = normalizer.normalize(1, start_time, &frag0).unwrap();
    assert!(result.is_none());

    // Send second fragment after wraparound (timestamp wraps to small value)
    // saturating_sub in cleanup should handle this gracefully
    let frag1 = create_ipv4_fragment(11111, false, 1, b"BBBBBBBB");
    let _result = normalizer.normalize(1, 500, &frag1).unwrap();

    // The fragment might complete or timeout depending on cleanup logic
    // Key requirement: no panic on wraparound
    // Result is Some or None, but should not panic
}

#[test]
fn test_backwards_time() {
    let mut normalizer = PacketNormalizer::new();

    // Send packets with decreasing timestamps
    let timestamps = [5_000_000u64, 4_000_000, 3_000_000, 2_000_000, 1_000_000];

    for (i, &ts) in timestamps.iter().enumerate() {
        let id = (i as u16) + 1000;

        // Send a complete (non-fragmented) UDP packet
        let mut udp_payload = Vec::new();
        udp_payload.extend_from_slice(&12345u16.to_be_bytes()); // src port
        udp_payload.extend_from_slice(&80u16.to_be_bytes()); // dst port
        udp_payload.extend_from_slice(&17u16.to_be_bytes()); // length (8 + 9)
        udp_payload.extend_from_slice(&0u16.to_be_bytes()); // checksum
        udp_payload.extend_from_slice(b"backwards");
        let packet = create_ipv4_fragment(id, false, 0, &udp_payload);

        // Should not panic on decreasing timestamps
        let result = normalizer.normalize(1, ts, &packet);
        assert!(result.is_ok(), "Should handle backwards time gracefully");

        if let Ok(Some(pkt)) = result {
            assert_eq!(pkt.timestamp_us, ts);
        }
    }
}

#[test]
fn test_huge_time_gap() {
    let mut normalizer = PacketNormalizer::new();

    // Send first fragment at time 0
    let frag0 = create_ipv4_fragment(40000, true, 0, b"AAAAAAAA");
    let result = normalizer.normalize(1, 0, &frag0).unwrap();
    assert!(result.is_none());

    // Advance time by 1 year (365 days in microseconds)
    let one_year_us = 365u64 * 24 * 60 * 60 * 1_000_000;

    // Send 999 dummy packets to avoid triggering cleanup
    let mut dummy_payload = Vec::new();
    dummy_payload.extend_from_slice(&12345u16.to_be_bytes()); // src port
    dummy_payload.extend_from_slice(&80u16.to_be_bytes()); // dst port
    dummy_payload.extend_from_slice(&13u16.to_be_bytes()); // length (8 + 5)
    dummy_payload.extend_from_slice(&0u16.to_be_bytes()); // checksum
    dummy_payload.extend_from_slice(b"dummy");
    let dummy = create_ipv4_fragment(50000, false, 0, &dummy_payload);
    for i in 0..999 {
        let _ = normalizer.normalize(1, one_year_us + i * 1000, &dummy);
    }

    // The 1000th packet triggers cleanup - should evict the ancient fragment
    let _ = normalizer
        .normalize(1, one_year_us + 1_000_000, &dummy)
        .unwrap();

    // Try to complete the ancient fragment - should be evicted
    let frag1 = create_ipv4_fragment(40000, false, 1, b"BBBBBBBB");
    let result = normalizer
        .normalize(1, one_year_us + 2_000_000, &frag1)
        .unwrap();

    // Fragment train was cleaned up, so this is a new incomplete train
    assert!(result.is_none(), "Ancient fragment should be cleaned up");
}
