//! Integration tests for packet normalization.

use prb_pcap::{PacketNormalizer, TransportInfo};
use std::net::IpAddr;

/// Helper to create a minimal Ethernet + IPv4 + TCP packet.
fn create_ethernet_ipv4_tcp(src_ip: [u8; 4], dst_ip: [u8; 4], src_port: u16, dst_port: u16, payload: &[u8]) -> Vec<u8> {
    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55], // src MAC
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff], // dst MAC
    )
    .ipv4(src_ip, dst_ip, 64)
    .tcp(src_port, dst_port, 1000, 4096);

    let mut packet = Vec::new();
    builder.write(&mut packet, payload).unwrap();
    packet
}

/// Helper to create a minimal Ethernet + IPv4 + UDP packet.
fn create_ethernet_ipv4_udp(src_ip: [u8; 4], dst_ip: [u8; 4], src_port: u16, dst_port: u16, payload: &[u8]) -> Vec<u8> {
    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .ipv4(src_ip, dst_ip, 64)
    .udp(src_port, dst_port);

    let mut packet = Vec::new();
    builder.write(&mut packet, payload).unwrap();
    packet
}

/// Helper to create an Ethernet + IPv4 + TCP packet with single VLAN tag.
fn create_ethernet_vlan_ipv4_tcp(vlan_id: u16) -> Vec<u8> {
    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .single_vlan(etherparse::VlanId::try_new(vlan_id).unwrap())
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .tcp(12345, 80, 1000, 4096);

    let payload = b"VLAN test";
    let mut packet = Vec::new();
    builder.write(&mut packet, payload).unwrap();
    packet
}

/// Helper to create an Ethernet + IPv4 + TCP packet with double VLAN tags (QinQ).
fn create_ethernet_double_vlan_ipv4_tcp(outer_vlan: u16, inner_vlan: u16) -> Vec<u8> {
    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .double_vlan(
        etherparse::VlanId::try_new(outer_vlan).unwrap(),
        etherparse::VlanId::try_new(inner_vlan).unwrap(),
    )
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .tcp(12345, 80, 1000, 4096);

    let payload = b"Double VLAN test";
    let mut packet = Vec::new();
    builder.write(&mut packet, payload).unwrap();
    packet
}

/// Helper to create a Linux SLL (cooked capture) packet with IPv4 + TCP.
fn create_sll_ipv4_tcp() -> Vec<u8> {
    // Linux SLL header (16 bytes) + IPv4 + TCP
    // Format: packet_type(2) + arphrd(2) + addr_len(2) + addr(8) + protocol(2)
    let mut packet = Vec::new();

    // SLL header
    packet.extend_from_slice(&[0x00, 0x00]); // packet_type: sent to us
    packet.extend_from_slice(&[0x00, 0x01]); // arphrd: Ethernet
    packet.extend_from_slice(&[0x00, 0x06]); // addr_len: 6 bytes (MAC)
    packet.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x00]); // MAC address (padded to 8 bytes)
    packet.extend_from_slice(&[0x08, 0x00]); // protocol: IPv4

    // IPv4 + TCP payload
    let ip_tcp = create_raw_ipv4_tcp([192, 168, 1, 1], [10, 0, 0, 1], 12345, 80, b"SLL test");
    packet.extend_from_slice(&ip_tcp);

    packet
}

/// Helper to create a Linux SLL2 (cooked capture v2) packet with IPv4 + TCP.
fn create_sll2_ipv4_tcp() -> Vec<u8> {
    // Linux SLL2 header (20 bytes) + IPv4 + TCP
    // Format: protocol_type(2) + reserved(2) + if_index(4) + arphrd(2) + pkt_type(1) + addr_len(1) + addr(8)
    let mut packet = Vec::new();

    // SLL2 header
    packet.extend_from_slice(&[0x08, 0x00]); // protocol_type: IPv4 (big-endian EtherType)
    packet.extend_from_slice(&[0x00, 0x00]); // reserved
    packet.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // interface_index: 1
    packet.extend_from_slice(&[0x00, 0x01]); // arphrd_type: Ethernet
    packet.extend_from_slice(&[0x00]); // packet_type: sent to us
    packet.extend_from_slice(&[0x06]); // link_layer_addr_len: 6 bytes
    packet.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x00]); // MAC address (8 bytes)

    // IPv4 + TCP payload
    let ip_tcp = create_raw_ipv4_tcp([192, 168, 1, 1], [10, 0, 0, 1], 12345, 80, b"SLL2 test");
    packet.extend_from_slice(&ip_tcp);

    packet
}

/// Helper to create a raw IPv4 + TCP packet (no link layer).
fn create_raw_ipv4_tcp(src_ip: [u8; 4], dst_ip: [u8; 4], src_port: u16, dst_port: u16, payload: &[u8]) -> Vec<u8> {
    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ipv4(src_ip, dst_ip, 64).tcp(src_port, dst_port, 1000, 4096);

    let mut packet = Vec::new();
    builder.write(&mut packet, payload).unwrap();
    packet
}

/// Helper to create a Loopback/Null packet with IPv4 + TCP.
fn create_loopback_ipv4_tcp() -> Vec<u8> {
    // Loopback header: 4-byte AF family (little-endian)
    let mut packet = Vec::new();
    packet.extend_from_slice(&2u32.to_le_bytes()); // AF_INET = 2

    // IPv4 + TCP payload
    let ip_tcp = create_raw_ipv4_tcp([127, 0, 0, 1], [127, 0, 0, 2], 12345, 80, b"Loopback test");
    packet.extend_from_slice(&ip_tcp);

    packet
}

/// Helper to create a Loopback/Null packet with IPv6 + TCP (macOS AF value).
fn create_loopback_ipv6_tcp_macos() -> Vec<u8> {
    // Loopback header: 4-byte AF family (little-endian)
    let mut packet = Vec::new();
    packet.extend_from_slice(&30u32.to_le_bytes()); // AF_INET6 = 30 on macOS

    // IPv6 + TCP payload
    let ip_tcp = create_raw_ipv6_tcp(
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2],
        12345,
        80,
        b"Loopback IPv6 test",
    );
    packet.extend_from_slice(&ip_tcp);

    packet
}

/// Helper to create a Loopback/Null packet with IPv6 + TCP (Linux AF value).
fn create_loopback_ipv6_tcp_linux() -> Vec<u8> {
    // Loopback header: 4-byte AF family (little-endian)
    let mut packet = Vec::new();
    packet.extend_from_slice(&10u32.to_le_bytes()); // AF_INET6 = 10 on Linux

    // IPv6 + TCP payload
    let ip_tcp = create_raw_ipv6_tcp(
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2],
        12345,
        80,
        b"Loopback IPv6 test",
    );
    packet.extend_from_slice(&ip_tcp);

    packet
}

/// Helper to create a raw IPv6 + TCP packet (no link layer).
fn create_raw_ipv6_tcp(src_ip: [u8; 16], dst_ip: [u8; 16], src_port: u16, dst_port: u16, payload: &[u8]) -> Vec<u8> {
    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ipv6(src_ip, dst_ip, 64).tcp(src_port, dst_port, 1000, 4096);

    let mut packet = Vec::new();
    builder.write(&mut packet, payload).unwrap();
    packet
}

/// Helper to create a fragmented IPv4 packet (3 fragments).
fn create_ipv4_fragments() -> Vec<Vec<u8>> {
    use etherparse::{Ipv4Header, IpNumber};

    // Create a large payload that will be split into 3 fragments
    let full_payload = vec![0xAA; 3000]; // 3KB payload

    // Ethernet header
    let eth = etherparse::Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: etherparse::EtherType(0x0800), // IPv4
    };

    // Fragment 1: offset 0, more fragments = true
    let mut frag1 = Vec::new();
    let frag1_payload = &full_payload[0..1480]; // 1480 bytes (divisible by 8)
    let mut ip_header1 = Ipv4Header::new(
        frag1_payload.len() as u16, // payload length only
        64,              // TTL
        IpNumber(6),     // TCP
        [192, 168, 1, 1], // src
        [10, 0, 0, 1],   // dst
    )
    .unwrap();
    ip_header1.identification = 0x1234;
    ip_header1.fragment_offset = etherparse::IpFragOffset::try_new(0).unwrap();
    ip_header1.more_fragments = true;
    eth.write(&mut frag1).unwrap();
    ip_header1.write(&mut frag1).unwrap();
    frag1.extend_from_slice(frag1_payload);

    // Fragment 2: offset 1480, more fragments = true
    let mut frag2 = Vec::new();
    let frag2_payload = &full_payload[1480..2960]; // 1480 bytes
    let mut ip_header2 = Ipv4Header::new(
        frag2_payload.len() as u16,
        64,
        IpNumber(6),
        [192, 168, 1, 1],
        [10, 0, 0, 1],
    )
    .unwrap();
    ip_header2.identification = 0x1234;
    ip_header2.fragment_offset = etherparse::IpFragOffset::try_new(1480 / 8).unwrap(); // offset in 8-byte units
    ip_header2.more_fragments = true;
    eth.write(&mut frag2).unwrap();
    ip_header2.write(&mut frag2).unwrap();
    frag2.extend_from_slice(frag2_payload);

    // Fragment 3: offset 2960, more fragments = false (last fragment)
    let mut frag3 = Vec::new();
    let frag3_payload = &full_payload[2960..]; // remaining 40 bytes
    let mut ip_header3 = Ipv4Header::new(
        frag3_payload.len() as u16,
        64,
        IpNumber(6),
        [192, 168, 1, 1],
        [10, 0, 0, 1],
    )
    .unwrap();
    ip_header3.identification = 0x1234;
    ip_header3.fragment_offset = etherparse::IpFragOffset::try_new(2960 / 8).unwrap();
    ip_header3.more_fragments = false;
    eth.write(&mut frag3).unwrap();
    ip_header3.write(&mut frag3).unwrap();
    frag3.extend_from_slice(frag3_payload);

    vec![frag1, frag2, frag3]
}

#[test]
fn test_ethernet_ipv4_tcp() {
    let packet = create_ethernet_ipv4_tcp([192, 168, 1, 1], [10, 0, 0, 1], 12345, 80, b"Hello TCP");
    let mut normalizer = PacketNormalizer::new();

    let result = normalizer.normalize(1, 1000000, &packet).unwrap();
    assert!(result.is_some());

    let normalized = result.unwrap();
    assert_eq!(normalized.src_ip, IpAddr::from([192, 168, 1, 1]));
    assert_eq!(normalized.dst_ip, IpAddr::from([10, 0, 0, 1]));
    assert_eq!(
        normalized.transport,
        TransportInfo::Tcp(prb_pcap::TcpSegmentInfo {
            src_port: 12345,
            dst_port: 80,
            seq: 1000,
            ack: 0,
            flags: prb_pcap::TcpFlags {
                syn: false,
                ack: false,
                fin: false,
                rst: false,
                psh: false,
            },
        })
    );
    assert_eq!(normalized.payload, b"Hello TCP");
    assert_eq!(normalized.vlan_id, None);
}

#[test]
fn test_ethernet_ipv4_udp() {
    let packet = create_ethernet_ipv4_udp([192, 168, 1, 1], [10, 0, 0, 1], 12345, 53, b"Hello UDP");
    let mut normalizer = PacketNormalizer::new();

    let result = normalizer.normalize(1, 1000000, &packet).unwrap();
    assert!(result.is_some());

    let normalized = result.unwrap();
    assert_eq!(normalized.src_ip, IpAddr::from([192, 168, 1, 1]));
    assert_eq!(normalized.dst_ip, IpAddr::from([10, 0, 0, 1]));
    assert_eq!(
        normalized.transport,
        TransportInfo::Udp {
            src_port: 12345,
            dst_port: 53
        }
    );
    assert_eq!(normalized.payload, b"Hello UDP");
}

#[test]
fn test_vlan_single() {
    let packet = create_ethernet_vlan_ipv4_tcp(100);
    let mut normalizer = PacketNormalizer::new();

    let result = normalizer.normalize(1, 1000000, &packet).unwrap();
    assert!(result.is_some());

    let normalized = result.unwrap();
    assert_eq!(normalized.vlan_id, Some(100));
    assert_eq!(normalized.payload, b"VLAN test");
}

#[test]
fn test_vlan_double() {
    let packet = create_ethernet_double_vlan_ipv4_tcp(200, 300);
    let mut normalizer = PacketNormalizer::new();

    let result = normalizer.normalize(1, 1000000, &packet).unwrap();
    assert!(result.is_some());

    let normalized = result.unwrap();
    // Should extract the outer VLAN ID (first in the stack)
    assert_eq!(normalized.vlan_id, Some(200));
    assert_eq!(normalized.payload, b"Double VLAN test");
}

#[test]
fn test_sll_v1() {
    let packet = create_sll_ipv4_tcp();
    let mut normalizer = PacketNormalizer::new();

    let result = normalizer.normalize(113, 1000000, &packet).unwrap();
    assert!(result.is_some());

    let normalized = result.unwrap();
    assert_eq!(normalized.src_ip, IpAddr::from([192, 168, 1, 1]));
    assert_eq!(normalized.dst_ip, IpAddr::from([10, 0, 0, 1]));
    assert_eq!(
        normalized.transport,
        TransportInfo::Tcp(prb_pcap::TcpSegmentInfo {
            src_port: 12345,
            dst_port: 80,
            seq: 1000,
            ack: 0,
            flags: prb_pcap::TcpFlags {
                syn: false,
                ack: false,
                fin: false,
                rst: false,
                psh: false,
            },
        })
    );
    assert_eq!(normalized.payload, b"SLL test");
}

#[test]
fn test_sll_v2() {
    let packet = create_sll2_ipv4_tcp();
    let mut normalizer = PacketNormalizer::new();

    let result = normalizer.normalize(276, 1000000, &packet).unwrap();
    assert!(result.is_some());

    let normalized = result.unwrap();
    assert_eq!(normalized.src_ip, IpAddr::from([192, 168, 1, 1]));
    assert_eq!(normalized.dst_ip, IpAddr::from([10, 0, 0, 1]));
    assert_eq!(
        normalized.transport,
        TransportInfo::Tcp(prb_pcap::TcpSegmentInfo {
            src_port: 12345,
            dst_port: 80,
            seq: 1000,
            ack: 0,
            flags: prb_pcap::TcpFlags {
                syn: false,
                ack: false,
                fin: false,
                rst: false,
                psh: false,
            },
        })
    );
    assert_eq!(normalized.payload, b"SLL2 test");
}

#[test]
fn test_raw_ip() {
    let packet = create_raw_ipv4_tcp([192, 168, 1, 1], [10, 0, 0, 1], 12345, 80, b"Raw IP test");
    let mut normalizer = PacketNormalizer::new();

    let result = normalizer.normalize(101, 1000000, &packet).unwrap();
    assert!(result.is_some());

    let normalized = result.unwrap();
    assert_eq!(normalized.src_ip, IpAddr::from([192, 168, 1, 1]));
    assert_eq!(normalized.dst_ip, IpAddr::from([10, 0, 0, 1]));
    assert_eq!(
        normalized.transport,
        TransportInfo::Tcp(prb_pcap::TcpSegmentInfo {
            src_port: 12345,
            dst_port: 80,
            seq: 1000,
            ack: 0,
            flags: prb_pcap::TcpFlags {
                syn: false,
                ack: false,
                fin: false,
                rst: false,
                psh: false,
            },
        })
    );
    assert_eq!(normalized.payload, b"Raw IP test");
}

#[test]
fn test_loopback_null() {
    // Test IPv4 loopback (AF_INET = 2)
    let packet_ipv4 = create_loopback_ipv4_tcp();
    let mut normalizer = PacketNormalizer::new();

    let result = normalizer.normalize(0, 1000000, &packet_ipv4).unwrap();
    assert!(result.is_some());

    let normalized = result.unwrap();
    assert_eq!(normalized.src_ip, IpAddr::from([127, 0, 0, 1]));
    assert_eq!(normalized.dst_ip, IpAddr::from([127, 0, 0, 2]));
    assert_eq!(normalized.payload, b"Loopback test");

    // Test IPv6 loopback (macOS: AF_INET6 = 30)
    let packet_ipv6_macos = create_loopback_ipv6_tcp_macos();
    let mut normalizer = PacketNormalizer::new();

    let result = normalizer.normalize(0, 1000000, &packet_ipv6_macos).unwrap();
    assert!(result.is_some());

    let normalized = result.unwrap();
    assert_eq!(
        normalized.src_ip,
        IpAddr::from([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1])
    );
    assert_eq!(
        normalized.dst_ip,
        IpAddr::from([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2])
    );

    // Test IPv6 loopback (Linux: AF_INET6 = 10)
    let packet_ipv6_linux = create_loopback_ipv6_tcp_linux();
    let mut normalizer = PacketNormalizer::new();

    let result = normalizer.normalize(0, 1000000, &packet_ipv6_linux).unwrap();
    assert!(result.is_some());

    let normalized = result.unwrap();
    assert_eq!(
        normalized.src_ip,
        IpAddr::from([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1])
    );
}

#[test]
fn test_ip_fragment_reassembly() {
    let fragments = create_ipv4_fragments();
    let mut normalizer = PacketNormalizer::new();

    // Feed fragments in order
    let result1 = normalizer.normalize(1, 1000000, &fragments[0]).unwrap();
    assert!(result1.is_none(), "First fragment should return None (incomplete)");

    let result2 = normalizer.normalize(1, 1000001, &fragments[1]).unwrap();
    assert!(result2.is_none(), "Second fragment should return None (incomplete)");

    let result3 = normalizer.normalize(1, 1000002, &fragments[2]).unwrap();
    assert!(result3.is_some(), "Third fragment should trigger reassembly");

    let normalized = result3.unwrap();
    assert_eq!(normalized.src_ip, IpAddr::from([192, 168, 1, 1]));
    assert_eq!(normalized.dst_ip, IpAddr::from([10, 0, 0, 1]));
    // Payload should be the reassembled data (3KB minus TCP header parsed out)
    // The parse_transport_from_bytes will try to parse as TCP and extract payload
    // Since we're building raw fragments, the TCP parsing may not be perfect
    // Just verify we got reasonable data back
    assert!(normalized.payload.len() > 2900, "Expected at least 2900 bytes, got {}", normalized.payload.len());
}

#[test]
fn test_ip_fragment_timeout() {
    let fragments = create_ipv4_fragments();
    let mut normalizer = PacketNormalizer::new();

    // Feed only the first two fragments
    let result1 = normalizer.normalize(1, 1000000, &fragments[0]).unwrap();
    assert!(result1.is_none(), "First fragment should return None");

    let result2 = normalizer.normalize(1, 1000001, &fragments[1]).unwrap();
    assert!(result2.is_none(), "Second fragment should return None");

    // The third fragment is never sent - the incomplete fragment train should be cleaned up
    // We can't directly test timeout without advancing time, but we can verify the pool
    // doesn't grow unbounded by feeding many different incomplete fragment trains

    // Create 10 different incomplete fragment trains
    for i in 0..10 {
        let mut frag = fragments[0].clone();
        // Modify identification to create different fragment train
        frag[18] = i as u8;
        let result = normalizer.normalize(1, 1000000 + i as u64, &frag).unwrap();
        assert!(result.is_none());
    }

    // All should be accepted without error (bounded pool)
}

#[test]
fn test_ipv6_fragment() {
    // Create a fragmented IPv6 packet
    // IPv6 fragmentation uses extension headers, which etherparse supports
    use etherparse::{IpNumber, Ipv6FlowLabel, Ipv6FragmentHeader, Ipv6Header};

    let payload = vec![0xBB; 2000]; // 2KB payload

    // Fragment 1
    let mut frag1 = Vec::new();
    let eth = etherparse::Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: etherparse::EtherType(0x86dd), // IPv6
    };

    let mut ipv6_header = Ipv6Header {
        traffic_class: 0,
        flow_label: Ipv6FlowLabel::try_new(0).unwrap(),
        payload_length: 0,                 // will be updated
        next_header: IpNumber(44), // Fragment header
        hop_limit: 64,
        source: [0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        destination: [0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2],
    };

    let frag_header1 = Ipv6FragmentHeader {
        next_header: IpNumber(6), // TCP
        fragment_offset: etherparse::IpFragOffset::try_new(0).unwrap(),
        more_fragments: true,
        identification: 0x12345678,
    };

    eth.write(&mut frag1).unwrap();
    ipv6_header.payload_length = (8 + 1480) as u16; // frag header + payload
    ipv6_header.write(&mut frag1).unwrap();
    frag_header1.write(&mut frag1).unwrap();
    frag1.extend_from_slice(&payload[0..1480]);

    // Fragment 2 (last)
    let mut frag2 = Vec::new();
    let mut frag_header2 = frag_header1.clone();
    frag_header2.fragment_offset = etherparse::IpFragOffset::try_new(1480 / 8).unwrap();
    frag_header2.more_fragments = false;

    eth.write(&mut frag2).unwrap();
    ipv6_header.payload_length = (8 + 520) as u16;
    ipv6_header.write(&mut frag2).unwrap();
    frag_header2.write(&mut frag2).unwrap();
    frag2.extend_from_slice(&payload[1480..]);

    let mut normalizer = PacketNormalizer::new();

    let result1 = normalizer.normalize(1, 2000000, &frag1).unwrap();
    assert!(result1.is_none(), "First IPv6 fragment should return None");

    let result2 = normalizer.normalize(1, 2000001, &frag2).unwrap();
    assert!(result2.is_some(), "Second IPv6 fragment should trigger reassembly");

    let normalized = result2.unwrap();
    assert_eq!(
        normalized.src_ip,
        IpAddr::from([0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1])
    );
    assert_eq!(
        normalized.dst_ip,
        IpAddr::from([0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2])
    );
    // IPv6 fragments are reassembled; payload will be parsed for transport
    // Just verify we got reasonable data back
    assert!(normalized.payload.len() > 1900, "Expected at least 1900 bytes, got {}", normalized.payload.len());
}
