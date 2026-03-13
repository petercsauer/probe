//! Property-based tests for packet normalization: TCP header parsing safety.

use etherparse::{EtherType, Ethernet2Header, IpNumber, Ipv4Header};
use prb_pcap::normalize_stateless;
use proptest::prelude::*;

/// Creates an Ethernet + IPv4 packet with arbitrary transport payload.
fn create_packet_with_transport(transport_data: &[u8]) -> Vec<u8> {
    let mut packet = Vec::new();

    let eth = Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: EtherType(0x0800),
    };
    eth.write(&mut packet).unwrap();

    let ipv4 = Ipv4Header::new(
        transport_data.len() as u16,
        64,
        IpNumber(6), // TCP
        [192, 168, 1, 1],
        [10, 0, 0, 1],
    )
    .unwrap();
    ipv4.write(&mut packet).unwrap();

    packet.extend_from_slice(transport_data);
    packet
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Test that TCP header parsing never panics with arbitrary bytes.
    ///
    /// This tests the manual TCP header parsing in `parse_transport_from_bytes`
    /// which is used for reassembled IP fragments. The parser checks `data.len() >= 20`
    /// but then indexes into the array, so we want to ensure no out-of-bounds panics.
    #[test]
    fn tcp_header_parsing_never_panics(
        header_bytes in prop::collection::vec(any::<u8>(), 0..100)
    ) {
        let packet = create_packet_with_transport(&header_bytes);

        // Should never panic, even with arbitrary/malformed TCP headers
        let result = normalize_stateless(1, 1000000, &packet);

        // Can succeed or fail, but must not panic
        // If header is too short or malformed, etherparse will return an error
        let _ = result;
    }

    /// Test that various TCP header sizes don't cause panics.
    ///
    /// TCP header can be 20-60 bytes (with options). Test boundary cases.
    #[test]
    fn tcp_header_size_boundaries(
        header_size in 0usize..100,
        data_offset in 0u8..16,
        flags in any::<u8>(),
        ports in prop::array::uniform2(any::<u16>()),
    ) {
        let mut tcp_bytes = vec![0u8; header_size];

        if tcp_bytes.len() >= 14 {
            // Set valid port numbers
            tcp_bytes[0..2].copy_from_slice(&ports[0].to_be_bytes());
            tcp_bytes[2..4].copy_from_slice(&ports[1].to_be_bytes());

            // Set data offset and flags
            let offset_flags = ((data_offset as u16) << 12) | (flags as u16);
            tcp_bytes[12..14].copy_from_slice(&offset_flags.to_be_bytes());
        }

        let packet = create_packet_with_transport(&tcp_bytes);
        let result = normalize_stateless(1, 1000000, &packet);

        // Should not panic regardless of header size or offset value
        let _ = result;
    }

    /// Test that UDP header parsing is robust to arbitrary data.
    #[test]
    fn udp_header_parsing_never_panics(
        header_bytes in prop::collection::vec(any::<u8>(), 0..100)
    ) {
        let mut packet = Vec::new();

        let eth = Ethernet2Header {
            source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
            destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
            ether_type: EtherType(0x0800),
        };
        eth.write(&mut packet).unwrap();

        let ipv4 = Ipv4Header::new(
            header_bytes.len() as u16,
            64,
            IpNumber(17), // UDP
            [192, 168, 1, 1],
            [10, 0, 0, 1],
        )
        .unwrap();
        ipv4.write(&mut packet).unwrap();

        packet.extend_from_slice(&header_bytes);

        // Should not panic with arbitrary UDP data
        let result = normalize_stateless(1, 1000000, &packet);
        let _ = result;
    }

    /// Test that IP header length variations don't cause issues.
    #[test]
    fn ip_header_length_variations(
        payload_size in 0usize..200,
        ttl in any::<u8>(),
        protocol in any::<u8>(),
    ) {
        let mut packet = Vec::new();

        let eth = Ethernet2Header {
            source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
            destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
            ether_type: EtherType(0x0800),
        };
        eth.write(&mut packet).unwrap();

        // Create IP header with arbitrary protocol
        let ipv4_result = Ipv4Header::new(
            payload_size as u16,
            ttl,
            IpNumber(protocol),
            [192, 168, 1, 1],
            [10, 0, 0, 1],
        );

        if let Ok(ipv4) = ipv4_result {
            ipv4.write(&mut packet).unwrap();
            packet.extend_from_slice(&vec![0u8; payload_size]);

            // Should handle various IP payload sizes and protocols
            let result = normalize_stateless(1, 1000000, &packet);
            let _ = result;
        }
    }

    /// Test that timestamp values don't cause issues.
    #[test]
    fn timestamp_values_robust(timestamp_us in any::<u64>()) {
        use etherparse::PacketBuilder;

        let builder = PacketBuilder::ethernet2(
            [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
            [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        )
        .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
        .udp(12345, 80);

        let mut packet = Vec::new();
        builder.write(&mut packet, b"test").unwrap();

        // Should handle any timestamp value without panic
        let result = normalize_stateless(1, timestamp_us, &packet);
        let _ = result;
    }
}
