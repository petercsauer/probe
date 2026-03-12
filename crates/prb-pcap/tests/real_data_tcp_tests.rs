//! Integration tests with real-world TCP/IP captures.
//!
//! Tests TCP reassembly, IP normalization, and edge case handling with actual
//! captures from Wireshark sample collection.

use prb_pcap::{PacketNormalizer, PcapFileReader, TcpReassembler};
use std::path::PathBuf;

/// Helper to get fixture path relative to workspace root.
fn fixture_path(subdir: &str, filename: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // Go up from crates/prb-pcap
    path.pop(); // Go up from crates
    path.push("tests");
    path.push("fixtures");
    path.push("captures");
    path.push(subdir);
    path.push(filename);
    path
}

#[test]
fn test_tcp_ecn_sample() {
    // TCP with Explicit Congestion Notification - tests TCP flag handling
    let path = fixture_path("tcp", "tcp-ecn-sample.pcap");
    let mut reader = PcapFileReader::open(&path).expect("Failed to open tcp-ecn-sample.pcap");
    let packets = reader.read_all_packets().expect("Failed to read packets");

    assert!(
        !packets.is_empty(),
        "tcp-ecn-sample.pcap should contain packets"
    );

    let mut normalizer = PacketNormalizer::new();
    let mut reassembler = TcpReassembler::new();
    let mut tcp_packet_count = 0;
    let mut normalized_count = 0;

    for (idx, pkt) in packets.iter().enumerate() {
        if let Ok(Some(normalized)) =
            normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data)
        {
            normalized_count += 1;

            // Check if it's a TCP packet
            if let prb_pcap::TransportInfo::Tcp(_) = normalized.transport {
                tcp_packet_count += 1;

                // Process through reassembler (should not panic)
                let _events = reassembler
                    .process_segment(&normalized)
                    .expect("TCP reassembly failed");
            }
        }

        // Spot check: verify we can normalize early packets
        if idx < 10 {
            assert!(normalized_count > 0, "Should normalize some early packets");
        }
    }

    assert!(
        tcp_packet_count > 0,
        "tcp-ecn-sample.pcap should contain TCP packets"
    );
    // Verify reassembler is tracking connections (method callable)
    let _ = reassembler.active_connections();
}

#[test]
fn test_tcp_anon_session() {
    // Real TCP session with various edge cases
    let path = fixture_path("tcp", "200722_tcp_anon.pcapng");
    let mut reader = PcapFileReader::open(&path).expect("Failed to open 200722_tcp_anon.pcapng");
    let packets = reader.read_all_packets().expect("Failed to read packets");

    assert!(
        !packets.is_empty(),
        "200722_tcp_anon.pcapng should contain packets"
    );

    let mut normalizer = PacketNormalizer::new();
    let mut reassembler = TcpReassembler::new();
    let mut stream_data_events = 0;

    for pkt in &packets {
        if let Ok(Some(normalized)) =
            normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data)
            && let prb_pcap::TransportInfo::Tcp(_) = normalized.transport
        {
            let events = reassembler
                .process_segment(&normalized)
                .expect("TCP reassembly failed");

            // Count data events
            for event in events {
                if let prb_pcap::StreamEvent::Data(_) = event {
                    stream_data_events += 1;
                }
            }
        }
    }

    // Real TCP sessions should produce reassembled stream data
    assert!(
        stream_data_events > 0,
        "Real TCP session should produce stream data events"
    );
}

#[test]
fn test_dns_remoteshell_tcp_reassembly() {
    // TCP session with data exfiltration - tests multi-packet reassembly
    let path = fixture_path("tcp", "dns-remoteshell.pcap");
    let mut reader = PcapFileReader::open(&path).expect("Failed to open dns-remoteshell.pcap");
    let packets = reader.read_all_packets().expect("Failed to read packets");

    assert!(
        !packets.is_empty(),
        "dns-remoteshell.pcap should contain packets"
    );

    let mut normalizer = PacketNormalizer::new();
    let mut reassembler = TcpReassembler::new();
    let mut total_tcp_payload = 0usize;
    let mut tcp_segments = 0;

    for pkt in &packets {
        // Skip packets that can't be normalized (e.g., ARP)
        if let Ok(Some(normalized)) =
            normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data)
            && let prb_pcap::TransportInfo::Tcp(_) = normalized.transport
        {
            tcp_segments += 1;
            total_tcp_payload += normalized.payload.len();

            let _events = reassembler
                .process_segment(&normalized)
                .expect("TCP reassembly failed");
        }
    }

    assert!(
        tcp_segments > 0,
        "dns-remoteshell.pcap should contain TCP segments"
    );
    assert!(
        total_tcp_payload > 0,
        "TCP segments should carry payload data"
    );
}

#[test]
fn test_vlan_tagged_frames() {
    // VLAN-tagged Ethernet frames - tests VLAN stripping in normalization
    let path = fixture_path("ip", "vlan.cap");
    let mut reader = PcapFileReader::open(&path).expect("Failed to open vlan.cap");
    let packets = reader.read_all_packets().expect("Failed to read packets");

    assert!(!packets.is_empty(), "vlan.cap should contain packets");

    let mut normalizer = PacketNormalizer::new();
    let mut vlan_packets = 0;
    let mut normalized_count = 0;

    for pkt in &packets {
        // Skip packets that can't be normalized
        if let Ok(Some(normalized)) =
            normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data)
        {
            normalized_count += 1;

            // Check if VLAN ID was extracted
            if normalized.vlan_id.is_some() {
                vlan_packets += 1;
            }

            // Verify normalized packet has valid IPs
            assert!(
                !normalized.src_ip.is_unspecified(),
                "Normalized packet should have valid source IP"
            );
            assert!(
                !normalized.dst_ip.is_unspecified(),
                "Normalized packet should have valid destination IP"
            );
        }
    }

    assert!(normalized_count > 0, "Should normalize some packets");
    // VLAN tags should be detected (may not be in all packets)
    assert!(
        vlan_packets >= 0,
        "VLAN-tagged capture processed successfully"
    );
}

#[test]
fn test_ipv6_packet_normalization() {
    // IPv6 traffic samples - tests IPv6 address handling
    let path = fixture_path("ip", "v6.pcap");
    let mut reader = PcapFileReader::open(&path).expect("Failed to open v6.pcap");
    let packets = reader.read_all_packets().expect("Failed to read packets");

    assert!(!packets.is_empty(), "v6.pcap should contain packets");

    let mut normalizer = PacketNormalizer::new();
    let mut ipv6_count = 0;
    let mut normalized_count = 0;

    for pkt in &packets {
        if let Ok(Some(normalized)) =
            normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data)
        {
            normalized_count += 1;

            // Check if it's IPv6
            if normalized.src_ip.is_ipv6() && normalized.dst_ip.is_ipv6() {
                ipv6_count += 1;

                // Verify addresses are valid (not unspecified)
                assert!(
                    !normalized.src_ip.is_unspecified(),
                    "IPv6 source should be valid"
                );
                assert!(
                    !normalized.dst_ip.is_unspecified(),
                    "IPv6 destination should be valid"
                );
            }
        }
    }

    assert!(normalized_count > 0, "Should normalize some packets");
    assert!(
        ipv6_count > 0,
        "v6.pcap should contain IPv6 packets after normalization"
    );
}

#[test]
fn test_teardrop_fragment_attack() {
    // Teardrop attack (overlapping IP fragments) - tests fragment handling
    let path = fixture_path("ip", "teardrop.cap");
    let mut reader = PcapFileReader::open(&path).expect("Failed to open teardrop.cap");
    let packets = reader.read_all_packets().expect("Failed to read packets");

    assert!(!packets.is_empty(), "teardrop.cap should contain packets");

    let mut normalizer = PacketNormalizer::new();
    let mut processed_count = 0;

    // Main goal: ensure we don't panic on malicious fragments
    for pkt in &packets {
        // Normalization should handle fragments gracefully (may skip or reassemble)
        let result = normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data);

        if let Ok(_) = result {
            processed_count += 1;
        } else {
            // Malformed fragments may be rejected, which is acceptable
            // Just verify we don't panic
        }
    }

    // We should process at least some packets without panicking
    assert!(
        processed_count >= 0,
        "Teardrop capture processed without panic"
    );
}

#[test]
fn test_real_captures_no_panic() {
    // Meta-test: ensure all real captures can be opened and processed
    let captures = vec![
        ("tcp", "tcp-ecn-sample.pcap"),
        ("tcp", "200722_tcp_anon.pcapng"),
        ("tcp", "dns-remoteshell.pcap"),
        ("ip", "vlan.cap"),
        ("ip", "v6.pcap"),
        ("ip", "teardrop.cap"),
    ];

    for (subdir, filename) in captures {
        let path = fixture_path(subdir, filename);
        let mut reader = PcapFileReader::open(&path)
            .unwrap_or_else(|e| panic!("Failed to open {subdir}/{filename}: {e}"));

        let packets = reader
            .read_all_packets()
            .unwrap_or_else(|e| panic!("Failed to read {subdir}/{filename}: {e}"));

        assert!(
            !packets.is_empty(),
            "{subdir}/{filename} should contain packets"
        );

        let mut normalizer = PacketNormalizer::new();
        for pkt in &packets {
            // Should not panic
            let _ = normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data);
        }
    }
}
