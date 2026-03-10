//! Pipeline orchestrator for parallel packet processing.

use crate::flow_key::FlowKey;
use crate::normalize::OwnedNormalizedPacket;
use prb_core::{CoreError, DebugEvent};
use std::path::PathBuf;

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
}

impl ParallelPipeline {
    /// Minimum packet count to enable parallel processing.
    ///
    /// For captures smaller than this, the overhead of thread pool startup
    /// and synchronization exceeds the benefit of parallelism.
    const PARALLEL_THRESHOLD: usize = 10_000;

    /// Creates a new parallel pipeline with the given configuration.
    pub fn new(config: PipelineConfig, capture_path: PathBuf) -> Self {
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

    /// Sequential execution path (placeholder).
    ///
    /// This will be fully implemented in later segments when we integrate
    /// with the existing pipeline code.
    fn run_sequential(
        &self,
        _packets: Vec<OwnedNormalizedPacket>,
    ) -> Result<Vec<DebugEvent>, CoreError> {
        // TODO: Call existing PcapCaptureAdapter::process_all_packets
        Ok(vec![])
    }

    /// Parallel execution path (placeholder).
    ///
    /// This will be implemented when we refactor the existing stages
    /// to implement BatchStage/StreamStage traits.
    fn run_parallel(
        &self,
        packets: Vec<OwnedNormalizedPacket>,
    ) -> Result<Vec<DebugEvent>, CoreError> {
        // Phase 2: Partition by flow
        let shards = self.partition_by_flow(packets);

        // Phase 3: Process each shard (placeholder for now)
        // In future segments, this will use rayon::par_iter
        let mut all_events = Vec::new();
        for shard in shards {
            let shard_events = self.process_shard(shard)?;
            all_events.extend(shard_events);
        }

        // Phase 4: Sort by timestamp
        all_events.sort_by_key(|e| e.timestamp);

        Ok(all_events)
    }

    /// Partitions packets by flow key into shards.
    fn partition_by_flow(&self, packets: Vec<OwnedNormalizedPacket>) -> Vec<Vec<OwnedNormalizedPacket>> {
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

    /// Processes a single shard (placeholder).
    ///
    /// This will be implemented when TCP reassembly, TLS decryption,
    /// and protocol decoding are refactored into stage implementations.
    fn process_shard(
        &self,
        _shard: Vec<OwnedNormalizedPacket>,
    ) -> Result<Vec<DebugEvent>, CoreError> {
        // TODO: TCP reassembly -> TLS decrypt -> protocol decode
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::{TcpSegmentInfo, TcpFlags, TransportInfo};
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
        let pipeline = ParallelPipeline::new(config, PathBuf::from("/tmp/test.pcap"));

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
        let pipeline = ParallelPipeline::new(config, PathBuf::from("/tmp/test.pcap"));

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
            assert!(shards[shard2].len() >= 1); // pkt2
        }
    }

    #[test]
    fn test_auto_shard_count() {
        let config = PipelineConfig {
            jobs: 0,
            batch_size: 4096,
            shard_count: 0, // Auto-detect
        };
        let pipeline = ParallelPipeline::new(config, PathBuf::from("/tmp/test.pcap"));

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
        let pipeline = ParallelPipeline::new(config, PathBuf::from("/tmp/test.pcap"));

        assert_eq!(pipeline.num_shards(), 16);
    }
}
