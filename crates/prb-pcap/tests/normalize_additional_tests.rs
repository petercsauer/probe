//! Additional tests for normalize module coverage.

use prb_pcap::normalize_stateless;

#[test]
fn test_normalize_other_protocol() {
    // Test Other transport protocol variant (e.g., IGMP, protocol 2)
    use etherparse::{EtherType, Ethernet2Header, IpNumber, Ipv4Header};

    let mut packet = Vec::new();

    let eth = Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: EtherType(0x0800),
    };
    eth.write(&mut packet).unwrap();

    // IGMP packet (protocol 2) - need at least 8 bytes payload
    let igmp_payload = [0x11, 0x00, 0xEE, 0x9B, 0xE0, 0x00, 0x00, 0x01]; // IGMP membership query
    let ipv4 = Ipv4Header::new(
        igmp_payload.len() as u16,
        64,
        IpNumber(2),
        [192, 168, 1, 1],
        [224, 0, 0, 1],
    )
    .unwrap();
    ipv4.write(&mut packet).unwrap();

    // IGMP payload
    packet.extend_from_slice(&igmp_payload);

    let result = normalize_stateless(1, 1000000, &packet).unwrap();

    if let prb_pcap::NormalizeResult::Packet(pkt) = result {
        // Should be classified as Other transport
        match pkt.transport {
            prb_pcap::TransportInfo::Other(proto) => {
                assert_eq!(proto, 2, "Should be protocol 2 (IGMP)");
            }
            _ => panic!("Expected Other transport type"),
        }
    } else {
        panic!("Expected Packet result");
    }
}

#[test]
fn test_tcp_flags_from_byte() {
    // Test TcpFlags::from_byte for various flag combinations
    use prb_pcap::TcpFlags;

    // No flags set
    let flags = TcpFlags::from_byte(0x00);
    assert!(!flags.fin);
    assert!(!flags.syn);
    assert!(!flags.rst);
    assert!(!flags.psh);
    assert!(!flags.ack);

    // SYN flag
    let flags = TcpFlags::from_byte(0x02);
    assert!(!flags.fin);
    assert!(flags.syn);
    assert!(!flags.rst);
    assert!(!flags.psh);
    assert!(!flags.ack);

    // SYN+ACK
    let flags = TcpFlags::from_byte(0x12);
    assert!(!flags.fin);
    assert!(flags.syn);
    assert!(!flags.rst);
    assert!(!flags.psh);
    assert!(flags.ack);

    // FIN+ACK
    let flags = TcpFlags::from_byte(0x11);
    assert!(flags.fin);
    assert!(!flags.syn);
    assert!(!flags.rst);
    assert!(!flags.psh);
    assert!(flags.ack);

    // RST
    let flags = TcpFlags::from_byte(0x04);
    assert!(!flags.fin);
    assert!(!flags.syn);
    assert!(flags.rst);
    assert!(!flags.psh);
    assert!(!flags.ack);

    // PSH+ACK
    let flags = TcpFlags::from_byte(0x18);
    assert!(!flags.fin);
    assert!(!flags.syn);
    assert!(!flags.rst);
    assert!(flags.psh);
    assert!(flags.ack);

    // All flags
    let flags = TcpFlags::from_byte(0x1F);
    assert!(flags.fin);
    assert!(flags.syn);
    assert!(flags.rst);
    assert!(flags.psh);
    assert!(flags.ack);
}

#[test]
fn test_owned_normalized_packet_conversion() {
    // Test OwnedNormalizedPacket conversions
    use prb_pcap::{
        NormalizedPacket, OwnedNormalizedPacket, TcpFlags, TcpSegmentInfo, TransportInfo,
    };
    use std::net::IpAddr;

    let borrowed = NormalizedPacket {
        timestamp_us: 1234567890,
        src_ip: IpAddr::from([192, 168, 1, 1]),
        dst_ip: IpAddr::from([10, 0, 0, 1]),
        transport: TransportInfo::Tcp(TcpSegmentInfo {
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
        }),
        vlan_id: Some(100),
        payload: b"test payload",
    };

    // Test from_borrowed
    let owned = OwnedNormalizedPacket::from_borrowed(&borrowed);
    assert_eq!(owned.timestamp_us, 1234567890);
    assert_eq!(owned.src_ip, IpAddr::from([192, 168, 1, 1]));
    assert_eq!(owned.dst_ip, IpAddr::from([10, 0, 0, 1]));
    assert_eq!(owned.vlan_id, Some(100));
    assert_eq!(owned.payload, b"test payload");

    // Test from_normalized (alias)
    let owned2 = OwnedNormalizedPacket::from_normalized(&borrowed);
    assert_eq!(owned2.timestamp_us, owned.timestamp_us);
    assert_eq!(owned2.payload, owned.payload);

    // Test as_normalized
    let borrowed2 = owned.as_normalized();
    assert_eq!(borrowed2.timestamp_us, 1234567890);
    assert_eq!(borrowed2.payload, b"test payload");
}

#[test]
fn test_udp_transport_info() {
    // Test UDP TransportInfo variant
    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .udp(5353, 5353); // mDNS

    let mut packet = Vec::new();
    builder.write(&mut packet, b"mDNS query").unwrap();

    let result = normalize_stateless(1, 1000000, &packet).unwrap();

    if let prb_pcap::NormalizeResult::Packet(pkt) = result {
        match pkt.transport {
            prb_pcap::TransportInfo::Udp { src_port, dst_port } => {
                assert_eq!(src_port, 5353);
                assert_eq!(dst_port, 5353);
            }
            _ => panic!("Expected UDP transport"),
        }
        assert_eq!(pkt.payload, b"mDNS query");
    } else {
        panic!("Expected Packet result");
    }
}
