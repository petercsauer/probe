//! Edge case tests for packet normalization: IPv6, VLAN, truncated, fragmented.

use prb_pcap::{normalize_stateless, NormalizeResult};
use std::net::IpAddr;

/// Helper to create an Ethernet + IPv4 packet.
fn create_ipv4_packet(src_ip: [u8; 4], dst_ip: [u8; 4], payload: &[u8]) -> Vec<u8> {
    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .ipv4(src_ip, dst_ip, 64)
    .udp(12345, 80);

    let mut packet = Vec::new();
    builder.write(&mut packet, payload).unwrap();
    packet
}

/// Helper to create an Ethernet + IPv6 packet.
fn create_ipv6_packet(src_ip: [u8; 16], dst_ip: [u8; 16], payload: &[u8]) -> Vec<u8> {
    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .ipv6(src_ip, dst_ip, 64)
    .udp(12345, 80);

    let mut packet = Vec::new();
    builder.write(&mut packet, payload).unwrap();
    packet
}

/// Helper to create a VLAN-tagged Ethernet frame.
fn create_vlan_packet(vlan_id: u16, inner_ethertype: u16, payload: &[u8]) -> Vec<u8> {
    let mut packet = Vec::new();

    // Ethernet header
    packet.extend_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]); // src MAC
    packet.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]); // dst MAC

    // VLAN tag (0x8100)
    packet.extend_from_slice(&[0x81, 0x00]);

    // TCI: priority (3 bits) + DEI (1 bit) + VID (12 bits)
    let tci = vlan_id & 0x0fff; // VID only, priority=0, DEI=0
    packet.extend_from_slice(&tci.to_be_bytes());

    // Inner EtherType
    packet.extend_from_slice(&inner_ethertype.to_be_bytes());

    // Payload
    packet.extend_from_slice(payload);

    packet
}

#[test]
fn test_normalize_ipv6_basic() {
    let src = [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01];
    let dst = [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02];

    // UDP payload
    let mut udp_packet = Vec::new();
    udp_packet.extend_from_slice(&12345u16.to_be_bytes()); // src port
    udp_packet.extend_from_slice(&80u16.to_be_bytes()); // dst port
    udp_packet.extend_from_slice(&20u16.to_be_bytes()); // length
    udp_packet.extend_from_slice(&0u16.to_be_bytes()); // checksum
    udp_packet.extend_from_slice(b"Hello IPv6");

    let packet = create_ipv6_packet(src, dst, &udp_packet);

    let result = normalize_stateless(1, 1000000, &packet).unwrap();

    if let NormalizeResult::Packet(pkt) = result {
        assert_eq!(pkt.src_ip, IpAddr::from(src));
        assert_eq!(pkt.dst_ip, IpAddr::from(dst));
    } else {
        panic!("Expected Packet result");
    }
}

#[test]
fn test_normalize_vlan_tagged() {
    // Use PacketBuilder to create a VLAN-tagged packet properly
    use etherparse::{PacketBuilder, VlanId};

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .single_vlan(VlanId::try_new(100).unwrap())
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .udp(12345, 80);

    let mut vlan_packet = Vec::new();
    builder.write(&mut vlan_packet, b"VLAN payload").unwrap();

    let result = normalize_stateless(1, 1000000, &vlan_packet).unwrap();

    if let NormalizeResult::Packet(pkt) = result {
        assert_eq!(pkt.vlan_id, Some(100));
        assert_eq!(pkt.src_ip, IpAddr::from([192, 168, 1, 1]));
    } else {
        panic!("Expected Packet result");
    }
}

#[test]
fn test_normalize_vlan_max_id() {
    // Use PacketBuilder for maximum VLAN ID (4095)
    use etherparse::{PacketBuilder, VlanId};

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .single_vlan(VlanId::try_new(4095).unwrap())
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .udp(12345, 80);

    let mut vlan_packet = Vec::new();
    builder.write(&mut vlan_packet, b"test").unwrap();

    let result = normalize_stateless(1, 1000000, &vlan_packet).unwrap();

    if let NormalizeResult::Packet(pkt) = result {
        assert_eq!(pkt.vlan_id, Some(4095));
    } else {
        panic!("Expected Packet result");
    }
}

#[test]
fn test_normalize_truncated_ethernet() {
    // Packet too short for Ethernet header (< 14 bytes)
    let truncated = vec![0xaa; 10];

    let result = normalize_stateless(1, 1000000, &truncated);
    assert!(result.is_err(), "Should error on truncated Ethernet header");
}

#[test]
fn test_normalize_truncated_ip() {
    // Valid Ethernet header but truncated IPv4 header
    let mut packet = Vec::new();

    // Ethernet header
    packet.extend_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]); // src
    packet.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]); // dst
    packet.extend_from_slice(&[0x08, 0x00]); // IPv4 EtherType

    // Truncated IPv4 header (only 10 bytes, need at least 20)
    packet.extend_from_slice(&[0x45, 0x00, 0x00, 0x1c, 0x00, 0x00, 0x00, 0x00, 0x40, 0x11]);

    let result = normalize_stateless(1, 1000000, &packet);
    assert!(result.is_err(), "Should error on truncated IP header");
}

#[test]
fn test_normalize_ipv4_fragment_first() {
    // Create a fragmented IPv4 packet (first fragment with MF=1, offset=0)
    use etherparse::{Ethernet2Header, EtherType, IpNumber, Ipv4Header};

    let mut packet = Vec::new();

    let eth = Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: EtherType(0x0800),
    };
    eth.write(&mut packet).unwrap();

    let mut ipv4 = Ipv4Header::new(100, 64, IpNumber(17), [192, 168, 1, 1], [10, 0, 0, 1]).unwrap();
    ipv4.more_fragments = true;
    ipv4.fragment_offset = 0.try_into().unwrap();
    ipv4.identification = 12345;
    ipv4.write(&mut packet).unwrap();

    packet.extend_from_slice(&[0xaa; 100]); // Fragment payload

    let result = normalize_stateless(1, 1000000, &packet).unwrap();

    match result {
        NormalizeResult::Fragment { .. } => {
            // Fragment detected successfully - first fragment with MF=1, offset=0
        }
        _ => panic!("Expected Fragment result"),
    }
}

#[test]
fn test_normalize_ipv4_fragment_middle() {
    // Middle fragment: MF=1, offset > 0
    use etherparse::{Ethernet2Header, EtherType, IpNumber, Ipv4Header};

    let mut packet = Vec::new();

    let eth = Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: EtherType(0x0800),
    };
    eth.write(&mut packet).unwrap();

    let mut ipv4 = Ipv4Header::new(100, 64, IpNumber(17), [192, 168, 1, 1], [10, 0, 0, 1]).unwrap();
    ipv4.more_fragments = true;
    ipv4.fragment_offset = 100u16.try_into().unwrap(); // Offset in 8-byte units
    ipv4.identification = 12345;
    ipv4.write(&mut packet).unwrap();

    packet.extend_from_slice(&[0xbb; 100]);

    let result = normalize_stateless(1, 1000000, &packet).unwrap();

    match result {
        NormalizeResult::Fragment { .. } => {
            // Fragment detected successfully - middle fragment with MF=1, offset > 0
        }
        _ => panic!("Expected Fragment result"),
    }
}

#[test]
fn test_normalize_ipv4_fragment_last() {
    // Last fragment: MF=0, offset > 0
    use etherparse::{Ethernet2Header, EtherType, IpNumber, Ipv4Header};

    let mut packet = Vec::new();

    let eth = Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: EtherType(0x0800),
    };
    eth.write(&mut packet).unwrap();

    let mut ipv4 = Ipv4Header::new(50, 64, IpNumber(17), [192, 168, 1, 1], [10, 0, 0, 1]).unwrap();
    ipv4.more_fragments = false;
    ipv4.fragment_offset = 200u16.try_into().unwrap();
    ipv4.identification = 12345;
    ipv4.write(&mut packet).unwrap();

    packet.extend_from_slice(&[0xcc; 50]);

    let result = normalize_stateless(1, 1000000, &packet).unwrap();

    match result {
        NormalizeResult::Fragment { .. } => {
            // Fragment detected successfully - last fragment with MF=0, offset > 0
        }
        _ => panic!("Expected Fragment result"),
    }
}

#[test]
fn test_normalize_malformed_udp_header() {
    // Valid Ethernet + IPv4, but truncated UDP header
    use etherparse::{Ethernet2Header, EtherType, IpNumber, Ipv4Header};

    let mut packet = Vec::new();

    let eth = Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: EtherType(0x0800),
    };
    eth.write(&mut packet).unwrap();

    let ipv4 = Ipv4Header::new(10, 64, IpNumber(17), [192, 168, 1, 1], [10, 0, 0, 1]).unwrap();
    ipv4.write(&mut packet).unwrap();

    // Truncated UDP header (only 4 bytes, need 8)
    packet.extend_from_slice(&[0x30, 0x39, 0x00, 0x50]);

    let result = normalize_stateless(1, 1000000, &packet);
    assert!(result.is_err(), "Should error on truncated UDP header");
}

#[test]
fn test_normalize_malformed_tcp_header() {
    // Valid Ethernet + IPv4, but truncated TCP header
    use etherparse::{Ethernet2Header, EtherType, IpNumber, Ipv4Header};

    let mut packet = Vec::new();

    let eth = Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: EtherType(0x0800),
    };
    eth.write(&mut packet).unwrap();

    let ipv4 = Ipv4Header::new(15, 64, IpNumber(6), [192, 168, 1, 1], [10, 0, 0, 1]).unwrap();
    ipv4.write(&mut packet).unwrap();

    // Truncated TCP header (only 15 bytes, need at least 20)
    packet.extend_from_slice(&[0xaa; 15]);

    let result = normalize_stateless(1, 1000000, &packet);
    assert!(result.is_err(), "Should error on truncated TCP header");
}

#[test]
fn test_normalize_icmp_packet() {
    // ICMP packet (protocol 1)
    use etherparse::{Ethernet2Header, EtherType, IpNumber, Ipv4Header};

    let mut packet = Vec::new();

    let eth = Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: EtherType(0x0800),
    };
    eth.write(&mut packet).unwrap();

    let ipv4 = Ipv4Header::new(20, 64, IpNumber(1), [192, 168, 1, 1], [10, 0, 0, 1]).unwrap();
    ipv4.write(&mut packet).unwrap();

    // ICMP Echo Request
    packet.extend_from_slice(&[0x08, 0x00, 0x00, 0x00]); // Type, Code, Checksum
    packet.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]); // ID, Sequence
    packet.extend_from_slice(b"ping payload");

    let result = normalize_stateless(1, 1000000, &packet).unwrap();

    if let NormalizeResult::Packet(pkt) = result {
        // ICMP is handled as "Other" transport
        assert_eq!(pkt.src_ip, IpAddr::from([192, 168, 1, 1]));
        assert_eq!(pkt.dst_ip, IpAddr::from([10, 0, 0, 1]));
    } else {
        panic!("Expected Packet result");
    }
}

#[test]
fn test_normalize_zero_length_payload() {
    // Valid headers but zero-length payload
    use etherparse::{Ethernet2Header, EtherType, IpNumber, Ipv4Header};

    let mut packet = Vec::new();

    let eth = Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: EtherType(0x0800),
    };
    eth.write(&mut packet).unwrap();

    let ipv4 = Ipv4Header::new(8, 64, IpNumber(17), [192, 168, 1, 1], [10, 0, 0, 1]).unwrap();
    ipv4.write(&mut packet).unwrap();

    // UDP header with zero payload
    packet.extend_from_slice(&12345u16.to_be_bytes()); // src port
    packet.extend_from_slice(&80u16.to_be_bytes()); // dst port
    packet.extend_from_slice(&8u16.to_be_bytes()); // length (header only)
    packet.extend_from_slice(&0u16.to_be_bytes()); // checksum

    let result = normalize_stateless(1, 1000000, &packet).unwrap();

    if let NormalizeResult::Packet(pkt) = result {
        assert_eq!(pkt.payload.len(), 0);
    } else {
        panic!("Expected Packet result");
    }
}

#[test]
fn test_normalize_unsupported_ethertype() {
    // Ethernet frame with unsupported EtherType (not IPv4/IPv6)
    let mut packet = Vec::new();

    packet.extend_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]); // src MAC
    packet.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]); // dst MAC
    packet.extend_from_slice(&[0x08, 0x06]); // ARP (0x0806)
    packet.extend_from_slice(&[0xaa; 28]); // ARP payload

    let result = normalize_stateless(1, 1000000, &packet);
    assert!(result.is_err(), "Should error on unsupported EtherType");
}

#[test]
fn test_normalize_timestamp_propagation() {
    let packet = create_ipv4_packet([192, 168, 1, 1], [10, 0, 0, 1], b"test");

    let timestamp_us = 1234567890123456u64;
    let result = normalize_stateless(1, timestamp_us, &packet).unwrap();

    if let NormalizeResult::Packet(pkt) = result {
        assert_eq!(pkt.timestamp_us, timestamp_us);
    } else {
        panic!("Expected Packet result");
    }
}
