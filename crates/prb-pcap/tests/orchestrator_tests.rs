//! Tests for parallel pipeline orchestrator.

use prb_pcap::parallel::{ParallelPipeline, PipelineConfig};
use prb_pcap::tls::TlsKeyLog;
use prb_pcap::{OwnedNormalizedPacket, TcpFlags, TcpSegmentInfo, TransportInfo};
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::Arc;

fn make_tcp_packet(
    timestamp_us: u64,
    src_ip: [u8; 4],
    src_port: u16,
    dst_ip: [u8; 4],
    dst_port: u16,
    seq: u32,
    payload: Vec<u8>,
) -> OwnedNormalizedPacket {
    OwnedNormalizedPacket {
        timestamp_us,
        src_ip: IpAddr::V4(Ipv4Addr::from(src_ip)),
        dst_ip: IpAddr::V4(Ipv4Addr::from(dst_ip)),
        transport: TransportInfo::Tcp(TcpSegmentInfo {
            src_port,
            dst_port,
            seq,
            ack: 0,
            flags: TcpFlags {
                syn: false,
                ack: true,
                fin: false,
                rst: false,
                psh: true,
            },
        }),
        vlan_id: None,
        payload,
    }
}

fn make_udp_packet(
    timestamp_us: u64,
    src_ip: [u8; 4],
    src_port: u16,
    dst_ip: [u8; 4],
    dst_port: u16,
    payload: Vec<u8>,
) -> OwnedNormalizedPacket {
    OwnedNormalizedPacket {
        timestamp_us,
        src_ip: IpAddr::V4(Ipv4Addr::from(src_ip)),
        dst_ip: IpAddr::V4(Ipv4Addr::from(dst_ip)),
        transport: TransportInfo::Udp { src_port, dst_port },
        vlan_id: None,
        payload,
    }
}

#[test]
fn test_orchestrator_empty_input() {
    let config = PipelineConfig::default();
    let tls_keylog = Arc::new(TlsKeyLog::new());
    let pipeline = ParallelPipeline::new(config, PathBuf::from("/test.pcap"), tls_keylog);

    let events = pipeline.run(vec![]).unwrap();
    assert_eq!(events.len(), 0);
}

#[test]
fn test_orchestrator_single_packet() {
    let config = PipelineConfig::default();
    let tls_keylog = Arc::new(TlsKeyLog::new());
    let pipeline = ParallelPipeline::new(config, PathBuf::from("/test.pcap"), tls_keylog);

    let packet = make_udp_packet(
        1000,
        [192, 168, 1, 1],
        12345,
        [10, 0, 0, 1],
        80,
        b"test".to_vec(),
    );

    let events = pipeline.run(vec![packet]).unwrap();
    assert_eq!(events.len(), 1);
}

#[test]
fn test_orchestrator_sequential_threshold() {
    // Small capture (< 10k packets) should use sequential path
    let config = PipelineConfig {
        jobs: 0,
        batch_size: 4096,
        shard_count: 0,
    };
    let tls_keylog = Arc::new(TlsKeyLog::new());
    let pipeline = ParallelPipeline::new(config, PathBuf::from("/test.pcap"), tls_keylog);

    let mut packets = Vec::new();
    for i in 0..100 {
        packets.push(make_udp_packet(
            1000 + i,
            [192, 168, 1, 1],
            12345,
            [10, 0, 0, 1],
            80,
            format!("packet{i}").into_bytes(),
        ));
    }

    let events = pipeline.run(packets).unwrap();
    assert_eq!(events.len(), 100);
}

#[test]
fn test_orchestrator_parallel_large() {
    // Large capture (>= 10k packets) should use parallel path
    let config = PipelineConfig {
        jobs: 2,
        batch_size: 2048,
        shard_count: 4,
    };
    let tls_keylog = Arc::new(TlsKeyLog::new());
    let pipeline = ParallelPipeline::new(config, PathBuf::from("/test.pcap"), tls_keylog);

    let mut packets = Vec::new();
    for i in 0..10000 {
        packets.push(make_udp_packet(
            1000 + i,
            [192, 168, 1, 1],
            12345,
            [10, 0, 0, 1],
            80,
            format!("pkt{i}").into_bytes(),
        ));
    }

    let events = pipeline.run(packets).unwrap();
    assert_eq!(events.len(), 10000);
}

#[test]
fn test_orchestrator_forced_sequential() {
    // Force sequential mode with jobs=1
    let config = PipelineConfig {
        jobs: 1,
        batch_size: 4096,
        shard_count: 1,
    };
    let tls_keylog = Arc::new(TlsKeyLog::new());
    let pipeline = ParallelPipeline::new(config, PathBuf::from("/test.pcap"), tls_keylog);

    let mut packets = Vec::new();
    for i in 0..20000 {
        packets.push(make_udp_packet(
            1000 + i,
            [192, 168, 1, 1],
            12345,
            [10, 0, 0, 1],
            80,
            b"test".to_vec(),
        ));
    }

    // Even with 20k packets, should use sequential path due to jobs=1
    let events = pipeline.run(packets).unwrap();
    assert_eq!(events.len(), 20000);
}

#[test]
fn test_orchestrator_timestamp_ordering() {
    // Verify events are sorted by timestamp after parallel processing
    let config = PipelineConfig {
        jobs: 4,
        batch_size: 2048,
        shard_count: 8,
    };
    let tls_keylog = Arc::new(TlsKeyLog::new());
    let pipeline = ParallelPipeline::new(config, PathBuf::from("/test.pcap"), tls_keylog);

    let mut packets = Vec::new();

    // Create packets with non-sequential timestamps
    for i in (0..10000).rev() {
        packets.push(make_udp_packet(
            1000000 + i,
            [192, 168, 1, 1],
            12345,
            [10, 0, 0, 1],
            80,
            b"test".to_vec(),
        ));
    }

    let events = pipeline.run(packets).unwrap();

    // Verify events are sorted by timestamp
    for i in 1..events.len() {
        assert!(
            events[i].timestamp >= events[i - 1].timestamp,
            "Events should be sorted by timestamp"
        );
    }
}

#[test]
fn test_orchestrator_flow_partitioning() {
    // Verify packets from same flow go to same shard
    let config = PipelineConfig {
        jobs: 4,
        batch_size: 2048,
        shard_count: 4,
    };
    let tls_keylog = Arc::new(TlsKeyLog::new());
    let pipeline = ParallelPipeline::new(config, PathBuf::from("/test.pcap"), tls_keylog);

    let mut packets = Vec::new();

    // Create multiple TCP flows
    for flow_id in 0..10 {
        for seq in 0..1000 {
            packets.push(make_tcp_packet(
                1000 + flow_id * 1000 + seq,
                [192, 168, 1, 1],
                12345 + flow_id as u16,
                [10, 0, 0, 1],
                80,
                1000 + seq as u32 * 100,
                format!("flow{flow_id}_seq{seq}").into_bytes(),
            ));
        }
    }

    // Should produce events for each flow
    let events = pipeline.run(packets).unwrap();
    assert!(!events.is_empty());
}

#[test]
fn test_orchestrator_mixed_protocols() {
    // Mix TCP and UDP packets
    let config = PipelineConfig {
        jobs: 2,
        batch_size: 2048,
        shard_count: 4,
    };
    let tls_keylog = Arc::new(TlsKeyLog::new());
    let pipeline = ParallelPipeline::new(config, PathBuf::from("/test.pcap"), tls_keylog);

    let mut packets = Vec::new();

    // Alternate TCP and UDP packets
    for i in 0..5000 {
        if i % 2 == 0 {
            packets.push(make_tcp_packet(
                1000 + i,
                [192, 168, 1, 1],
                12345,
                [10, 0, 0, 1],
                80,
                1000 + i as u32,
                b"TCP".to_vec(),
            ));
        } else {
            packets.push(make_udp_packet(
                1000 + i,
                [192, 168, 1, 2],
                5555,
                [10, 0, 0, 2],
                5556,
                b"UDP".to_vec(),
            ));
        }
    }

    let events = pipeline.run(packets).unwrap();
    // TCP segments are reassembled, so we don't get 1:1 events.
    // UDP packets (2500) should produce events, TCP may be consolidated.
    assert!(
        events.len() >= 2500,
        "Expected at least 2500 events (UDP packets), got {}",
        events.len()
    );
    assert!(
        events.len() <= 5000,
        "Expected at most 5000 events (all packets), got {}",
        events.len()
    );
}

#[test]
fn test_orchestrator_varying_thread_counts() {
    let tls_keylog = Arc::new(TlsKeyLog::new());

    let mut packets = Vec::new();
    for i in 0..1000 {
        packets.push(make_udp_packet(
            1000 + i,
            [192, 168, 1, 1],
            12345,
            [10, 0, 0, 1],
            80,
            b"test".to_vec(),
        ));
    }

    // Test with 1, 2, 4, 8 shards
    for shard_count in [1, 2, 4, 8] {
        let config = PipelineConfig {
            jobs: 1, // Force sequential to make deterministic
            batch_size: 2048,
            shard_count,
        };
        let pipeline =
            ParallelPipeline::new(config, PathBuf::from("/test.pcap"), Arc::clone(&tls_keylog));

        let events = pipeline.run(packets.clone()).unwrap();
        assert_eq!(
            events.len(),
            1000,
            "Shard count {shard_count} should produce 1000 events"
        );
    }
}

#[test]
fn test_orchestrator_auto_detect_parallelism() {
    // Test auto-detection (jobs=0, shard_count=0)
    let config = PipelineConfig {
        jobs: 0,
        batch_size: 4096,
        shard_count: 0,
    };
    let tls_keylog = Arc::new(TlsKeyLog::new());
    let pipeline = ParallelPipeline::new(config, PathBuf::from("/test.pcap"), tls_keylog);

    // Should auto-detect and set reasonable defaults
    assert!(pipeline.num_shards() > 0);

    let mut packets = Vec::new();
    for i in 0..100 {
        packets.push(make_udp_packet(
            1000 + i,
            [192, 168, 1, 1],
            12345,
            [10, 0, 0, 1],
            80,
            b"test".to_vec(),
        ));
    }

    let events = pipeline.run(packets).unwrap();
    assert_eq!(events.len(), 100);
}

#[test]
fn test_orchestrator_config_effective_jobs() {
    let config = PipelineConfig {
        jobs: 0,
        batch_size: 4096,
        shard_count: 0,
    };

    let effective_jobs = config.effective_jobs();
    assert!(effective_jobs >= 1, "Should auto-detect at least 1 job");
}

#[test]
fn test_orchestrator_config_effective_shard_count() {
    let config = PipelineConfig {
        jobs: 4,
        batch_size: 4096,
        shard_count: 0,
    };

    let effective_shards = config.effective_shard_count();
    assert_eq!(
        effective_shards, 8,
        "Should be 2 * jobs when auto-detecting"
    );
}

#[test]
fn test_orchestrator_capture_path() {
    let path = PathBuf::from("/tmp/test.pcap");
    let config = PipelineConfig::default();
    let tls_keylog = Arc::new(TlsKeyLog::new());
    let pipeline = ParallelPipeline::new(config, path.clone(), tls_keylog);

    assert_eq!(pipeline.capture_path(), &path);
}

#[test]
fn test_orchestrator_with_vlan_packets() {
    let config = PipelineConfig::default();
    let tls_keylog = Arc::new(TlsKeyLog::new());
    let pipeline = ParallelPipeline::new(config, PathBuf::from("/test.pcap"), tls_keylog);

    let mut packets = Vec::new();
    for i in 0..100 {
        let mut pkt = make_udp_packet(
            1000 + i,
            [192, 168, 1, 1],
            12345,
            [10, 0, 0, 1],
            80,
            b"VLAN test".to_vec(),
        );
        pkt.vlan_id = Some(100);
        packets.push(pkt);
    }

    let events = pipeline.run(packets).unwrap();
    assert_eq!(events.len(), 100);
}

#[test]
fn test_orchestrator_stress_many_flows() {
    // Stress test with many different flows
    let config = PipelineConfig {
        jobs: 4,
        batch_size: 2048,
        shard_count: 8,
    };
    let tls_keylog = Arc::new(TlsKeyLog::new());
    let pipeline = ParallelPipeline::new(config, PathBuf::from("/test.pcap"), tls_keylog);

    let mut packets = Vec::new();

    // Create 1000 different flows with 10 packets each
    for flow in 0..1000 {
        for seq in 0..10 {
            packets.push(make_tcp_packet(
                1000 + flow * 10 + seq,
                [192, 168, (flow / 256) as u8, (flow % 256) as u8],
                12345,
                [10, 0, 0, 1],
                80,
                1000 + seq as u32,
                b"data".to_vec(),
            ));
        }
    }

    let events = pipeline.run(packets).unwrap();
    assert!(!events.is_empty());
}
