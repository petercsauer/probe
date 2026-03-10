//! Integration tests with adversarial and malicious network captures.
//!
//! Tests robustness, no-panic guarantees, and graceful degradation when
//! processing malformed packets, attack patterns, and fuzz corpus inputs.

use prb_pcap::{PacketNormalizer, PcapFileReader, TcpReassembler};
use std::path::PathBuf;
use std::time::Instant;

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
fn test_invalid_file_format_rejected() {
    // Non-pcap file (HTML document) should be rejected gracefully
    // Goal: verify reader rejects invalid files without panic
    let path = fixture_path("adversarial", "malformed-ip.pcap");
    let result = PcapFileReader::open(&path);

    // Should either reject during open or read
    match result {
        Ok(mut reader) => {
            // If open succeeds, read should fail
            let packets_result = reader.read_all_packets();
            assert!(
                packets_result.is_err(),
                "Invalid file format should be rejected"
            );
        }
        Err(_) => {
            // Expected: invalid format rejected during open
        }
    }

    // Main assertion: we reached here without panic
    assert!(true, "Invalid file format handled gracefully");
}

#[test]
fn test_empty_pcap_no_panic() {
    // Empty pcap file (only header, no packets)
    // Goal: verify we handle empty files gracefully
    let path = fixture_path("adversarial", "empty.pcap");
    let mut reader = PcapFileReader::open(&path).expect("Failed to open empty.pcap");
    let packets = reader.read_all_packets().expect("Failed to read packets");

    // Empty pcap should have zero packets
    assert_eq!(packets.len(), 0, "empty.pcap should contain no packets");

    // Processing empty packet list should not panic
    let mut normalizer = PacketNormalizer::new();
    for pkt in packets.iter() {
        let _ = normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data);
    }
}

#[test]
fn test_dns_tunneling_exfiltration_no_panic() {
    // DNS tunneling/data exfiltration capture
    // Goal: verify pipeline handles unusual DNS patterns without panic
    let path = fixture_path("tcp", "dns-remoteshell.pcap");
    let mut reader = PcapFileReader::open(&path).expect("Failed to open dns-remoteshell.pcap");
    let packets = reader.read_all_packets().expect("Failed to read packets");

    assert!(
        !packets.is_empty(),
        "dns-remoteshell.pcap should contain packets"
    );

    let mut normalizer = PacketNormalizer::new();
    let mut reassembler = TcpReassembler::new();
    let mut normalized_count = 0;

    // Process through full pipeline - should not panic
    for pkt in packets.iter() {
        if let Ok(Some(normalized)) = normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data)
        {
            normalized_count += 1;

            if let prb_pcap::TransportInfo::Tcp(_) = normalized.transport {
                // Reassembly should handle exfiltration traffic gracefully
                let _ = reassembler.process_segment(&normalized);
            }
        }
    }

    assert!(
        normalized_count > 0,
        "Should normalize some packets from DNS tunneling capture"
    );
}

#[test]
fn test_teardrop_overlapping_fragments_no_panic() {
    // Teardrop attack: overlapping IP fragments designed to crash systems
    // Goal: verify fragment reassembly doesn't panic or corrupt state
    let path = fixture_path("ip", "teardrop.cap");
    let mut reader = PcapFileReader::open(&path).expect("Failed to open teardrop.cap");
    let packets = reader.read_all_packets().expect("Failed to read packets");

    assert!(!packets.is_empty(), "teardrop.cap should contain packets");

    let mut normalizer = PacketNormalizer::new();
    let mut processed = 0;
    let mut rejected = 0;

    for pkt in packets.iter() {
        // Parser may reject overlapping fragments or process them
        // Either behavior is acceptable as long as we don't panic
        match normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data) {
            Ok(Some(_)) => processed += 1,
            Ok(None) | Err(_) => rejected += 1,
        }
    }

    assert!(
        processed + rejected == packets.len(),
        "All packets should be processed or rejected, no panic"
    );
}

#[test]
fn test_nanosecond_timestamp_edge_case() {
    // PCAP with nanosecond resolution timestamps
    // Goal: verify unsupported timestamp formats are rejected gracefully
    let path = fixture_path("adversarial", "dhcp-nanosecond.pcap");
    let result = PcapFileReader::open(&path);

    match result {
        Ok(mut reader) => {
            // If open succeeds, try reading
            let packets_result = reader.read_all_packets();
            match packets_result {
                Ok(packets) => {
                    // If it works, verify timestamps are reasonable
                    let mut normalizer = PacketNormalizer::new();
                    for pkt in packets.iter() {
                        if let Ok(Some(normalized)) =
                            normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data)
                        {
                            assert!(
                                normalized.timestamp_us > 0,
                                "Timestamp should be positive"
                            );
                        }
                    }
                }
                Err(_) => {
                    // Acceptable: unsupported nanosecond format rejected
                }
            }
        }
        Err(_) => {
            // Expected: nanosecond format not supported, rejected gracefully
        }
    }

    // Main assertion: we reached here without panic
    assert!(true, "Nanosecond timestamp format handled gracefully");
}

#[test]
fn test_vlan_tagged_adversarial() {
    // VLAN-tagged frames with potential edge cases
    // Goal: verify VLAN stripping doesn't cause issues
    let path = fixture_path("ip", "vlan.cap");
    let mut reader = PcapFileReader::open(&path).expect("Failed to open vlan.cap");
    let packets = reader.read_all_packets().expect("Failed to read packets");

    assert!(!packets.is_empty(), "vlan.cap should contain packets");

    let mut normalizer = PacketNormalizer::new();
    let mut processed = 0;

    for pkt in packets.iter() {
        // VLAN processing should not panic
        match normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data) {
            Ok(Some(_)) => processed += 1,
            Ok(None) | Err(_) => {}
        }
    }

    assert!(processed > 0, "Should process VLAN-tagged packets");
}

#[test]
fn test_ipv6_adversarial_patterns() {
    // IPv6 packets with various edge cases
    // Goal: verify IPv6 handling is robust
    let path = fixture_path("ip", "v6.pcap");
    let mut reader = PcapFileReader::open(&path).expect("Failed to open v6.pcap");
    let packets = reader.read_all_packets().expect("Failed to read packets");

    assert!(!packets.is_empty(), "v6.pcap should contain packets");

    let mut normalizer = PacketNormalizer::new();
    let mut ipv6_count = 0;

    for pkt in packets.iter() {
        // IPv6 processing should not panic
        if let Ok(Some(normalized)) = normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data)
        {
            if normalized.src_ip.is_ipv6() {
                ipv6_count += 1;
            }
        }
    }

    assert!(ipv6_count > 0, "Should process IPv6 packets");
}

#[test]
fn test_tcp_edge_cases_no_panic() {
    // TCP packets with ECN flags and various edge cases
    // Goal: verify TCP parser handles unusual flag combinations
    let path = fixture_path("tcp", "tcp-ecn-sample.pcap");
    let mut reader = PcapFileReader::open(&path).expect("Failed to open tcp-ecn-sample.pcap");
    let packets = reader.read_all_packets().expect("Failed to read packets");

    assert!(
        !packets.is_empty(),
        "tcp-ecn-sample.pcap should contain packets"
    );

    let mut normalizer = PacketNormalizer::new();
    let mut reassembler = TcpReassembler::new();
    let mut tcp_packets = 0;

    for pkt in packets.iter() {
        if let Ok(Some(normalized)) = normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data)
        {
            if let prb_pcap::TransportInfo::Tcp(_) = normalized.transport {
                tcp_packets += 1;
                // Reassembler should handle ECN and edge case flags gracefully
                let _ = reassembler.process_segment(&normalized);
            }
        }
    }

    assert!(tcp_packets > 0, "Should process TCP packets with ECN flags");
}

#[test]
fn test_mixed_adversarial_captures_no_panic() {
    // Meta-test: process multiple adversarial captures in sequence
    // Goal: verify state doesn't corrupt across different malicious inputs
    let captures = vec![
        ("adversarial", "empty.pcap"),
        ("tcp", "dns-remoteshell.pcap"),
        ("ip", "teardrop.cap"),
        ("adversarial", "dhcp-nanosecond.pcap"),
        ("tcp", "tcp-ecn-sample.pcap"),
        ("ip", "v6.pcap"),
    ];

    let mut total_processed = 0;
    let mut total_errors = 0;

    for (subdir, filename) in captures {
        let path = fixture_path(subdir, filename);
        let open_result = PcapFileReader::open(&path);

        match open_result {
            Ok(mut reader) => {
                let packets_result = reader.read_all_packets();
                match packets_result {
                    Ok(packets) => {
                        let mut normalizer = PacketNormalizer::new();
                        let mut reassembler = TcpReassembler::new();

                        for pkt in packets.iter() {
                            match normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data) {
                                Ok(Some(normalized)) => {
                                    total_processed += 1;
                                    // Process through TCP reassembler if applicable
                                    if let prb_pcap::TransportInfo::Tcp(_) = normalized.transport {
                                        let _ = reassembler.process_segment(&normalized);
                                    }
                                }
                                Ok(None) | Err(_) => {
                                    total_errors += 1;
                                }
                            }
                        }
                    }
                    Err(_) => {
                        total_errors += 1;
                    }
                }
            }
            Err(_) => {
                total_errors += 1;
            }
        }
    }

    // Main assertion: we processed multiple adversarial inputs without panic
    assert!(
        total_processed + total_errors > 0,
        "Mixed adversarial captures processed without panic"
    );
}

#[test]
fn test_performance_bounded_adversarial() {
    // Performance test: verify adversarial input doesn't cause infinite loops
    // Goal: ensure all adversarial captures complete within reasonable time
    let captures = vec![
        ("adversarial", "dhcp-nanosecond.pcap"),
        ("tcp", "dns-remoteshell.pcap"),
        ("ip", "teardrop.cap"),
        ("ip", "v6.pcap"),
    ];

    for (subdir, filename) in captures {
        let path = fixture_path(subdir, filename);
        let start = Instant::now();

        let open_result = PcapFileReader::open(&path);
        if let Ok(mut reader) = open_result {
            if let Ok(packets) = reader.read_all_packets() {
                let mut normalizer = PacketNormalizer::new();
                let mut reassembler = TcpReassembler::new();

                for pkt in packets.iter() {
                    if let Ok(Some(normalized)) =
                        normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data)
                    {
                        if let prb_pcap::TransportInfo::Tcp(_) = normalized.transport {
                            let _ = reassembler.process_segment(&normalized);
                        }
                    }
                }
            }
        }

        let elapsed = start.elapsed();
        // Each small adversarial capture should process in under 5 seconds
        assert!(
            elapsed.as_secs() < 5,
            "{}/{} took too long: {:?}",
            subdir,
            filename,
            elapsed
        );
    }
}

#[test]
fn test_memory_bounded_adversarial() {
    // Memory test: verify reassembler doesn't accumulate unbounded state
    // Goal: ensure adversarial inputs don't cause OOM
    let path = fixture_path("tcp", "dns-remoteshell.pcap");
    let mut reader = PcapFileReader::open(&path).expect("Failed to open dns-remoteshell.pcap");
    let packets = reader.read_all_packets().expect("Failed to read packets");

    let mut normalizer = PacketNormalizer::new();
    let mut reassembler = TcpReassembler::new();

    // Process packets twice to simulate repeated malicious connections
    for _ in 0..2 {
        for pkt in packets.iter() {
            if let Ok(Some(normalized)) =
                normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data)
            {
                if let prb_pcap::TransportInfo::Tcp(_) = normalized.transport {
                    let _ = reassembler.process_segment(&normalized);
                }
            }
        }
    }

    // Verify reassembler tracks reasonable number of connections
    let active = reassembler.active_connections();
    assert!(
        active < 10000,
        "Reassembler should not accumulate unbounded connections: {}",
        active
    );
}
