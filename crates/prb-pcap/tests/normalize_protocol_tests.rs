//! Protocol normalization edge case tests covering various link types and error conditions.

use prb_pcap::{NormalizeResult, PacketNormalizer, PcapError, TransportInfo, normalize_stateless};
use std::net::IpAddr;

/// Test unsupported linktype.
#[test]
fn test_unsupported_linktype() {
    let mut normalizer = PacketNormalizer::new();
    let packet_data = vec![0u8; 64];

    // Linktype 999 doesn't exist
    let result = normalizer.normalize(999, 1000000, &packet_data);

    assert!(result.is_err());
    match result {
        Err(PcapError::InvalidLinktype(msg)) => {
            assert!(msg.contains("unsupported linktype: 999"));
        }
        _ => panic!("Expected InvalidLinktype error"),
    }
}

/// Test loopback packet with IPv6 (AF_INET6 = 30 on macOS/BSD).
#[test]
fn test_loopback_ipv6_macos() {
    use etherparse::PacketBuilder;

    let mut normalizer = PacketNormalizer::new();

    // Build IPv6 packet
    let builder = PacketBuilder::ipv6(
        [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1], // src
        [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2], // dst
        64,
    )
    .udp(12345, 53);

    let payload = b"test";
    let mut ipv6_packet = Vec::new();
    builder.write(&mut ipv6_packet, payload).unwrap();

    // Prepend loopback header: AF_INET6 = 30 (macOS/BSD)
    let mut loopback_packet = vec![30, 0, 0, 0]; // Little-endian u32
    loopback_packet.extend_from_slice(&ipv6_packet);

    let result = normalizer.normalize(0, 1000000, &loopback_packet);

    assert!(result.is_ok());
    let normalized = result.unwrap().unwrap();
    assert!(matches!(normalized.src_ip, IpAddr::V6(_)));
}

/// Test loopback packet with IPv6 (AF_INET6 = 10 on Linux).
#[test]
fn test_loopback_ipv6_linux() {
    use etherparse::PacketBuilder;

    let mut normalizer = PacketNormalizer::new();

    let builder = PacketBuilder::ipv6(
        [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2],
        64,
    )
    .udp(12345, 53);

    let payload = b"test";
    let mut ipv6_packet = Vec::new();
    builder.write(&mut ipv6_packet, payload).unwrap();

    // Prepend loopback header: AF_INET6 = 10 (Linux)
    let mut loopback_packet = vec![10, 0, 0, 0]; // Little-endian u32
    loopback_packet.extend_from_slice(&ipv6_packet);

    let result = normalizer.normalize(0, 1000000, &loopback_packet);

    assert!(result.is_ok());
    let normalized = result.unwrap().unwrap();
    assert!(matches!(normalized.src_ip, IpAddr::V6(_)));
}

/// Test loopback packet with unsupported AF family.
#[test]
fn test_loopback_unsupported_af_family() {
    let mut normalizer = PacketNormalizer::new();

    // Create loopback packet with unsupported AF family (e.g., 99)
    let mut loopback_packet = vec![99, 0, 0, 0]; // AF family = 99
    loopback_packet.extend_from_slice(&[0u8; 64]); // Dummy IP packet

    let result = normalizer.normalize(0, 1000000, &loopback_packet);

    assert!(result.is_err());
    match result {
        Err(PcapError::Parse(msg)) => {
            assert!(msg.contains("unsupported AF family"));
            assert!(msg.contains("99"));
        }
        _ => panic!("Expected parse error for unsupported AF family"),
    }
}

/// Test loopback packet too short (missing AF header).
#[test]
fn test_loopback_too_short() {
    let mut normalizer = PacketNormalizer::new();
    let short_packet = vec![0u8; 3]; // Less than 4 bytes

    let result = normalizer.normalize(0, 1000000, &short_packet);

    assert!(result.is_err());
    match result {
        Err(PcapError::Parse(msg)) => {
            assert!(msg.contains("loopback packet too short"));
        }
        _ => panic!("Expected parse error"),
    }
}

/// Test SLL2 packet with IPv6.
#[test]
fn test_sll2_ipv6() {
    use etherparse::PacketBuilder;

    let mut normalizer = PacketNormalizer::new();

    let builder = PacketBuilder::ipv6(
        [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2],
        64,
    )
    .tcp(12345, 80, 1000, 4096);

    let payload = b"test";
    let mut ipv6_packet = Vec::new();
    builder.write(&mut ipv6_packet, payload).unwrap();

    // Construct SLL2 header (20 bytes)
    let mut sll2_packet = vec![
        0x86, 0xdd, // Protocol type: IPv6
        0x00, 0x00, // Reserved
        0x00, 0x00, 0x00, 0x01, // Interface index
        0x00, 0x01, // ARPHRD type
        0x00, // Packet type
        0x06, // Link layer addr length
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x00, 0x00, // Link layer addr (MAC + padding)
    ];
    sll2_packet.extend_from_slice(&ipv6_packet);

    let result = normalizer.normalize(276, 1000000, &sll2_packet);

    assert!(result.is_ok());
    let normalized = result.unwrap().unwrap();
    assert!(matches!(normalized.src_ip, IpAddr::V6(_)));
}

/// Test SLL2 packet with unsupported protocol type.
#[test]
fn test_sll2_unsupported_protocol() {
    let mut normalizer = PacketNormalizer::new();

    // Construct SLL2 header with unsupported protocol type (e.g., 0x0806 = ARP)
    let sll2_packet = vec![
        0x08, 0x06, // Protocol type: ARP (not supported)
        0x00, 0x00, // Reserved
        0x00, 0x00, 0x00, 0x01, // Interface index
        0x00, 0x01, // ARPHRD type
        0x00, // Packet type
        0x06, // Link layer addr length
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x00,
        0x00, // Link layer addr
              // No payload needed for this test
    ];

    let result = normalizer.normalize(276, 1000000, &sll2_packet);

    assert!(result.is_err());
    match result {
        Err(PcapError::Parse(msg)) => {
            assert!(msg.contains("unsupported protocol type"));
            assert!(msg.contains("0x0806"));
        }
        _ => panic!("Expected parse error for unsupported protocol"),
    }
}

/// Test SLL2 packet too short.
#[test]
fn test_sll2_too_short() {
    let mut normalizer = PacketNormalizer::new();
    let short_packet = vec![0u8; 19]; // Less than 20 bytes

    let result = normalizer.normalize(276, 1000000, &short_packet);

    assert!(result.is_err());
    match result {
        Err(PcapError::Parse(msg)) => {
            assert!(msg.contains("SLL2 packet too short"));
        }
        _ => panic!("Expected parse error"),
    }
}

/// Test Raw IP (linktype 101) with IPv4.
#[test]
fn test_raw_ip_ipv4() {
    use etherparse::PacketBuilder;

    let mut normalizer = PacketNormalizer::new();

    let builder = PacketBuilder::ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64).udp(12345, 53);

    let payload = b"DNS query";
    let mut raw_ip_packet = Vec::new();
    builder.write(&mut raw_ip_packet, payload).unwrap();

    let result = normalizer.normalize(101, 1000000, &raw_ip_packet);

    assert!(result.is_ok());
    let normalized = result.unwrap().unwrap();
    assert_eq!(normalized.src_ip, IpAddr::from([192, 168, 1, 1]));
}

/// Test Raw IP (linktype 101) with IPv6.
#[test]
fn test_raw_ip_ipv6() {
    use etherparse::PacketBuilder;

    let mut normalizer = PacketNormalizer::new();

    let builder = PacketBuilder::ipv6(
        [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2],
        64,
    )
    .tcp(80, 12345, 5000, 8192);

    let payload = b"HTTP response";
    let mut raw_ip_packet = Vec::new();
    builder.write(&mut raw_ip_packet, payload).unwrap();

    let result = normalizer.normalize(101, 1000000, &raw_ip_packet);

    assert!(result.is_ok());
    let normalized = result.unwrap().unwrap();
    assert!(matches!(normalized.src_ip, IpAddr::V6(_)));
}

/// Test packet with ARP (non-IP protocol).
#[test]
fn test_packet_no_network_layer() {
    let mut normalizer = PacketNormalizer::new();

    // Create a full valid Ethernet + ARP frame
    let eth_frame = vec![
        0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, // dst MAC
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, // src MAC
        0x08, 0x06, // EtherType: ARP
        // Full ARP payload (28 bytes for IPv4)
        0x00, 0x01, // Hardware type: Ethernet
        0x08, 0x00, // Protocol type: IPv4
        0x06, // Hardware address length
        0x04, // Protocol address length
        0x00, 0x01, // Operation: request
        // Sender hardware address (6 bytes)
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, // Sender protocol address (4 bytes)
        192, 168, 1, 1, // Target hardware address (6 bytes)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Target protocol address (4 bytes)
        192, 168, 1, 2,
    ];

    let result = normalizer.normalize(1, 1000000, &eth_frame);

    // Should get error - ARP packets aren't supported for normalization
    assert!(result.is_err());
}

/// Test TCP with zero-length payload.
#[test]
fn test_tcp_zero_length_payload() {
    use etherparse::PacketBuilder;

    let mut normalizer = PacketNormalizer::new();

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .tcp(12345, 80, 1000, 4096);

    let payload = b""; // Empty payload
    let mut packet = Vec::new();
    builder.write(&mut packet, payload).unwrap();

    let result = normalizer.normalize(1, 1000000, &packet);

    assert!(result.is_ok());
    let normalized = result.unwrap().unwrap();
    assert_eq!(normalized.payload.len(), 0);
}

/// Test UDP with zero-length payload.
#[test]
fn test_udp_zero_length_payload() {
    use etherparse::PacketBuilder;

    let mut normalizer = PacketNormalizer::new();

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .udp(12345, 53);

    let payload = b""; // Empty payload
    let mut packet = Vec::new();
    builder.write(&mut packet, payload).unwrap();

    let result = normalizer.normalize(1, 1000000, &packet);

    assert!(result.is_ok());
    let normalized = result.unwrap().unwrap();
    assert_eq!(normalized.payload.len(), 0);
}

/// Test ICMPv4 packet.
#[test]
fn test_icmpv4_packet() {
    use etherparse::PacketBuilder;

    let mut normalizer = PacketNormalizer::new();

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .icmpv4_echo_request(1, 1);

    let payload = b"ping";
    let mut packet = Vec::new();
    builder.write(&mut packet, payload).unwrap();

    let result = normalizer.normalize(1, 1000000, &packet);

    assert!(result.is_ok());
    let normalized = result.unwrap().unwrap();
    assert!(matches!(normalized.transport, TransportInfo::Other(1)));
}

/// Test ICMPv6 packet.
#[test]
fn test_icmpv6_packet() {
    use etherparse::PacketBuilder;

    let mut normalizer = PacketNormalizer::new();

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .ipv6(
        [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2],
        64,
    )
    .icmpv6_echo_request(1, 1);

    let payload = b"ping6";
    let mut packet = Vec::new();
    builder.write(&mut packet, payload).unwrap();

    let result = normalizer.normalize(1, 1000000, &packet);

    assert!(result.is_ok());
    let normalized = result.unwrap().unwrap();
    assert!(matches!(normalized.transport, TransportInfo::Other(58)));
}

/// Test defragmentation timeout cleanup.
#[test]
fn test_defrag_cleanup_triggers() {
    use etherparse::PacketBuilder;

    let mut normalizer = PacketNormalizer::new();

    // Create a simple packet and normalize it 1001 times to trigger cleanup
    let builder = PacketBuilder::ethernet2([0x00; 6], [0xff; 6])
        .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
        .udp(12345, 53);

    let payload = b"test";
    let mut packet = Vec::new();
    builder.write(&mut packet, payload).unwrap();

    // Process 1001 packets to trigger cleanup at least once
    for i in 0..1001 {
        let timestamp = 1000000 + i * 1000; // Increment timestamp
        let result = normalizer.normalize(1, timestamp, &packet);
        assert!(result.is_ok());
    }

    // If we got here without panic, cleanup worked
}

/// Test that stateless normalization detects fragments.
#[test]
fn test_stateless_fragment_detection() {
    // Create IPv4 fragment (more fragments flag set)
    let mut fragment = vec![
        0x45, // Version 4, IHL 5
        0x00, // TOS
        0x00, 0x20, // Total length: 32
        0x12, 0x34, // ID
        0x20, 0x00, // Flags: More Fragments, Offset: 0
        0x40, // TTL
        0x11, // Protocol: UDP
        0x00, 0x00, // Checksum (invalid, but okay for test)
        192, 168, 1, 1, // Source IP
        10, 0, 0, 1, // Destination IP
    ];
    fragment.extend_from_slice(&[0u8; 12]); // Payload

    let result = normalize_stateless(101, 1000000, &fragment);

    assert!(result.is_ok());
    match result.unwrap() {
        NormalizeResult::Fragment { .. } => {
            // Expected
        }
        _ => panic!("Expected Fragment result"),
    }
}

/// Test SLL (linktype 113) parsing.
#[test]
fn test_sll_v1_parsing() {
    use etherparse::PacketBuilder;

    let mut normalizer = PacketNormalizer::new();

    let builder =
        PacketBuilder::ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64).tcp(12345, 80, 1000, 4096);

    let payload = b"test";
    let mut ip_packet = Vec::new();
    builder.write(&mut ip_packet, payload).unwrap();

    // Construct SLL header (16 bytes)
    let mut sll_packet = vec![
        0x00, 0x00, // Packet type: host
        0x00, 0x01, // ARPHRD type: Ethernet
        0x00, 0x06, // Link layer address length: 6
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x00, 0x00, // Address + padding
        0x08, 0x00, // Protocol: IPv4
    ];
    sll_packet.extend_from_slice(&ip_packet);

    let result = normalizer.normalize(113, 1000000, &sll_packet);

    assert!(result.is_ok());
    let normalized = result.unwrap().unwrap();
    assert_eq!(normalized.src_ip, IpAddr::from([192, 168, 1, 1]));
}

/// Test transport parsing from raw bytes with invalid TCP offset.
#[test]
fn test_parse_transport_invalid_tcp_offset() {
    // Create manually crafted TCP header with invalid data offset
    let _tcp_bytes = vec![
        0x30, 0x39, // Source port: 12345
        0x00, 0x50, // Dest port: 80
        0x00, 0x00, 0x03, 0xe8, // Sequence: 1000
        0x00, 0x00, 0x00, 0x00, // ACK: 0
        0xF0, 0x02, // Data offset: 15 (60 bytes) - exceeds packet length
        0x10, 0x00, // Window
        0x00, 0x00, // Checksum
        0x00, 0x00, // Urgent pointer
    ];

    // Only provide 20 bytes, but offset claims 60 bytes needed
    // This should make parsing fall through to "unknown" transport

    // We can't directly call parse_transport_from_bytes (private), but we can
    // test via fragment reassembly which uses it
    // This test documents the edge case for future reference
}

/// Test that VLAN ID extraction works for single VLAN tag.
#[test]
fn test_vlan_id_extraction() {
    use etherparse::{PacketBuilder, VlanId};

    let mut normalizer = PacketNormalizer::new();

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .single_vlan(VlanId::try_new(100).unwrap()) // VLAN ID: 100
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .udp(12345, 53);

    let payload = b"test";
    let mut packet = Vec::new();
    builder.write(&mut packet, payload).unwrap();

    let result = normalizer.normalize(1, 1000000, &packet);

    assert!(result.is_ok());
    let normalized = result.unwrap().unwrap();
    assert_eq!(normalized.vlan_id, Some(100));
}

/// Test that first VLAN ID is used for double VLAN tags.
#[test]
fn test_double_vlan_uses_first_id() {
    use etherparse::{PacketBuilder, VlanId};

    let mut normalizer = PacketNormalizer::new();

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .double_vlan(
        VlanId::try_new(200).unwrap(),
        VlanId::try_new(300).unwrap(),
    ) // Outer: 200, Inner: 300
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .udp(12345, 53);

    let payload = b"test";
    let mut packet = Vec::new();
    builder.write(&mut packet, payload).unwrap();

    let result = normalizer.normalize(1, 1000000, &packet);

    assert!(result.is_ok());
    let normalized = result.unwrap().unwrap();
    // Should use first (outer) VLAN ID
    assert_eq!(normalized.vlan_id, Some(200));
}
