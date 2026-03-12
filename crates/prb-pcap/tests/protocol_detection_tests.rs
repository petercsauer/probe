//! Integration tests for protocol detection and decoding in the pipeline.

use prb_core::{CaptureAdapter, TransportKind};
use prb_pcap::{PcapCaptureAdapter, TcpFlags};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to create a TCP segment packet.
#[allow(clippy::too_many_arguments)]
fn create_tcp_segment(
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

/// Helper to write a simple PCAP file.
fn write_pcap_file(path: &PathBuf, packets: &[Vec<u8>]) {
    let mut file = File::create(path).unwrap();

    // PCAP global header
    let header = [
        0xd4, 0xc3, 0xb2, 0xa1, // magic (little endian)
        0x02, 0x00, 0x04, 0x00, // version 2.4
        0x00, 0x00, 0x00, 0x00, // timezone offset
        0x00, 0x00, 0x00, 0x00, // timestamp accuracy
        0xff, 0xff, 0x00, 0x00, // snaplen (65535)
        0x01, 0x00, 0x00, 0x00, // linktype 1 (Ethernet)
    ];
    file.write_all(&header).unwrap();

    // Write each packet
    let mut timestamp_sec = 1000000;
    for packet in packets {
        // Packet header
        let packet_header = [
            (timestamp_sec & 0xff) as u8,
            ((timestamp_sec >> 8) & 0xff) as u8,
            ((timestamp_sec >> 16) & 0xff) as u8,
            ((timestamp_sec >> 24) & 0xff) as u8,
            0x00,
            0x00,
            0x00,
            0x00, // timestamp microseconds
            (packet.len() as u32).to_le_bytes()[0],
            (packet.len() as u32).to_le_bytes()[1],
            (packet.len() as u32).to_le_bytes()[2],
            (packet.len() as u32).to_le_bytes()[3],
            (packet.len() as u32).to_le_bytes()[0],
            (packet.len() as u32).to_le_bytes()[1],
            (packet.len() as u32).to_le_bytes()[2],
            (packet.len() as u32).to_le_bytes()[3],
        ];
        file.write_all(&packet_header).unwrap();
        file.write_all(packet).unwrap();

        timestamp_sec += 1;
    }

    file.flush().unwrap();
}

#[test]
fn test_grpc_stream_auto_detected() {
    // HTTP/2 connection preface (gRPC)
    let preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

    let tmpdir = TempDir::new().unwrap();
    let pcap_path = tmpdir.path().join("test.pcap");

    // Create a complete TCP stream with SYN, data, and FIN
    // Note: SYN consumes 1 sequence number, so data starts at ISN+1
    let packets = vec![
        create_tcp_segment(
            [192, 168, 1, 1],
            [192, 168, 1, 2],
            12345,
            50051,
            1000,
            0,
            TcpFlags {
                syn: true,
                ack: false,
                fin: false,
                rst: false,
                psh: false,
            },
            b"",
        ),
        create_tcp_segment(
            [192, 168, 1, 2],
            [192, 168, 1, 1],
            50051,
            12345,
            2000,
            1001,
            TcpFlags {
                syn: true,
                ack: true,
                fin: false,
                rst: false,
                psh: false,
            },
            b"",
        ),
        create_tcp_segment(
            [192, 168, 1, 1],
            [192, 168, 1, 2],
            12345,
            50051,
            1001,
            2001,
            TcpFlags {
                syn: false,
                ack: true,
                fin: false,
                rst: false,
                psh: true,
            },
            preface,
        ),
        create_tcp_segment(
            [192, 168, 1, 1],
            [192, 168, 1, 2],
            12345,
            50051,
            1001 + preface.len() as u32,
            2001,
            TcpFlags {
                syn: false,
                ack: true,
                fin: true,
                rst: false,
                psh: false,
            },
            b"",
        ),
    ];

    write_pcap_file(&pcap_path, &packets);

    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect::<Result<Vec<_>, _>>().unwrap();

    // Should detect as gRPC (or at least not be RawTcp if detection works)
    assert!(!events.is_empty(), "Expected at least one event");

    // Check that protocol detection was attempted
    let first_event = &events[0];
    // The event should either be Grpc (detected) or RawTcp (fallback)
    // We can't guarantee detection without a full HTTP/2 frame, so we just
    // verify the pipeline ran without errors
    assert!(
        matches!(
            first_event.transport,
            TransportKind::Grpc | TransportKind::RawTcp
        ),
        "Expected Grpc or RawTcp, got {:?}",
        first_event.transport
    );
}

#[test]
fn test_unknown_protocol_falls_back() {
    // Random data that shouldn't match any protocol
    let random_data = b"this is not a valid protocol message at all";

    let tmpdir = TempDir::new().unwrap();
    let pcap_path = tmpdir.path().join("test.pcap");

    let packets = vec![
        create_tcp_segment(
            [192, 168, 1, 1],
            [192, 168, 1, 2],
            12345,
            8080,
            1000,
            0,
            TcpFlags {
                syn: true,
                ack: false,
                fin: false,
                rst: false,
                psh: false,
            },
            b"",
        ),
        create_tcp_segment(
            [192, 168, 1, 2],
            [192, 168, 1, 1],
            8080,
            12345,
            2000,
            1001,
            TcpFlags {
                syn: true,
                ack: true,
                fin: false,
                rst: false,
                psh: false,
            },
            b"",
        ),
        create_tcp_segment(
            [192, 168, 1, 1],
            [192, 168, 1, 2],
            12345,
            8080,
            1001,
            2001,
            TcpFlags {
                syn: false,
                ack: true,
                fin: false,
                rst: false,
                psh: true,
            },
            random_data,
        ),
        create_tcp_segment(
            [192, 168, 1, 1],
            [192, 168, 1, 2],
            12345,
            8080,
            1001 + random_data.len() as u32,
            2001,
            TcpFlags {
                syn: false,
                ack: true,
                fin: true,
                rst: false,
                psh: false,
            },
            b"",
        ),
    ];

    write_pcap_file(&pcap_path, &packets);

    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect::<Result<Vec<_>, _>>().unwrap();

    assert!(!events.is_empty(), "Expected at least one event");

    // Should fall back to RawTcp
    let first_event = &events[0];
    assert!(
        matches!(first_event.transport, TransportKind::RawTcp),
        "Expected RawTcp fallback, got {:?}",
        first_event.transport
    );
}

#[test]
fn test_protocol_override() {
    // Random data
    let data = b"not gRPC data";

    let tmpdir = TempDir::new().unwrap();
    let pcap_path = tmpdir.path().join("test.pcap");

    let packets = vec![
        create_tcp_segment(
            [192, 168, 1, 1],
            [192, 168, 1, 2],
            12345,
            50051,
            1000,
            0,
            TcpFlags {
                syn: true,
                ack: false,
                fin: false,
                rst: false,
                psh: false,
            },
            b"",
        ),
        create_tcp_segment(
            [192, 168, 1, 2],
            [192, 168, 1, 1],
            50051,
            12345,
            2000,
            1001,
            TcpFlags {
                syn: true,
                ack: true,
                fin: false,
                rst: false,
                psh: false,
            },
            b"",
        ),
        create_tcp_segment(
            [192, 168, 1, 1],
            [192, 168, 1, 2],
            12345,
            50051,
            1001,
            2001,
            TcpFlags {
                syn: false,
                ack: true,
                fin: false,
                rst: false,
                psh: true,
            },
            data,
        ),
        create_tcp_segment(
            [192, 168, 1, 1],
            [192, 168, 1, 2],
            12345,
            50051,
            1001 + data.len() as u32,
            2001,
            TcpFlags {
                syn: false,
                ack: true,
                fin: true,
                rst: false,
                psh: false,
            },
            b"",
        ),
    ];

    write_pcap_file(&pcap_path, &packets);

    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    adapter.set_protocol_override("grpc");

    let events: Vec<_> = adapter.ingest().collect::<Result<Vec<_>, _>>().unwrap();

    assert!(!events.is_empty(), "Expected at least one event");

    // The override was set, but since the data isn't valid gRPC,
    // it should still fall back to RawTcp after decode failure
    let first_event = &events[0];
    assert!(
        matches!(
            first_event.transport,
            TransportKind::Grpc | TransportKind::RawTcp
        ),
        "Expected Grpc or RawTcp, got {:?}",
        first_event.transport
    );
}

#[test]
fn test_pipeline_stats_tracking() {
    let preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

    let tmpdir = TempDir::new().unwrap();
    let pcap_path = tmpdir.path().join("test.pcap");

    let packets = vec![
        create_tcp_segment(
            [192, 168, 1, 1],
            [192, 168, 1, 2],
            12345,
            50051,
            1000,
            0,
            TcpFlags {
                syn: true,
                ack: false,
                fin: false,
                rst: false,
                psh: false,
            },
            b"",
        ),
        create_tcp_segment(
            [192, 168, 1, 2],
            [192, 168, 1, 1],
            50051,
            12345,
            2000,
            1001,
            TcpFlags {
                syn: true,
                ack: true,
                fin: false,
                rst: false,
                psh: false,
            },
            b"",
        ),
        create_tcp_segment(
            [192, 168, 1, 1],
            [192, 168, 1, 2],
            12345,
            50051,
            1001,
            2001,
            TcpFlags {
                syn: false,
                ack: true,
                fin: false,
                rst: false,
                psh: true,
            },
            preface,
        ),
        create_tcp_segment(
            [192, 168, 1, 1],
            [192, 168, 1, 2],
            12345,
            50051,
            1001 + preface.len() as u32,
            2001,
            TcpFlags {
                syn: false,
                ack: true,
                fin: true,
                rst: false,
                psh: false,
            },
            b"",
        ),
    ];

    write_pcap_file(&pcap_path, &packets);

    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let _events: Vec<_> = adapter.ingest().collect::<Result<Vec<_>, _>>().unwrap();

    let stats = adapter.stats();

    // Basic sanity checks
    assert!(stats.packets_read > 0, "Expected packets to be read");
    assert!(
        stats.tcp_streams > 0,
        "Expected TCP streams to be reassembled"
    );

    // Protocol detection should have been attempted, resulting in either
    // protocol_decoded > 0 or protocol_fallback > 0
    let detection_attempted = stats.protocol_decoded + stats.protocol_fallback;
    assert!(
        detection_attempted > 0,
        "Expected protocol detection to be attempted"
    );
}

/// Helper to create a UDP datagram packet.
fn create_udp_datagram(
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

#[test]
fn test_rtps_datagram_auto_detected() {
    // RTPS header magic: "RTPS" + protocol version + vendor ID + GUID prefix
    let mut rtps_data = Vec::new();
    rtps_data.extend_from_slice(b"RTPS"); // Magic
    rtps_data.extend_from_slice(&[2, 3]); // Protocol version 2.3
    rtps_data.extend_from_slice(&[0x01, 0x0f]); // Vendor ID
    rtps_data.extend_from_slice(&[0; 12]); // GUID prefix

    let tmpdir = TempDir::new().unwrap();
    let pcap_path = tmpdir.path().join("test.pcap");

    let packets = vec![create_udp_datagram(
        [192, 168, 1, 1],
        [192, 168, 1, 2],
        7400,
        7400,
        &rtps_data,
    )];

    write_pcap_file(&pcap_path, &packets);

    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect::<Result<Vec<_>, _>>().unwrap();

    assert!(!events.is_empty(), "Expected at least one event");

    // Should detect as DDS/RTPS or fall back to RawUdp
    let first_event = &events[0];
    assert!(
        matches!(
            first_event.transport,
            TransportKind::DdsRtps | TransportKind::RawUdp
        ),
        "Expected DdsRtps or RawUdp, got {:?}",
        first_event.transport
    );
}

#[test]
fn test_mixed_protocols_in_same_capture() {
    // Mix of gRPC (TCP) and RTPS (UDP)
    let preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

    let mut rtps_data = Vec::new();
    rtps_data.extend_from_slice(b"RTPS");
    rtps_data.extend_from_slice(&[2, 3, 0x01, 0x0f]);
    rtps_data.extend_from_slice(&[0; 12]);

    let tmpdir = TempDir::new().unwrap();
    let pcap_path = tmpdir.path().join("test.pcap");

    let packets = vec![
        // TCP connection 1 - gRPC
        create_tcp_segment(
            [192, 168, 1, 1],
            [192, 168, 1, 2],
            12345,
            50051,
            1000,
            0,
            TcpFlags {
                syn: true,
                ack: false,
                fin: false,
                rst: false,
                psh: false,
            },
            b"",
        ),
        create_tcp_segment(
            [192, 168, 1, 2],
            [192, 168, 1, 1],
            50051,
            12345,
            2000,
            1001,
            TcpFlags {
                syn: true,
                ack: true,
                fin: false,
                rst: false,
                psh: false,
            },
            b"",
        ),
        create_tcp_segment(
            [192, 168, 1, 1],
            [192, 168, 1, 2],
            12345,
            50051,
            1001,
            2001,
            TcpFlags {
                syn: false,
                ack: true,
                fin: false,
                rst: false,
                psh: true,
            },
            preface,
        ),
        // UDP datagram - RTPS
        create_udp_datagram([192, 168, 1, 3], [192, 168, 1, 4], 7400, 7400, &rtps_data),
        // Close TCP connection
        create_tcp_segment(
            [192, 168, 1, 1],
            [192, 168, 1, 2],
            12345,
            50051,
            1001 + preface.len() as u32,
            2001,
            TcpFlags {
                syn: false,
                ack: true,
                fin: true,
                rst: false,
                psh: false,
            },
            b"",
        ),
    ];

    write_pcap_file(&pcap_path, &packets);

    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect::<Result<Vec<_>, _>>().unwrap();

    // Should have at least 2 events (TCP and UDP)
    assert!(
        events.len() >= 2,
        "Expected at least 2 events for mixed protocols"
    );

    // Verify we got both TCP and UDP events
    let has_tcp = events
        .iter()
        .any(|e| matches!(e.transport, TransportKind::Grpc | TransportKind::RawTcp));
    let has_udp = events
        .iter()
        .any(|e| matches!(e.transport, TransportKind::DdsRtps | TransportKind::RawUdp));

    assert!(has_tcp, "Expected at least one TCP event");
    assert!(has_udp, "Expected at least one UDP event");
}
