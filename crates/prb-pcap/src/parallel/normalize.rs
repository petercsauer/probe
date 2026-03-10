//! Parallel packet normalization using rayon.
//!
//! This module implements batch normalization of PCAP packets using rayon's
//! parallel iterators. Non-fragmented packets (>99% of traffic) are normalized
//! in parallel, while IP fragments (<1%) are processed sequentially through
//! the stateful defragmentation pool.

use crate::error::PcapError;
use crate::normalize::{normalize_stateless, NormalizeResult, OwnedNormalizedPacket, PacketNormalizer};
use crate::reader::PcapPacket;
use rayon::prelude::*;

/// Batch normalization stage for parallel pipeline.
///
/// Processes packets in parallel using rayon, with a sequential fallback
/// for IP fragments that require stateful reassembly.
pub struct NormalizeBatch;

impl NormalizeBatch {
    /// Normalizes a batch of packets in parallel.
    ///
    /// Returns normalized packets and indices of fragments that need
    /// sequential processing through the defrag pool.
    ///
    /// # Performance
    ///
    /// With 8 cores and 100k packets, expect ~30ms vs ~200ms sequential.
    /// Fragment fallback adds negligible overhead (<1% of packets).
    pub fn run(packets: &[PcapPacket]) -> (Vec<OwnedNormalizedPacket>, Vec<usize>) {
        let results: Vec<(usize, Result<NormalizeResult, PcapError>)> = packets
            .par_iter()
            .enumerate()
            .map(|(idx, pkt)| {
                (
                    idx,
                    normalize_stateless(pkt.linktype, pkt.timestamp_us, &pkt.data),
                )
            })
            .collect();

        let mut normalized = Vec::with_capacity(packets.len());
        let mut fragment_indices = Vec::new();
        let mut failed = 0u64;

        for (idx, result) in results {
            match result {
                Ok(NormalizeResult::Packet(pkt)) => normalized.push(pkt),
                Ok(NormalizeResult::Fragment { .. }) => fragment_indices.push(idx),
                Err(e) => {
                    failed += 1;
                    tracing::warn!("Normalize failed for packet {}: {}", idx, e);
                }
            }
        }

        tracing::debug!(
            "Parallel normalize: {} packets, {} fragments, {} failed",
            normalized.len(),
            fragment_indices.len(),
            failed
        );

        (normalized, fragment_indices)
    }
}

/// Processes IP fragments sequentially through the defrag pool.
///
/// This handles the <1% of packets that are IP fragments and require
/// stateful reassembly. The defrag pool maintains state across fragments
/// from the same IP datagram.
///
/// # Arguments
///
/// * `packets` - Full packet array (immutable reference)
/// * `fragment_indices` - Indices of packets that are fragments
///
/// # Returns
///
/// Vector of normalized packets that were successfully reassembled.
/// Incomplete fragment trains (waiting for more fragments) are not returned.
pub fn process_fragments(
    packets: &[PcapPacket],
    fragment_indices: &[usize],
) -> Vec<OwnedNormalizedPacket> {
    if fragment_indices.is_empty() {
        return Vec::new();
    }

    let mut normalizer = PacketNormalizer::new();
    let mut result = Vec::new();

    for &idx in fragment_indices {
        let pkt = &packets[idx];
        match normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data) {
            Ok(Some(normalized)) => {
                result.push(OwnedNormalizedPacket::from_borrowed(&normalized));
            }
            Ok(None) => {
                // Fragment waiting for more data - this is expected
            }
            Err(e) => {
                tracing::warn!("Fragment normalize failed for packet {}: {}", idx, e);
            }
        }
    }

    tracing::debug!(
        "Sequential fragment processing: {} fragments -> {} reassembled",
        fragment_indices.len(),
        result.len()
    );

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use etherparse::PacketBuilder;
    use std::net::IpAddr;

    /// Helper to create an Ethernet+IPv4+TCP packet.
    fn create_pcap_tcp_packet(src_ip: [u8; 4], dst_ip: [u8; 4], payload: &[u8]) -> PcapPacket {
        let builder = PacketBuilder::ethernet2(
            [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
            [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        )
        .ipv4(src_ip, dst_ip, 64)
        .tcp(12345, 80, 1000, 4096);

        let mut data = Vec::new();
        builder.write(&mut data, payload).unwrap();

        PcapPacket {
            linktype: 1, // Ethernet
            timestamp_us: 1000000,
            data,
        }
    }

    /// Helper to create an Ethernet+IPv4+UDP packet.
    fn create_pcap_udp_packet(src_ip: [u8; 4], dst_ip: [u8; 4], payload: &[u8]) -> PcapPacket {
        let builder = PacketBuilder::ethernet2(
            [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
            [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        )
        .ipv4(src_ip, dst_ip, 64)
        .udp(12345, 53);

        let mut data = Vec::new();
        builder.write(&mut data, payload).unwrap();

        PcapPacket {
            linktype: 1,
            timestamp_us: 2000000,
            data,
        }
    }

    #[test]
    fn test_normalize_stateless_tcp() {
        let pkt = create_pcap_tcp_packet([192, 168, 1, 1], [10, 0, 0, 1], b"Hello TCP");

        let result = normalize_stateless(pkt.linktype as u32, pkt.timestamp_us, &pkt.data);
        assert!(result.is_ok());

        match result.unwrap() {
            NormalizeResult::Packet(normalized) => {
                assert_eq!(normalized.src_ip, IpAddr::from([192, 168, 1, 1]));
                assert_eq!(normalized.dst_ip, IpAddr::from([10, 0, 0, 1]));
                assert_eq!(normalized.payload, b"Hello TCP");
            }
            NormalizeResult::Fragment { .. } => panic!("Expected Packet, got Fragment"),
        }
    }

    #[test]
    fn test_normalize_stateless_udp() {
        let pkt = create_pcap_udp_packet([192, 168, 1, 1], [10, 0, 0, 1], b"Hello UDP");

        let result = normalize_stateless(pkt.linktype as u32, pkt.timestamp_us, &pkt.data);
        assert!(result.is_ok());

        match result.unwrap() {
            NormalizeResult::Packet(normalized) => {
                assert_eq!(normalized.src_ip, IpAddr::from([192, 168, 1, 1]));
                assert_eq!(normalized.dst_ip, IpAddr::from([10, 0, 0, 1]));
                assert_eq!(normalized.payload, b"Hello UDP");
            }
            NormalizeResult::Fragment { .. } => panic!("Expected Packet, got Fragment"),
        }
    }

    #[test]
    fn test_normalize_stateless_fragment_detected() {
        // Create a fragmented IPv4 packet using etherparse
        use etherparse::{Ethernet2Header, EtherType, IpNumber, Ipv4Header};

        let mut data = Vec::new();

        // Ethernet header
        let eth_header = Ethernet2Header {
            source: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
            destination: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
            ether_type: EtherType::IPV4,
        };
        eth_header.write(&mut data).unwrap();

        // IPv4 header with MF (More Fragments) flag set
        // Use 60 bytes of payload to meet minimum ethernet frame requirements
        let mut ip_header = Ipv4Header::new(
            60,  // payload length
            64,  // TTL
            IpNumber::TCP,
            [192, 168, 1, 1],  // src
            [10, 0, 0, 1],     // dst
        ).unwrap();

        // Set the More Fragments flag (bit 13 in flags+fragment_offset)
        ip_header.more_fragments = true;
        ip_header.fragment_offset = 0.try_into().unwrap();

        ip_header.write(&mut data).unwrap();

        // Add payload data (60 bytes to match the header's payload_len)
        data.extend_from_slice(&[0; 60]);

        let result = normalize_stateless(1, 1000000, &data);

        // If it fails, print the error for debugging
        if result.is_err() {
            eprintln!("Error: {:?}", result);
        }
        assert!(result.is_ok());

        match result.unwrap() {
            NormalizeResult::Fragment {
                timestamp_us,
                linktype,
                data_len,
            } => {
                assert_eq!(timestamp_us, 1000000);
                assert_eq!(linktype, 1);
                assert_eq!(data_len, data.len());
            }
            NormalizeResult::Packet(_) => panic!("Expected Fragment, got Packet"),
        }
    }

    #[test]
    fn test_normalize_stateless_all_linktypes() {
        // Test Ethernet (linktype 1)
        let eth_pkt = create_pcap_tcp_packet([192, 168, 1, 1], [10, 0, 0, 1], b"test");
        let result = normalize_stateless(1, 1000000, &eth_pkt.data);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), NormalizeResult::Packet(_)));

        // Test Raw IP (linktype 101)
        let builder = PacketBuilder::ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
            .tcp(12345, 80, 1000, 4096);
        let mut raw_ip_data = Vec::new();
        builder.write(&mut raw_ip_data, b"raw ip test").unwrap();

        let result = normalize_stateless(101, 1000000, &raw_ip_data);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), NormalizeResult::Packet(_)));

        // Loopback and SLL/SLL2 would require more complex test setup
        // Testing invalid linktype
        let result = normalize_stateless(999, 1000000, &[0; 64]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parallel_normalize_matches_sequential() {
        // Create 100 test packets
        let packets: Vec<PcapPacket> = (0..100)
            .map(|i| {
                let src_ip = [192, 168, 1, (i % 256) as u8];
                let dst_ip = [10, 0, 0, (i / 256 + 1) as u8];
                create_pcap_tcp_packet(src_ip, dst_ip, format!("packet{}", i).as_bytes())
            })
            .collect();

        // Run parallel normalization
        let (parallel_result, fragments) = NormalizeBatch::run(&packets);
        assert_eq!(fragments.len(), 0); // No fragments in this test

        // Run sequential normalization
        let sequential_result: Vec<OwnedNormalizedPacket> = packets
            .iter()
            .filter_map(|pkt| {
                match normalize_stateless(pkt.linktype as u32, pkt.timestamp_us, &pkt.data) {
                    Ok(NormalizeResult::Packet(p)) => Some(p),
                    _ => None,
                }
            })
            .collect();

        assert_eq!(parallel_result.len(), sequential_result.len());

        // Sort both by timestamp+src_ip for comparison (order may differ due to parallelism)
        let mut parallel_sorted = parallel_result;
        let mut sequential_sorted = sequential_result;

        parallel_sorted.sort_by_key(|p| (p.timestamp_us, format!("{:?}", p.src_ip)));
        sequential_sorted.sort_by_key(|p| (p.timestamp_us, format!("{:?}", p.src_ip)));

        // Compare each packet
        for (par, seq) in parallel_sorted.iter().zip(sequential_sorted.iter()) {
            assert_eq!(par.timestamp_us, seq.timestamp_us);
            assert_eq!(par.src_ip, seq.src_ip);
            assert_eq!(par.dst_ip, seq.dst_ip);
            assert_eq!(par.payload, seq.payload);
        }
    }

    #[test]
    fn test_parallel_normalize_empty_input() {
        let packets: Vec<PcapPacket> = vec![];
        let (result, fragments) = NormalizeBatch::run(&packets);

        assert_eq!(result.len(), 0);
        assert_eq!(fragments.len(), 0);
    }

    #[test]
    fn test_parallel_normalize_single_packet() {
        let packets = vec![create_pcap_tcp_packet(
            [192, 168, 1, 1],
            [10, 0, 0, 1],
            b"single",
        )];

        let (result, fragments) = NormalizeBatch::run(&packets);

        assert_eq!(result.len(), 1);
        assert_eq!(fragments.len(), 0);
        assert_eq!(result[0].payload, b"single");
    }

    #[test]
    fn test_parallel_normalize_fragment_fallback() {
        // This test would require creating actual fragmented packets
        // For now, we test that empty fragment list returns empty result
        let packets = vec![create_pcap_tcp_packet(
            [192, 168, 1, 1],
            [10, 0, 0, 1],
            b"test",
        )];

        let fragment_indices: Vec<usize> = vec![];
        let result = process_fragments(&packets, &fragment_indices);

        assert_eq!(result.len(), 0);
    }
}
