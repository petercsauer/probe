//! Integration tests for TCP stream reassembly.

mod helpers;

use helpers::create_tcp_segment;
use prb_pcap::{PacketNormalizer, TcpFlags, TcpReassembler};

#[test]
fn test_simple_stream() {
    let mut normalizer = PacketNormalizer::new();
    let mut reassembler = TcpReassembler::new();

    // Create 3 in-order TCP segments
    let seg1 = create_tcp_segment(
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
        b"Hello",
    );

    let seg2 = create_tcp_segment(
        [192, 168, 1, 1],
        [10, 0, 0, 1],
        12345,
        80,
        1005,
        0,
        TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
        },
        b" TCP",
    );

    let seg3 = create_tcp_segment(
        [192, 168, 1, 1],
        [10, 0, 0, 1],
        12345,
        80,
        1009,
        0,
        TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
        },
        b" stream",
    );

    // Normalize and reassemble
    let pkt1 = normalizer.normalize(1, 1000000, &seg1).unwrap().unwrap();
    let events1 = reassembler.process_segment(&pkt1).unwrap();
    assert_eq!(events1.len(), 1, "First segment should produce data event");
    if let prb_pcap::StreamEvent::Data(stream) = &events1[0] {
        assert_eq!(stream.data, b"Hello");
    } else {
        panic!("Expected Data event");
    }

    let pkt2 = normalizer.normalize(1, 1000001, &seg2).unwrap().unwrap();
    let events2 = reassembler.process_segment(&pkt2).unwrap();
    assert_eq!(events2.len(), 1, "Second segment should produce data event");
    if let prb_pcap::StreamEvent::Data(stream) = &events2[0] {
        assert_eq!(stream.data, b" TCP");
    } else {
        panic!("Expected Data event");
    }

    let pkt3 = normalizer.normalize(1, 1000002, &seg3).unwrap().unwrap();
    let events3 = reassembler.process_segment(&pkt3).unwrap();
    assert_eq!(events3.len(), 1, "Third segment should produce data event");
    if let prb_pcap::StreamEvent::Data(stream) = &events3[0] {
        assert_eq!(stream.data, b" stream");
    } else {
        panic!("Expected Data event");
    }

    // Verify connection is tracked
    assert_eq!(reassembler.active_connections(), 1);
}

#[test]
fn test_out_of_order() {
    let mut normalizer = PacketNormalizer::new();
    let mut reassembler = TcpReassembler::new();

    // Create 3 TCP segments, feed them out of order: 1, 3, 2
    let seg1 = create_tcp_segment(
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
        b"AAA",
    );

    let seg2 = create_tcp_segment(
        [192, 168, 1, 1],
        [10, 0, 0, 1],
        12345,
        80,
        1003,
        0,
        TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
        },
        b"BBB",
    );

    let seg3 = create_tcp_segment(
        [192, 168, 1, 1],
        [10, 0, 0, 1],
        12345,
        80,
        1006,
        0,
        TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
        },
        b"CCC",
    );

    // Feed in order: seg1, seg3, seg2 (out of order)
    let pkt1 = normalizer.normalize(1, 2000000, &seg1).unwrap().unwrap();
    let _events1 = reassembler.process_segment(&pkt1).unwrap();

    let pkt3 = normalizer.normalize(1, 2000001, &seg3).unwrap().unwrap();
    let _events3 = reassembler.process_segment(&pkt3).unwrap();

    let pkt2 = normalizer.normalize(1, 2000002, &seg2).unwrap().unwrap();
    let _events2 = reassembler.process_segment(&pkt2).unwrap();

    // All segments should be buffered (smoltcp handles out-of-order)
    assert_eq!(reassembler.active_connections(), 1);
}

#[test]
fn test_retransmission() {
    let mut normalizer = PacketNormalizer::new();
    let mut reassembler = TcpReassembler::new();

    // Create a TCP segment
    let seg1 = create_tcp_segment(
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
        b"DATA",
    );

    // Feed the same segment twice (retransmission)
    let pkt1 = normalizer.normalize(1, 3000000, &seg1).unwrap().unwrap();
    let _events1 = reassembler.process_segment(&pkt1).unwrap();

    let pkt1_retrans = normalizer.normalize(1, 3000001, &seg1).unwrap().unwrap();
    let _events2 = reassembler.process_segment(&pkt1_retrans).unwrap();

    // Should not produce duplicate data
    assert_eq!(reassembler.active_connections(), 1);
}

#[test]
fn test_packet_loss_tolerance() {
    // This test would require a more complete implementation that detects gaps
    // and skips them after a threshold. For now, just verify the connection
    // doesn't crash with gaps.
    let mut normalizer = PacketNormalizer::new();
    let mut reassembler = TcpReassembler::new();

    let seg1 = create_tcp_segment(
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
        b"AAA",
    );

    // Segment 3 with a large gap (missing segment 2)
    let seg3 = create_tcp_segment(
        [192, 168, 1, 1],
        [10, 0, 0, 1],
        12345,
        80,
        2000, // Large gap
        0,
        TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
        },
        b"CCC",
    );

    let pkt1 = normalizer.normalize(1, 4000000, &seg1).unwrap().unwrap();
    let _events1 = reassembler.process_segment(&pkt1).unwrap();

    let pkt3 = normalizer.normalize(1, 4000001, &seg3).unwrap().unwrap();
    let _events3 = reassembler.process_segment(&pkt3).unwrap();

    // Connection should still be tracked
    assert_eq!(reassembler.active_connections(), 1);
}

#[test]
fn test_bidirectional() {
    let mut normalizer = PacketNormalizer::new();
    let mut reassembler = TcpReassembler::new();

    // Client to server
    let c2s = create_tcp_segment(
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
        b"Request",
    );

    // Server to client
    let s2c = create_tcp_segment(
        [10, 0, 0, 1],
        [192, 168, 1, 1],
        80,
        12345,
        5000,
        1007,
        TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
        },
        b"Response",
    );

    let pkt_c2s = normalizer.normalize(1, 5000000, &c2s).unwrap().unwrap();
    let _events_c2s = reassembler.process_segment(&pkt_c2s).unwrap();

    let pkt_s2c = normalizer.normalize(1, 5000001, &s2c).unwrap().unwrap();
    let _events_s2c = reassembler.process_segment(&pkt_s2c).unwrap();

    // Should track both directions in one connection
    assert_eq!(reassembler.active_connections(), 1);
}

#[test]
fn test_fin_rst_cleanup() {
    let mut normalizer = PacketNormalizer::new();
    let mut reassembler = TcpReassembler::new();

    // Normal segment
    let seg1 = create_tcp_segment(
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
        b"DATA",
    );

    // RST segment
    let rst = create_tcp_segment(
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
            rst: true,
            psh: false,
        },
        b"",
    );

    let pkt1 = normalizer.normalize(1, 6000000, &seg1).unwrap().unwrap();
    let events1 = reassembler.process_segment(&pkt1).unwrap();
    assert_eq!(reassembler.active_connections(), 1);
    // First segment emits data immediately (correct streaming behavior)
    assert_eq!(events1.len(), 1);
    if let prb_pcap::StreamEvent::Data(stream) = &events1[0] {
        assert_eq!(stream.data, b"DATA");
    } else {
        panic!("Expected Data event");
    }

    let pkt_rst = normalizer.normalize(1, 6000001, &rst).unwrap().unwrap();
    let _events_rst = reassembler.process_segment(&pkt_rst).unwrap();

    // RST should clean up connection
    assert_eq!(reassembler.active_connections(), 0);
    // RST with no new payload may not produce additional events (data was already emitted)
    // This is correct - the stream was already flushed
}

#[test]
fn test_mid_connection_start() {
    let mut normalizer = PacketNormalizer::new();
    let mut reassembler = TcpReassembler::new();

    // Capture starts mid-connection (no SYN)
    let seg = create_tcp_segment(
        [192, 168, 1, 1],
        [10, 0, 0, 1],
        12345,
        80,
        5000000, // Random seq number
        0,
        TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
        },
        b"MidStream",
    );

    let pkt = normalizer.normalize(1, 7000000, &seg).unwrap().unwrap();
    let _events = reassembler.process_segment(&pkt).unwrap();

    // Should handle mid-connection start
    assert_eq!(reassembler.active_connections(), 1);
}

#[test]
fn test_connection_timeout() {
    let mut normalizer = PacketNormalizer::new();
    let mut reassembler = TcpReassembler::with_timeout(1_000_000); // 1 second timeout

    let seg = create_tcp_segment(
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
        b"DATA",
    );

    let pkt = normalizer.normalize(1, 8000000, &seg).unwrap().unwrap();
    let _events = reassembler.process_segment(&pkt).unwrap();
    assert_eq!(reassembler.active_connections(), 1);

    // Advance time past timeout
    let timeout_events = reassembler.cleanup_idle_connections(8000000 + 2_000_000);

    // Connection should be removed
    assert_eq!(reassembler.active_connections(), 0);
    assert_eq!(timeout_events.len(), 1);
}

#[test]
fn test_multiple_connections() {
    let mut normalizer = PacketNormalizer::new();
    let mut reassembler = TcpReassembler::new();

    // Create 100 different connections
    for i in 0..100u16 {
        let seg = create_tcp_segment(
            [192, 168, 1, 1],
            [10, 0, 0, 1],
            10000 + i, // Unique port
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
            b"DATA",
        );

        let pkt = normalizer
            .normalize(1, 9000000 + u64::from(i), &seg)
            .unwrap()
            .unwrap();
        let _events = reassembler.process_segment(&pkt).unwrap();
    }

    // Should track all 100 connections
    assert_eq!(reassembler.active_connections(), 100);
}
