//! Pipeline orchestrator for parallel packet processing.

use crate::flow_key::FlowKey;
use crate::normalize::OwnedNormalizedPacket;
use crate::parallel::ShardProcessor;
use crate::tls::TlsKeyLog;
use prb_core::{CoreError, DebugEvent};
use std::path::PathBuf;
use std::sync::Arc;

/// Configuration for the parallel pipeline.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Number of parallel jobs (0 = auto-detect, 1 = sequential).
    pub jobs: usize,
    /// Packets per normalization batch (default: 4096).
    pub batch_size: usize,
    /// Number of flow shards (0 = auto, default: 2 * num_cpus).
    pub shard_count: usize,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            jobs: 0,
            batch_size: 4096,
            shard_count: 0,
        }
    }
}

impl PipelineConfig {
    /// Returns the effective number of jobs, auto-detecting if configured as 0.
    pub fn effective_jobs(&self) -> usize {
        if self.jobs == 0 {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        } else {
            self.jobs
        }
    }

    /// Returns the effective shard count, auto-detecting if configured as 0.
    pub fn effective_shard_count(&self) -> usize {
        if self.shard_count == 0 {
            self.effective_jobs() * 2
        } else {
            self.shard_count
        }
    }
}

/// Parallel packet processing pipeline.
///
/// Orchestrates multi-stage processing with adaptive parallelism:
/// - Phase 1: Parallel packet normalization (batch)
/// - Phase 1b: Single-threaded IP fragment reassembly (stream)
/// - Phase 2: Partition by flow key (stateless)
/// - Phase 3: Per-shard TCP reassembly + TLS + decode (stream per shard)
/// - Phase 4: Merge and sort by timestamp
pub struct ParallelPipeline {
    config: PipelineConfig,
    num_shards: usize,
    capture_path: PathBuf,
    tls_keylog: Arc<TlsKeyLog>,
}

impl ParallelPipeline {
    /// Minimum packet count to enable parallel processing.
    ///
    /// For captures smaller than this, the overhead of thread pool startup
    /// and synchronization exceeds the benefit of parallelism.
    const PARALLEL_THRESHOLD: usize = 10_000;

    /// Creates a new parallel pipeline with the given configuration.
    pub fn new(config: PipelineConfig, capture_path: PathBuf, tls_keylog: Arc<TlsKeyLog>) -> Self {
        let num_shards = if config.shard_count == 0 {
            std::thread::available_parallelism()
                .map(|n| n.get() * 2)
                .unwrap_or(8)
        } else {
            config.shard_count
        };

        Self {
            config,
            num_shards,
            capture_path,
            tls_keylog,
        }
    }

    /// Returns the number of shards used for flow partitioning.
    pub fn num_shards(&self) -> usize {
        self.num_shards
    }

    /// Returns the capture path.
    pub fn capture_path(&self) -> &PathBuf {
        &self.capture_path
    }

    /// Runs the pipeline on a batch of packets.
    ///
    /// Automatically selects parallel or sequential execution based on
    /// packet count and configuration.
    pub fn run(&self, packets: Vec<OwnedNormalizedPacket>) -> Result<Vec<DebugEvent>, CoreError> {
        if packets.len() < Self::PARALLEL_THRESHOLD || self.config.jobs == 1 {
            self.run_sequential(packets)
        } else {
            self.run_parallel(packets)
        }
    }

    /// Sequential execution path using a single shard.
    ///
    /// For small captures (< 10k packets), the overhead of parallel processing
    /// exceeds the benefit. This path uses ShardProcessor with 1 shard.
    fn run_sequential(
        &self,
        packets: Vec<OwnedNormalizedPacket>,
    ) -> Result<Vec<DebugEvent>, CoreError> {
        tracing::debug!(
            "Small capture ({} packets < {} threshold), using sequential path",
            packets.len(),
            Self::PARALLEL_THRESHOLD,
        );

        let shard_processor =
            ShardProcessor::new(Arc::clone(&self.tls_keylog), self.capture_path.clone());

        // Process all packets as a single shard
        let events = shard_processor.process_single_shard(packets);

        tracing::info!("Sequential processing complete: {} events", events.len());
        Ok(events)
    }

    /// Parallel execution path using ShardProcessor.
    fn run_parallel(
        &self,
        packets: Vec<OwnedNormalizedPacket>,
    ) -> Result<Vec<DebugEvent>, CoreError> {
        let total = packets.len();
        tracing::info!(
            "Processing {} packets with {} shards",
            total,
            self.num_shards
        );

        // Phase 2: Partition by flow
        let shards = self.partition_by_flow(packets);
        let shard_sizes: Vec<_> = shards.iter().map(|s| s.len()).collect();
        tracing::info!(
            "Partitioned into {} shards: {:?}",
            shards.len(),
            shard_sizes
        );

        // Phase 3: Process each shard in parallel
        let shard_processor =
            ShardProcessor::new(Arc::clone(&self.tls_keylog), self.capture_path.clone());
        let shard_events: Vec<Vec<DebugEvent>> = shard_processor.process_shards(shards);

        // Phase 4: Flatten and sort by timestamp
        let mut all_events: Vec<DebugEvent> = shard_events.into_iter().flatten().collect();
        all_events.sort_by_key(|e| e.timestamp);

        tracing::info!("Pipeline complete: {} events", all_events.len());
        Ok(all_events)
    }

    /// Partitions packets by flow key into shards.
    fn partition_by_flow(
        &self,
        packets: Vec<OwnedNormalizedPacket>,
    ) -> Vec<Vec<OwnedNormalizedPacket>> {
        let mut shards: Vec<Vec<OwnedNormalizedPacket>> = vec![Vec::new(); self.num_shards];

        for packet in packets {
            let shard_idx = if let Some(flow_key) = FlowKey::from_packet(&packet) {
                flow_key.shard_index(self.num_shards)
            } else {
                // Packets without TCP/UDP (e.g., ICMP) go to shard 0
                0
            };

            shards[shard_idx].push(packet);
        }

        shards
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::{TcpFlags, TcpSegmentInfo, TransportInfo};
    use std::net::{IpAddr, Ipv4Addr};

    fn make_tcp_packet(
        src_ip: IpAddr,
        src_port: u16,
        dst_ip: IpAddr,
        dst_port: u16,
    ) -> OwnedNormalizedPacket {
        OwnedNormalizedPacket {
            timestamp_us: 1000000,
            src_ip,
            dst_ip,
            transport: TransportInfo::Tcp(TcpSegmentInfo {
                src_port,
                dst_port,
                seq: 100,
                ack: 0,
                flags: TcpFlags {
                    syn: true,
                    ack: false,
                    fin: false,
                    rst: false,
                    psh: false,
                },
            }),
            vlan_id: None,
            payload: vec![],
        }
    }

    #[test]
    fn test_pipeline_config_defaults() {
        let config = PipelineConfig::default();
        assert_eq!(config.jobs, 0); // Auto-detect
        assert_eq!(config.batch_size, 4096);
        assert_eq!(config.shard_count, 0); // Auto-detect
    }

    #[test]
    fn test_parallel_threshold() {
        let config = PipelineConfig::default();
        let tls_keylog = Arc::new(TlsKeyLog::new());
        let pipeline = ParallelPipeline::new(config, PathBuf::from("/tmp/test.pcap"), tls_keylog);

        // Small capture should use sequential path
        let small_packets = vec![make_tcp_packet(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            8080,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            50051,
        )];

        let result = pipeline.run(small_packets);
        assert!(result.is_ok());
    }

    #[test]
    fn test_partition_by_flow() {
        let config = PipelineConfig {
            jobs: 4,
            batch_size: 4096,
            shard_count: 4,
        };
        let tls_keylog = Arc::new(TlsKeyLog::new());
        let pipeline = ParallelPipeline::new(config, PathBuf::from("/tmp/test.pcap"), tls_keylog);

        let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // Create packets from two different flows
        let pkt1a = make_tcp_packet(ip1, 8080, ip2, 50051);
        let pkt1b = make_tcp_packet(ip2, 50051, ip1, 8080); // Reverse direction
        let pkt2 = make_tcp_packet(ip1, 9090, ip2, 50052); // Different flow

        let packets = vec![pkt1a, pkt1b, pkt2];
        let shards = pipeline.partition_by_flow(packets);

        // All shards together should have 3 packets
        let total_packets: usize = shards.iter().map(|s| s.len()).sum();
        assert_eq!(total_packets, 3);

        // Packets from same flow (bidirectional) should be in same shard
        let flow1_key = FlowKey::from_packet(&make_tcp_packet(ip1, 8080, ip2, 50051)).unwrap();
        let flow2_key = FlowKey::from_packet(&make_tcp_packet(ip1, 9090, ip2, 50052)).unwrap();

        let shard1 = flow1_key.shard_index(4);
        let shard2 = flow2_key.shard_index(4);

        // If they map to different shards, verify packet distribution
        if shard1 != shard2 {
            assert!(shards[shard1].len() >= 2); // pkt1a and pkt1b
            assert!(!shards[shard2].is_empty()); // pkt2
        }
    }

    #[test]
    fn test_auto_shard_count() {
        let config = PipelineConfig {
            jobs: 0,
            batch_size: 4096,
            shard_count: 0, // Auto-detect
        };
        let tls_keylog = Arc::new(TlsKeyLog::new());
        let pipeline = ParallelPipeline::new(config, PathBuf::from("/tmp/test.pcap"), tls_keylog);

        let num_shards = pipeline.num_shards();
        // Should be at least 2 (fallback is 8 if detection fails)
        assert!(num_shards >= 2);
    }

    #[test]
    fn test_explicit_shard_count() {
        let config = PipelineConfig {
            jobs: 4,
            batch_size: 4096,
            shard_count: 16,
        };
        let tls_keylog = Arc::new(TlsKeyLog::new());
        let pipeline = ParallelPipeline::new(config, PathBuf::from("/tmp/test.pcap"), tls_keylog);

        assert_eq!(pipeline.num_shards(), 16);
    }
}
