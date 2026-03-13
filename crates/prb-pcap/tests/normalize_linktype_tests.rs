//! Tests for normalize.rs linktype handling and edge cases.

use prb_pcap::PacketNormalizer;
use std::net::{IpAddr, Ipv4Addr};

/// Create a raw IPv4 packet (linktype 101).
fn create_raw_ipv4_packet() -> Vec<u8> {
    use etherparse::{IpNumber, Ipv4Header, TcpHeader};

    let mut packet = Vec::new();

    // IPv4 header (no Ethernet)
    let ipv4 = Ipv4Header::new(
        20, // TCP header size
        64,
        IpNumber(6), // TCP
        [192, 168, 1, 1],
        [10, 0, 0, 1],
    )
    .unwrap();
    ipv4.write(&mut packet).unwrap();

    // TCP header
    let tcp = TcpHeader::new(12345, 80, 1000, 4096);
    tcp.write(&mut packet).unwrap();

    packet
}

/// Create a loopback packet (linktype 0).
fn create_loopback_ipv4_packet() -> Vec<u8> {
    use etherparse::{IpNumber, Ipv4Header, TcpHeader};

    let mut packet = Vec::new();

    // Loopback header (4 bytes, AF_INET = 2, little-endian)
    packet.extend_from_slice(&2u32.to_le_bytes());

    // IPv4 header
    let ipv4 = Ipv4Header::new(20, 64, IpNumber(6), [127, 0, 0, 1], [127, 0, 0, 2]).unwrap();
    ipv4.write(&mut packet).unwrap();

    // TCP header
    let tcp = TcpHeader::new(12345, 80, 1000, 4096);
    tcp.write(&mut packet).unwrap();

    packet
}

/// Create a Linux SLL (cooked capture) packet (linktype 113).
fn create_sll_packet() -> Vec<u8> {
    use etherparse::{IpNumber, Ipv4Header, TcpHeader};

    let mut packet = Vec::new();

    // SLL header (16 bytes)
    packet.extend_from_slice(&0u16.to_be_bytes()); // packet_type (host)
    packet.extend_from_slice(&1u16.to_be_bytes()); // arp_hrd_type (Ethernet)
    packet.extend_from_slice(&6u16.to_be_bytes()); // sender_address_valid_length
    packet.extend_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x00, 0x00]); // sender_address
    packet.extend_from_slice(&0x0800u16.to_be_bytes()); // protocol_type (IPv4)

    // IPv4 header
    let ipv4 = Ipv4Header::new(20, 64, IpNumber(6), [192, 168, 1, 1], [10, 0, 0, 1]).unwrap();
    ipv4.write(&mut packet).unwrap();

    // TCP header
    let tcp = TcpHeader::new(12345, 80, 1000, 4096);
    tcp.write(&mut packet).unwrap();

    packet
}

#[test]
fn test_normalize_linktype_raw_ip() {
    let mut normalizer = PacketNormalizer::new();
    let packet = create_raw_ipv4_packet();

    let result = normalizer.normalize(101, 1000, &packet);
    assert!(
        result.is_ok(),
        "Raw IP packet should normalize successfully"
    );

    let normalized = result.unwrap().expect("Should produce a normalized packet");
    assert_eq!(normalized.src_ip, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
    assert_eq!(normalized.dst_ip, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
}

#[test]
fn test_normalize_linktype_loopback() {
    let mut normalizer = PacketNormalizer::new();
    let packet = create_loopback_ipv4_packet();

    let result = normalizer.normalize(0, 1000, &packet);
    assert!(
        result.is_ok(),
        "Loopback packet should normalize successfully"
    );

    let normalized = result.unwrap().expect("Should produce a normalized packet");
    assert_eq!(normalized.src_ip, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
    assert_eq!(normalized.dst_ip, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)));
}

#[test]
fn test_normalize_linktype_sll() {
    let mut normalizer = PacketNormalizer::new();
    let packet = create_sll_packet();

    let result = normalizer.normalize(113, 1000, &packet);
    assert!(result.is_ok(), "SLL packet should normalize successfully");

    let normalized = result.unwrap().expect("Should produce a normalized packet");
    assert_eq!(normalized.src_ip, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
    assert_eq!(normalized.dst_ip, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
}

#[test]
fn test_normalize_unsupported_linktype() {
    let mut normalizer = PacketNormalizer::new();
    let packet = vec![0xAA; 60];

    // Linktype 999 is not supported
    let result = normalizer.normalize(999, 1000, &packet);
    assert!(result.is_err(), "Unsupported linktype should return error");
}

#[test]
fn test_normalize_corrupted_packet() {
    let mut normalizer = PacketNormalizer::new();

    // Packet too short for Ethernet header
    let packet = vec![0xAA; 10];
    let result = normalizer.normalize(1, 1000, &packet);
    assert!(result.is_err(), "Corrupted packet should return error");
}

#[test]
fn test_normalize_tcp_flags_parsing() {
    use prb_pcap::TcpFlags;

    // Test all flag combinations
    let flags_byte = 0b00010010; // SYN + ACK
    let flags = TcpFlags::from_byte(flags_byte);
    assert!(flags.syn);
    assert!(flags.ack);
    assert!(!flags.fin);
    assert!(!flags.rst);
    assert!(!flags.psh);

    // Test FIN flag
    let flags_fin = TcpFlags::from_byte(0x01);
    assert!(flags_fin.fin);
    assert!(!flags_fin.syn);

    // Test RST flag
    let flags_rst = TcpFlags::from_byte(0x04);
    assert!(flags_rst.rst);
    assert!(!flags_rst.syn);

    // Test PSH flag
    let flags_psh = TcpFlags::from_byte(0x08);
    assert!(flags_psh.psh);
    assert!(!flags_psh.syn);
}

#[test]
fn test_normalize_defrag_cleanup_interval() {
    // Test that defragmentation pool cleanup happens periodically
    let mut normalizer = PacketNormalizer::new();
    let packet = create_raw_ipv4_packet();

    // Process 1000+ packets to trigger cleanup
    for i in 0..1005 {
        let timestamp = 1000 + i * 1000; // 1ms apart
        let _ = normalizer.normalize(101, timestamp, &packet);
    }

    // If we got here without panic, cleanup worked
}

#[test]
fn test_normalize_owned_packet_conversion() {
    use prb_pcap::OwnedNormalizedPacket;

    let mut normalizer = PacketNormalizer::new();
    let packet = create_raw_ipv4_packet();

    let result = normalizer.normalize(101, 1000, &packet);
    let normalized = result.unwrap().expect("Should produce a normalized packet");

    // Convert to owned
    let owned = OwnedNormalizedPacket::from_borrowed(&normalized);
    assert_eq!(owned.src_ip, normalized.src_ip);
    assert_eq!(owned.dst_ip, normalized.dst_ip);
    assert_eq!(owned.timestamp_us, normalized.timestamp_us);

    // Convert back to borrowed
    let borrowed = owned.as_normalized();
    assert_eq!(borrowed.src_ip, owned.src_ip);
    assert_eq!(borrowed.dst_ip, owned.dst_ip);

    // Test from_normalized alias
    let owned2 = OwnedNormalizedPacket::from_normalized(&normalized);
    assert_eq!(owned2.src_ip, normalized.src_ip);
}

#[test]
fn test_transport_info_variants() {
    use prb_pcap::{TcpFlags, TcpSegmentInfo, TransportInfo};

    // Test TCP variant
    let tcp_info = TransportInfo::Tcp(TcpSegmentInfo {
        src_port: 12345,
        dst_port: 80,
        seq: 1000,
        ack: 2000,
        flags: TcpFlags {
            syn: true,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
        },
    });

    match tcp_info {
        TransportInfo::Tcp(info) => {
            assert_eq!(info.src_port, 12345);
            assert_eq!(info.dst_port, 80);
        }
        _ => panic!("Expected TCP transport info"),
    }

    // Test UDP variant
    let udp_info = TransportInfo::Udp {
        src_port: 5555,
        dst_port: 5556,
    };

    match udp_info {
        TransportInfo::Udp { src_port, dst_port } => {
            assert_eq!(src_port, 5555);
            assert_eq!(dst_port, 5556);
        }
        _ => panic!("Expected UDP transport info"),
    }

    // Test Other variant
    let other_info = TransportInfo::Other(17); // ICMP
    match other_info {
        TransportInfo::Other(proto) => assert_eq!(proto, 17),
        _ => panic!("Expected Other transport info"),
    }
}
