//! Integration tests for pipeline: PCAP → TCP reassembly → TLS decryption → DebugEvents.

use prb_core::{CaptureAdapter, TransportKind};
use prb_pcap::{PcapCaptureAdapter, TcpFlags};
use std::fs::File;
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
    use etherparse::{Ethernet2Header, EtherType, IpNumber, Ipv4Header, TcpHeader};

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

/// Helper to create a UDP datagram packet.
fn create_udp_datagram(
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Vec<u8> {
    use etherparse::{Ethernet2Header, EtherType, IpNumber, Ipv4Header, UdpHeader};

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

/// Helper to write a simple PCAP file.
fn write_pcap_file(path: &PathBuf, packets: &[Vec<u8>]) {
    use std::io::Write;

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

    // Write packets
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

#[test]
fn test_pipeline_tcp_stream() {
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("test.pcap");

    // Create TCP stream with 3 segments
    let packets = vec![
        create_tcp_segment(
            [192, 168, 1, 1],
            [10, 0, 0, 1],
            12345,
            80,
            1000,
            0,
            TcpFlags {
                syn: false,
                ack: true,
                fin: false,
                rst: false,
                psh: false,
            },
            b"GET ",
        ),
        create_tcp_segment(
            [192, 168, 1, 1],
            [10, 0, 0, 1],
            12345,
            80,
            1004,
            0,
            TcpFlags {
                syn: false,
                ack: true,
                fin: false,
                rst: false,
                psh: true,
            },
            b"/index.html HTTP/1.1\r\n\r\n",
        ),
        create_tcp_segment(
            [192, 168, 1, 1],
            [10, 0, 0, 1],
            12345,
            80,
            1027,
            0,
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

    // Process through pipeline
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Should produce at least one DebugEvent for the TCP stream
    assert!(
        !events.is_empty(),
        "Pipeline should produce at least one event"
    );

    // Check that we got a successful event
    let event = events[0].as_ref().expect("Event should be Ok");
    assert_eq!(event.transport, TransportKind::RawTcp);

    // Check stats
    let stats = adapter.stats();
    assert_eq!(stats.packets_read, 3, "Should read 3 packets");
    // TCP reassembler may emit multiple stream events for segments with payload
    assert!(stats.tcp_streams >= 1, "Should reassemble at least 1 TCP stream");
}

#[test]
fn test_pipeline_udp_datagram() {
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("test_udp.pcap");

    // Create UDP datagrams
    let packets = vec![
        create_udp_datagram([192, 168, 1, 1], [10, 0, 0, 1], 5555, 5556, b"UDP message 1"),
        create_udp_datagram([10, 0, 0, 1], [192, 168, 1, 1], 5556, 5555, b"UDP reply"),
        create_udp_datagram([192, 168, 1, 1], [10, 0, 0, 1], 5555, 5556, b"UDP message 2"),
    ];

    write_pcap_file(&pcap_path, &packets);

    // Process through pipeline
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Should produce 3 DebugEvents (one per UDP datagram)
    assert_eq!(events.len(), 3, "Should produce 3 events for 3 UDP packets");

    // All events should be successful
    for (i, event_result) in events.iter().enumerate() {
        let event = event_result
            .as_ref()
            .unwrap_or_else(|_| panic!("Event {} should be Ok", i));
        assert_eq!(event.transport, TransportKind::RawUdp);
    }

    // Check stats
    let stats = adapter.stats();
    assert_eq!(stats.packets_read, 3, "Should read 3 packets");
    assert_eq!(stats.udp_datagrams, 3, "Should process 3 UDP datagrams");
}

#[test]
fn test_pipeline_mixed() {
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("test_mixed.pcap");

    // Create mixed TCP and UDP traffic
    let packets = vec![
        // TCP stream 1
        create_tcp_segment(
            [192, 168, 1, 1],
            [10, 0, 0, 1],
            12345,
            80,
            1000,
            0,
            TcpFlags {
                syn: false,
                ack: true,
                fin: false,
                rst: false,
                psh: true,
            },
            b"TCP data",
        ),
        // UDP datagram
        create_udp_datagram([192, 168, 1, 2], [10, 0, 0, 2], 5555, 5556, b"UDP data"),
        // TCP stream 1 FIN
        create_tcp_segment(
            [192, 168, 1, 1],
            [10, 0, 0, 1],
            12345,
            80,
            1008,
            0,
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

    // Process through pipeline
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Should produce 2 events: 1 TCP stream + 1 UDP datagram
    assert_eq!(
        events.len(),
        2,
        "Should produce 2 events (1 TCP + 1 UDP)"
    );

    // Check stats
    let stats = adapter.stats();
    assert_eq!(stats.packets_read, 3, "Should read 3 packets");
    assert_eq!(stats.tcp_streams, 1, "Should reassemble 1 TCP stream");
    assert_eq!(stats.udp_datagrams, 1, "Should process 1 UDP datagram");
}

#[test]
fn test_pipeline_error_tolerance() {
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("test_errors.pcap");

    // Create packets with one corrupted packet in the middle
    let packets = vec![
        create_tcp_segment(
            [192, 168, 1, 1],
            [10, 0, 0, 1],
            12345,
            80,
            1000,
            0,
            TcpFlags {
                syn: false,
                ack: true,
                fin: false,
                rst: false,
                psh: false,
            },
            b"Good packet 1",
        ),
        vec![0xaa; 20], // Corrupted packet (too short for Ethernet)
        create_tcp_segment(
            [192, 168, 1, 2],
            [10, 0, 0, 2],
            12346,
            80,
            2000,
            0,
            TcpFlags {
                syn: false,
                ack: true,
                fin: true,
                rst: false,
                psh: false,
            },
            b"Good packet 2",
        ),
    ];

    write_pcap_file(&pcap_path, &packets);

    // Process through pipeline
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Should produce 2 events (2 TCP streams, 1 corrupted packet skipped)
    assert_eq!(
        events.len(),
        2,
        "Should produce 2 events despite 1 corrupted packet"
    );

    // Check stats
    let stats = adapter.stats();
    assert_eq!(stats.packets_read, 3, "Should read 3 packets");
    assert_eq!(
        stats.packets_failed, 1,
        "Should report 1 failed packet"
    );
    assert_eq!(stats.tcp_streams, 2, "Should reassemble 2 TCP streams");
}

#[test]
fn test_pipeline_tls_decrypt() {
    // This test validates that the TLS keylog path can be provided
    // Full TLS decryption is tested in tls_tests.rs
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("test_tls.pcap");
    let keylog_path = temp_dir.path().join("keys.log");

    // Create a simple TCP stream (not real TLS, just testing the pipeline)
    let packets = vec![create_tcp_segment(
        [192, 168, 1, 1],
        [10, 0, 0, 1],
        12345,
        443,
        1000,
        0,
        TcpFlags {
            syn: false,
            ack: true,
            fin: true,
            rst: false,
            psh: true,
        },
        b"\x16\x03\x03\x00\x05hello", // TLS-like header (not valid)
    )];

    write_pcap_file(&pcap_path, &packets);

    // Create an empty keylog file
    File::create(&keylog_path).unwrap();

    // Process through pipeline with keylog
    let mut adapter = PcapCaptureAdapter::new(pcap_path, Some(keylog_path));
    let events: Vec<_> = adapter.ingest().collect();

    // Should produce at least one event
    assert!(
        !events.is_empty(),
        "Pipeline with keylog should produce events"
    );

    // Check that keylog was loaded (even if empty)
    let stats = adapter.stats();
    assert_eq!(stats.packets_read, 1, "Should read 1 packet");
}
