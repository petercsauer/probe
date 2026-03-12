//! Comprehensive tests for streaming pipeline edge cases.

use prb_pcap::parallel::streaming::StreamingPipeline;
use prb_pcap::reader::PcapPacket;
use prb_pcap::tls::TlsKeyLog;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn create_synthetic_packet(timestamp_us: u64, payload: &[u8]) -> PcapPacket {
    use etherparse::PacketBuilder;

    let builder = PacketBuilder::ethernet2(
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
    )
    .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
    .udp(12345, 80);

    let mut data = Vec::new();
    builder.write(&mut data, payload).unwrap();

    PcapPacket {
        linktype: 1, // Ethernet
        timestamp_us,
        data,
    }
}

#[test]
fn test_streaming_micro_batch_accumulation() {
    let keylog = Arc::new(TlsKeyLog::new());
    let pipeline = StreamingPipeline::new(
        16, // Small batch size to test accumulation
        2,
        PathBuf::from("/test.pcap"),
        keylog,
    );

    let (tx, handle) = pipeline.start();

    // Send exactly one batch worth of packets
    for i in 0..16 {
        let pkt = create_synthetic_packet(1000 + i, b"batch test");
        tx.send(pkt).unwrap();
    }

    drop(tx);

    let events = handle.recv_all();
    assert_eq!(events.len(), 16);

    let stats = handle.stats();
    assert_eq!(stats.packets_received, 16);
    assert_eq!(stats.packets_normalized, 16);
}

#[test]
fn test_streaming_partial_batch_flush() {
    // Verify partial batches are flushed on EOF
    let keylog = Arc::new(TlsKeyLog::new());
    let pipeline = StreamingPipeline::new(
        32, // Batch size
        2,
        PathBuf::from("/test.pcap"),
        keylog,
    );

    let (tx, handle) = pipeline.start();

    // Send less than one batch (13 packets)
    for i in 0..13 {
        let pkt = create_synthetic_packet(1000 + i, b"partial");
        tx.send(pkt).unwrap();
    }

    drop(tx);

    let events = handle.recv_all();
    assert_eq!(events.len(), 13, "Partial batch should be flushed");

    let stats = handle.stats();
    assert_eq!(stats.packets_received, 13);
}

#[test]
fn test_streaming_multiple_batches() {
    let keylog = Arc::new(TlsKeyLog::new());
    let pipeline = StreamingPipeline::new(16, 2, PathBuf::from("/test.pcap"), keylog);

    let (tx, handle) = pipeline.start();

    // Send 2.5 batches (40 packets)
    for i in 0..40 {
        let pkt = create_synthetic_packet(1000 + i, b"multi batch");
        tx.send(pkt).unwrap();
    }

    drop(tx);

    let events = handle.recv_all();
    assert_eq!(events.len(), 40);

    let stats = handle.stats();
    assert_eq!(stats.packets_received, 40);
}

#[test]
fn test_streaming_slow_sender() {
    // Simulate slow packet arrival
    let keylog = Arc::new(TlsKeyLog::new());
    let pipeline = StreamingPipeline::new(32, 2, PathBuf::from("/test.pcap"), keylog);

    let (tx, handle) = pipeline.start();

    thread::spawn(move || {
        for i in 0..10 {
            let pkt = create_synthetic_packet(1000 + i, b"slow");
            tx.send(pkt).unwrap();
            thread::sleep(Duration::from_millis(1)); // Slow sender
        }
    });

    let events = handle.recv_all();
    assert_eq!(events.len(), 10);
}

#[test]
fn test_streaming_fast_burst() {
    // Send a fast burst of packets
    let keylog = Arc::new(TlsKeyLog::new());
    let pipeline = StreamingPipeline::new(64, 4, PathBuf::from("/test.pcap"), keylog);

    let (tx, handle) = pipeline.start();

    // Send 500 packets as fast as possible
    for i in 0..500 {
        let pkt = create_synthetic_packet(1000 + i, b"burst");
        tx.send(pkt).unwrap();
    }

    drop(tx);

    let events = handle.recv_all();
    assert_eq!(events.len(), 500);

    let stats = handle.stats();
    assert_eq!(stats.packets_received, 500);
}

#[test]
fn test_streaming_channel_capacity_respected() {
    // Verify bounded channels enforce backpressure
    let keylog = Arc::new(TlsKeyLog::new());
    let pipeline = StreamingPipeline::new(
        8, // Small batch
        2,
        PathBuf::from("/test.pcap"),
        keylog,
    );

    let (tx, handle) = pipeline.start();

    // Spawn sender thread - bounded channel will apply backpressure
    thread::spawn(move || {
        for i in 0..1000 {
            let pkt = create_synthetic_packet(1000 + i, b"backpressure test");
            tx.send(pkt).unwrap();
        }
    });

    let events = handle.recv_all();
    assert_eq!(events.len(), 1000);
}

#[test]
fn test_streaming_stats_collection() {
    let keylog = Arc::new(TlsKeyLog::new());
    let pipeline = StreamingPipeline::new(16, 2, PathBuf::from("/test.pcap"), keylog);

    let (tx, handle) = pipeline.start();

    for i in 0..50 {
        let pkt = create_synthetic_packet(1000 + i, b"stats test");
        tx.send(pkt).unwrap();
    }

    drop(tx);
    let _ = handle.recv_all();

    let stats = handle.stats();
    assert_eq!(stats.packets_received, 50);
    assert_eq!(stats.packets_normalized, 50);
    assert_eq!(stats.packets_routed, 50);
    assert_eq!(stats.normalize_errors, 0);
}

#[test]
fn test_streaming_immediate_close() {
    // Close sender immediately without sending any packets
    let keylog = Arc::new(TlsKeyLog::new());
    let pipeline = StreamingPipeline::new(32, 2, PathBuf::from("/test.pcap"), keylog);

    let (tx, handle) = pipeline.start();

    drop(tx); // Close immediately

    let events = handle.recv_all();
    assert_eq!(events.len(), 0);

    let stats = handle.stats();
    assert_eq!(stats.packets_received, 0);
    assert_eq!(stats.events_emitted, 0);
}

#[test]
fn test_streaming_varying_shard_counts() {
    // Test with different shard counts
    for num_shards in [1, 2, 4, 8, 16] {
        let keylog = Arc::new(TlsKeyLog::new());
        let pipeline = StreamingPipeline::new(32, num_shards, PathBuf::from("/test.pcap"), keylog);

        let (tx, handle) = pipeline.start();

        for i in 0..100 {
            let pkt = create_synthetic_packet(1000 + i, b"shard test");
            tx.send(pkt).unwrap();
        }

        drop(tx);

        let events = handle.recv_all();
        assert_eq!(
            events.len(),
            100,
            "Shard count {num_shards} should produce 100 events"
        );
        assert_eq!(handle.num_shards(), num_shards);
    }
}

#[test]
fn test_streaming_recv_one_by_one() {
    // Test receiving events one by one instead of recv_all()
    let keylog = Arc::new(TlsKeyLog::new());
    let pipeline = StreamingPipeline::new(16, 2, PathBuf::from("/test.pcap"), keylog);

    let (tx, handle) = pipeline.start();

    thread::spawn(move || {
        for i in 0..10 {
            let pkt = create_synthetic_packet(1000 + i, b"one by one");
            tx.send(pkt).unwrap();
        }
    });

    let mut count = 0;
    while let Some(_event) = handle.recv() {
        count += 1;
    }

    assert_eq!(count, 10);
}

#[test]
fn test_streaming_large_payloads() {
    // Test with large packet payloads
    let keylog = Arc::new(TlsKeyLog::new());
    let pipeline = StreamingPipeline::new(
        8, // Small batch to test memory pressure
        2,
        PathBuf::from("/test.pcap"),
        keylog,
    );

    let (tx, handle) = pipeline.start();

    // Send packets with 10KB payloads
    let large_payload = vec![0x42u8; 10240];
    for i in 0..50 {
        let pkt = create_synthetic_packet(1000 + i, &large_payload);
        tx.send(pkt).unwrap();
    }

    drop(tx);

    let events = handle.recv_all();
    assert_eq!(events.len(), 50);
}

#[test]
fn test_streaming_interleaved_flows() {
    // Test with packets from multiple interleaved flows
    let keylog = Arc::new(TlsKeyLog::new());
    let pipeline = StreamingPipeline::new(32, 4, PathBuf::from("/test.pcap"), keylog);

    let (tx, handle) = pipeline.start();

    // Create 5 flows, interleaving packets
    for round in 0..20 {
        for flow in 0..5 {
            let pkt =
                create_synthetic_packet(1000 + round * 5 + flow, format!("flow{flow}").as_bytes());
            tx.send(pkt).unwrap();
        }
    }

    drop(tx);

    let events = handle.recv_all();
    assert_eq!(events.len(), 100); // 20 rounds * 5 flows
}

#[test]
fn test_streaming_stats_snapshot_consistency() {
    let keylog = Arc::new(TlsKeyLog::new());
    let pipeline = StreamingPipeline::new(16, 2, PathBuf::from("/test.pcap"), keylog);

    let (tx, handle) = pipeline.start();

    for i in 0..30 {
        let pkt = create_synthetic_packet(1000 + i, b"consistency");
        tx.send(pkt).unwrap();
    }

    drop(tx);
    let _ = handle.recv_all();

    // Take multiple snapshots
    let stats1 = handle.stats();
    let stats2 = handle.stats();

    // After pipeline completes, stats should be stable
    assert_eq!(stats1.packets_received, stats2.packets_received);
    assert_eq!(stats1.events_emitted, stats2.events_emitted);
}

#[test]
fn test_streaming_mixed_valid_invalid_packets() {
    // Mix valid and invalid (malformed) packets
    let keylog = Arc::new(TlsKeyLog::new());
    let pipeline = StreamingPipeline::new(16, 2, PathBuf::from("/test.pcap"), keylog);

    let (tx, handle) = pipeline.start();

    for i in 0..50 {
        if i % 5 == 0 {
            // Every 5th packet is malformed (too short)
            let pkt = PcapPacket {
                linktype: 1,
                timestamp_us: 1000 + i,
                data: vec![0xAA; 5], // Too short for Ethernet header
            };
            tx.send(pkt).unwrap();
        } else {
            // Valid packet
            let pkt = create_synthetic_packet(1000 + i, b"valid");
            tx.send(pkt).unwrap();
        }
    }

    drop(tx);
    let events = handle.recv_all();

    // Should produce events only for valid packets (40 out of 50)
    assert_eq!(events.len(), 40);

    let stats = handle.stats();
    assert_eq!(stats.packets_received, 50);
    assert_eq!(stats.normalize_errors, 10); // 10 malformed packets
}
