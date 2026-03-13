//! Test helper utilities for creating PCAP files and network packets.

#![allow(dead_code)]

use prb_pcap::TcpFlags;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Helper to create a TCP segment packet with Ethernet, IPv4, and TCP headers.
#[allow(clippy::too_many_arguments)]
pub fn create_tcp_segment(
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    flags: TcpFlags,
    payload: &[u8],
) -> Vec<u8> {
    use etherparse::{EtherType, Ethernet2Header, IpNumber, Ipv4Header, TcpHeader};

    let mut packet = Vec::new();

    // Ethernet header
    let eth = Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: EtherType(0x0800), // IPv4
    };
    eth.write(&mut packet).unwrap();

    // IPv4 header
    let payload_len = (20 + payload.len()) as u16; // TCP header (20) + payload
    let ipv4 = Ipv4Header::new(payload_len, 64, IpNumber(6), src_ip, dst_ip).unwrap();
    ipv4.write(&mut packet).unwrap();

    // TCP header
    let mut tcp = TcpHeader::new(src_port, dst_port, seq, 4096);
    tcp.acknowledgment_number = ack;
    tcp.syn = flags.syn;
    tcp.ack = flags.ack;
    tcp.fin = flags.fin;
    tcp.rst = flags.rst;
    tcp.psh = flags.psh;
    tcp.write(&mut packet).unwrap();

    // Payload
    packet.extend_from_slice(payload);

    packet
}

/// Helper to create a UDP datagram packet with Ethernet, IPv4, and UDP headers.
pub fn create_udp_datagram(
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Vec<u8> {
    use etherparse::{EtherType, Ethernet2Header, IpNumber, Ipv4Header, UdpHeader};

    let mut packet = Vec::new();

    // Ethernet header
    let eth = Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: EtherType(0x0800), // IPv4
    };
    eth.write(&mut packet).unwrap();

    // IPv4 header
    let payload_len = (8 + payload.len()) as u16; // UDP header (8) + payload
    let ipv4 = Ipv4Header::new(payload_len, 64, IpNumber(17), src_ip, dst_ip).unwrap();
    ipv4.write(&mut packet).unwrap();

    // UDP header
    let udp = UdpHeader {
        source_port: src_port,
        destination_port: dst_port,
        length: (8 + payload.len()) as u16,
        checksum: 0, // Not validated in tests
    };
    udp.write(&mut packet).unwrap();

    // Payload
    packet.extend_from_slice(payload);

    packet
}

/// Helper to write a simple PCAP file with the given packets.
///
/// Creates a PCAP file with a standard global header and writes each packet
/// with incrementing timestamps.
pub fn write_pcap_file<P: AsRef<Path>>(path: P, packets: &[Vec<u8>]) {
    let mut file = File::create(path).unwrap();

    // PCAP global header
    let header = [
        0xd4, 0xc3, 0xb2, 0xa1, // Magic number (little-endian)
        0x02, 0x00, // Version major
        0x04, 0x00, // Version minor
        0x00, 0x00, 0x00, 0x00, // Timezone offset
        0x00, 0x00, 0x00, 0x00, // Timestamp accuracy
        0xff, 0xff, 0x00, 0x00, // Snaplen (65535)
        0x01, 0x00, 0x00, 0x00, // Link-layer type (Ethernet)
    ];
    file.write_all(&header).unwrap();

    // Write packets with incrementing timestamps
    let mut ts_sec = 1700000000u32;
    let ts_usec = 0u32;

    for packet in packets {
        ts_sec += 1; // Increment timestamp

        // Packet header
        file.write_all(&ts_sec.to_le_bytes()).unwrap(); // Timestamp seconds
        file.write_all(&ts_usec.to_le_bytes()).unwrap(); // Timestamp microseconds
        file.write_all(&(packet.len() as u32).to_le_bytes())
            .unwrap(); // Included length
        file.write_all(&(packet.len() as u32).to_le_bytes())
            .unwrap(); // Original length

        // Packet data
        file.write_all(packet).unwrap();
    }

    file.flush().unwrap();
}
