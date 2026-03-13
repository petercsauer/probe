//! Link-layer edge case tests: SLL2, Loopback, VLAN, truncated frames.

use prb_pcap::normalize_stateless;

#[test]
fn test_sll2_truncated_header() {
    // SLL2 header is 20 bytes, but provide only 10 bytes
    let truncated = vec![
        0x08, 0x00, // protocol type (IPv4)
        0x00, 0x00, // reserved
        0x00, 0x00, 0x00, 0x01, // interface index
        0x00,
        0x01, // arphrd_type
              // Missing: packet_type, link_layer_addr_len, link_layer_addr
    ];

    // Should return error, not panic
    let result = normalize_stateless(276, 1000000, &truncated);
    assert!(
        result.is_err(),
        "SLL2 with truncated header should return error"
    );
}

#[test]
fn test_sll2_valid_minimal() {
    // Valid 20-byte SLL2 header followed by IPv4 packet
    let mut packet = Vec::new();

    // SLL2 header
    packet.extend_from_slice(&[0x08, 0x00]); // protocol type (IPv4)
    packet.extend_from_slice(&[0x00, 0x00]); // reserved
    packet.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // interface index
    packet.extend_from_slice(&[0x00, 0x01]); // arphrd_type
    packet.push(0x00); // packet_type
    packet.push(0x06); // link_layer_addr_len (6 bytes for MAC)
    packet.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x00]); // link_layer_addr

    // Minimal IPv4 + UDP packet
    use etherparse::PacketBuilder;
    let builder = PacketBuilder::ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64).udp(12345, 80);

    let mut ip_packet = Vec::new();
    builder.write(&mut ip_packet, b"test").unwrap();
    packet.extend_from_slice(&ip_packet);

    let result = normalize_stateless(276, 1000000, &packet);
    assert!(result.is_ok(), "Valid SLL2 packet should parse");
}

#[test]
fn test_loopback_invalid_af_family() {
    // Loopback header with unsupported AF family
    let mut packet = Vec::new();

    // AF family 99 (not AF_INET or AF_INET6)
    packet.extend_from_slice(&99u32.to_le_bytes());

    // IPv4 packet payload
    use etherparse::PacketBuilder;
    let builder = PacketBuilder::ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64).udp(12345, 80);
    builder.write(&mut packet, b"test").unwrap();

    // Should return error for unsupported AF family
    let result = normalize_stateless(0, 1000000, &packet);
    assert!(
        result.is_err(),
        "Loopback with invalid AF family should return error"
    );
}

#[test]
fn test_loopback_valid_af_inet6_linux() {
    // Loopback with AF_INET6 on Linux (value 10)
    let mut packet = Vec::new();

    // AF_INET6 = 10 on Linux
    packet.extend_from_slice(&10u32.to_le_bytes());

    // IPv6 packet
    use etherparse::PacketBuilder;
    let src = [
        0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01,
    ];
    let dst = [
        0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02,
    ];

    let builder = PacketBuilder::ipv6(src, dst, 64).udp(12345, 80);
    builder.write(&mut packet, b"test").unwrap();

    let result = normalize_stateless(0, 1000000, &packet);
    assert!(
        result.is_ok(),
        "Loopback with AF_INET6 (Linux) should parse"
    );
}

#[test]
fn test_loopback_valid_af_inet6_macos() {
    // Loopback with AF_INET6 on macOS/BSD (value 30)
    let mut packet = Vec::new();

    // AF_INET6 = 30 on macOS/BSD
    packet.extend_from_slice(&30u32.to_le_bytes());

    // IPv6 packet
    use etherparse::PacketBuilder;
    let src = [
        0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01,
    ];
    let dst = [
        0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02,
    ];

    let builder = PacketBuilder::ipv6(src, dst, 64).udp(12345, 80);
    builder.write(&mut packet, b"test").unwrap();

    let result = normalize_stateless(0, 1000000, &packet);
    assert!(
        result.is_ok(),
        "Loopback with AF_INET6 (macOS/BSD) should parse"
    );
}

#[test]
fn test_vlan_max_depth() {
    // Test packet with 4 nested VLAN tags (0x8100 repeated)
    // etherparse should handle nested VLANs
    use etherparse::{PacketBuilder, VlanId};

    // Create packet with double VLAN (etherparse supports up to double VLAN)
    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .double_vlan(VlanId::try_new(100).unwrap(), VlanId::try_new(200).unwrap())
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .udp(12345, 80);

    let mut packet = Vec::new();
    builder.write(&mut packet, b"VLAN test").unwrap();

    let result = normalize_stateless(1, 1000000, &packet);
    assert!(result.is_ok(), "Double VLAN packet should parse");

    // Check that first VLAN ID is extracted
    if let Ok(prb_pcap::NormalizeResult::Packet(pkt)) = result {
        assert_eq!(pkt.vlan_id, Some(100), "First VLAN ID should be extracted");
    }
}

#[test]
fn test_truncated_ethernet_frame() {
    // Ethernet frame that claims to have payload but is truncated
    let mut packet = Vec::new();

    // Ethernet header (14 bytes)
    packet.extend_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]); // src MAC
    packet.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]); // dst MAC
    packet.extend_from_slice(&[0x08, 0x00]); // EtherType (IPv4)

    // Truncated IPv4 header (only 5 bytes instead of minimum 20)
    packet.extend_from_slice(&[0x45, 0x00, 0x00, 0x1c, 0x00]);

    let result = normalize_stateless(1, 1000000, &packet);
    assert!(
        result.is_err(),
        "Truncated Ethernet frame should return error"
    );
}

#[test]
fn test_zero_length_ethernet_frame() {
    // Ethernet header with no payload
    let mut packet = Vec::new();

    packet.extend_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]); // src MAC
    packet.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]); // dst MAC
    packet.extend_from_slice(&[0x08, 0x00]); // EtherType (IPv4)

    // No payload at all

    let result = normalize_stateless(1, 1000000, &packet);
    assert!(
        result.is_err(),
        "Zero-length Ethernet frame should return error"
    );
}

#[test]
fn test_sll2_unsupported_protocol() {
    // SLL2 with unsupported protocol type (not IPv4 or IPv6)
    let mut packet = Vec::new();

    // SLL2 header with ARP protocol (0x0806)
    packet.extend_from_slice(&[0x08, 0x06]); // protocol type (ARP)
    packet.extend_from_slice(&[0x00, 0x00]); // reserved
    packet.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // interface index
    packet.extend_from_slice(&[0x00, 0x01]); // arphrd_type
    packet.push(0x00); // packet_type
    packet.push(0x06); // link_layer_addr_len
    packet.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x00]); // link_layer_addr

    // ARP payload
    packet.extend_from_slice(&[0xaa; 28]);

    let result = normalize_stateless(276, 1000000, &packet);
    assert!(
        result.is_err(),
        "SLL2 with unsupported protocol should return error"
    );
}
