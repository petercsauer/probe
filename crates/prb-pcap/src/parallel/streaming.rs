//! Streaming pipeline using crossbeam channels for bounded backpressure.
//!
//! Architecture:
//! ```text
//! Source Thread → [bounded channel] → Normalizer Thread (rayon micro-batches)
//!   → [per-shard channels] → Shard Workers (TCP reassembly + TLS + decode)
//!   → [MPSC channel] → Output Collector
//! ```

use crate::flow_key::FlowKey;
use crate::normalize::{normalize_stateless, NormalizeResult, OwnedNormalizedPacket};
use crate::parallel::stats::{AtomicPipelineStats, PipelineStats};
use crate::reader::PcapPacket;
use crate::tcp::TcpReassembler;
use crate::tls::TlsStreamProcessor;
use crossbeam_channel::{bounded, Receiver, Sender};
use prb_core::DebugEvent;
use rayon::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;

/// Streaming pipeline that processes packets as they arrive via channels.
///
/// Key features:
/// - Bounded channels for backpressure (prevents OOM on slow consumers)
/// - Micro-batching in normalizer (collect batch, rayon parallel normalize, route)
/// - Per-shard TCP reassembly (independent state, true parallelism)
/// - Thread-safe TLS decryption using Arc<TlsKeyLog>
pub struct StreamingPipeline {
    batch_size: usize,
    num_shards: usize,
    capture_path: PathBuf,
    tls_keylog: Arc<crate::tls::TlsKeyLog>,
}

impl StreamingPipeline {
    /// Creates a new streaming pipeline.
    ///
    /// # Arguments
    ///
    /// * `batch_size` - Number of packets to collect before parallel normalization
    /// * `num_shards` - Number of parallel shard workers (typically 2 * num_cpus)
    /// * `capture_path` - Path to capture file for event metadata
    /// * `tls_keylog` - Shared TLS keylog for decryption
    pub fn new(
        batch_size: usize,
        num_shards: usize,
        capture_path: PathBuf,
        tls_keylog: Arc<crate::tls::TlsKeyLog>,
    ) -> Self {
        assert!(batch_size > 0, "batch_size must be > 0");
        assert!(num_shards > 0, "num_shards must be > 0");

        Self {
            batch_size,
            num_shards,
            capture_path,
            tls_keylog,
        }
    }

    /// Starts the streaming pipeline and returns a handle for collecting results.
    ///
    /// Spawns threads:
    /// - 1 normalizer thread (with rayon thread pool)
    /// - N shard worker threads
    ///
    /// The source feeds packets into the returned sender. When done, drop the sender
    /// to signal EOF and wait for the handle to collect all results.
    ///
    /// # Returns
    ///
    /// A tuple of (packet_sender, pipeline_handle) where:
    /// - `packet_sender` - Send packets here (bounded channel with backpressure)
    /// - `pipeline_handle` - Collect events and stats from here
    pub fn start(&self) -> (Sender<PcapPacket>, PipelineHandle) {
        let stats = Arc::new(AtomicPipelineStats::new());

        // Channel capacities based on batch size to balance memory and throughput
        let source_capacity = self.batch_size * 4; // Allow buffering of 4 batches
        let shard_capacity = self.batch_size; // One batch per shard
        let output_capacity = self.batch_size * 2; // Buffer 2 batches of events

        // Create channels
        let (packet_tx, packet_rx) = bounded::<PcapPacket>(source_capacity);
        let (event_tx, event_rx) = bounded::<DebugEvent>(output_capacity);

        // Create per-shard channels
        let mut shard_senders: Vec<Sender<OwnedNormalizedPacket>> = Vec::new();
        let mut shard_receivers: Vec<Receiver<OwnedNormalizedPacket>> = Vec::new();
        for _ in 0..self.num_shards {
            let (tx, rx) = bounded(shard_capacity);
            shard_senders.push(tx);
            shard_receivers.push(rx);
        }

        // Spawn normalizer thread
        let normalizer_stats = Arc::clone(&stats);
        let normalizer_batch_size = self.batch_size;
        let normalizer_num_shards = self.num_shards;
        thread::spawn(move || {
            normalize_and_route(
                packet_rx,
                shard_senders,
                normalizer_batch_size,
                normalizer_num_shards,
                normalizer_stats,
            );
        });

        // Spawn shard worker threads
        let capture_path = self.capture_path.clone();
        let tls_keylog = Arc::clone(&self.tls_keylog);
        for shard_rx in shard_receivers {
            let shard_event_tx = event_tx.clone();
            let shard_stats = Arc::clone(&stats);
            let shard_capture_path = capture_path.clone();
            let shard_tls_keylog = Arc::clone(&tls_keylog);

            thread::spawn(move || {
                shard_worker(
                    shard_rx,
                    shard_event_tx,
                    shard_capture_path,
                    shard_tls_keylog,
                    shard_stats,
                );
            });
        }

        // Return handle
        let handle = PipelineHandle {
            event_rx,
            stats,
            num_shards: self.num_shards,
        };

        (packet_tx, handle)
    }
}

/// Handle for collecting events and statistics from a running pipeline.
pub struct PipelineHandle {
    event_rx: Receiver<DebugEvent>,
    stats: Arc<AtomicPipelineStats>,
    num_shards: usize,
}

impl PipelineHandle {
    /// Receives the next event from the pipeline, blocking until available.
    ///
    /// Returns `None` when all shard workers have finished and the channel is empty.
    pub fn recv(&self) -> Option<DebugEvent> {
        self.event_rx.recv().ok()
    }

    /// Receives all remaining events, blocking until pipeline completes.
    ///
    /// This consumes all events from the channel until all shard workers exit.
    pub fn recv_all(&self) -> Vec<DebugEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.recv() {
            events.push(event);
        }
        events
    }

    /// Returns a snapshot of current pipeline statistics.
    pub fn stats(&self) -> PipelineStats {
        self.stats.snapshot()
    }

    /// Returns the number of shards in this pipeline.
    pub fn num_shards(&self) -> usize {
        self.num_shards
    }
}

/// Normalizer thread function: receives packets, micro-batches, parallel normalizes, routes to shards.
fn normalize_and_route(
    packet_rx: Receiver<PcapPacket>,
    shard_txs: Vec<Sender<OwnedNormalizedPacket>>,
    batch_size: usize,
    num_shards: usize,
    stats: Arc<AtomicPipelineStats>,
) {
    let mut batch = Vec::with_capacity(batch_size);

    loop {
        // Collect up to batch_size packets
        batch.clear();
        for _ in 0..batch_size {
            match packet_rx.recv() {
                Ok(pkt) => {
                    stats.packets_received.fetch_add(1, Ordering::Relaxed);
                    batch.push(pkt);
                }
                Err(_) => {
                    // Channel closed - process final partial batch and exit
                    break;
                }
            }
        }

        if batch.is_empty() {
            // Channel closed and no packets left
            break;
        }

        // Parallel normalize using rayon
        let results: Vec<(usize, Result<NormalizeResult, crate::error::PcapError>)> = batch
            .par_iter()
            .enumerate()
            .map(|(idx, pkt)| {
                (
                    idx,
                    normalize_stateless(pkt.linktype, pkt.timestamp_us, &pkt.data),
                )
            })
            .collect();

        // Route normalized packets to shards
        for (_idx, result) in results {
            match result {
                Ok(NormalizeResult::Packet(pkt)) => {
                    stats.packets_normalized.fetch_add(1, Ordering::Relaxed);

                    // Determine shard index
                    let shard_idx = FlowKey::from_packet(&pkt)
                        .map(|k| k.shard_index(num_shards))
                        .unwrap_or(0);

                    // Send to shard (blocks on backpressure)
                    if shard_txs[shard_idx].send(pkt).is_ok() {
                        stats.packets_routed.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Ok(NormalizeResult::Fragment { .. }) => {
                    // Drop fragments in streaming mode (acceptable trade-off)
                    stats.fragments.fetch_add(1, Ordering::Relaxed);
                }
                Err(_) => {
                    stats.normalize_errors.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }

    // Drop shard senders to signal EOF to workers
    drop(shard_txs);
}

/// Shard worker thread function: receives normalized packets, reassembles TCP, decrypts TLS, emits events.
fn shard_worker(
    packet_rx: Receiver<OwnedNormalizedPacket>,
    event_tx: Sender<DebugEvent>,
    capture_path: PathBuf,
    tls_keylog: Arc<crate::tls::TlsKeyLog>,
    stats: Arc<AtomicPipelineStats>,
) {
    use crate::normalize::TransportInfo;
    use crate::tcp::StreamEvent;

    let mut reassembler = TcpReassembler::new();
    let tls_processor = TlsStreamProcessor::with_keylog_ref(tls_keylog);

    // Process packets as they arrive
    while let Ok(packet) = packet_rx.recv() {
        match &packet.transport {
            TransportInfo::Tcp(_) => {
                match reassembler.process_owned_segment(&packet) {
                    Ok(stream_events) => {
                        for stream_event in stream_events {
                            if let StreamEvent::Data(stream) = stream_event {
                                match tls_processor.decrypt_stream(stream.clone()) {
                                    Ok(decrypted_stream) => {
                                        let event = create_tcp_event(stream, decrypted_stream, &capture_path);
                                        stats.events_emitted.fetch_add(1, Ordering::Relaxed);
                                        if event_tx.send(event).is_err() {
                                            return;
                                        }
                                    }
                                    Err(_) => {
                                        let event = create_tcp_event_encrypted(stream, &capture_path);
                                        stats.events_emitted.fetch_add(1, Ordering::Relaxed);
                                        if event_tx.send(event).is_err() {
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => tracing::warn!("TCP reassembly error: {}", e),
                }
            }
            TransportInfo::Udp { src_port, dst_port } => {
                let event = create_udp_event(&packet, *src_port, *dst_port, &capture_path);
                stats.events_emitted.fetch_add(1, Ordering::Relaxed);
                if event_tx.send(event).is_err() {
                    return;
                }
            }
            TransportInfo::Other(_) => {
                // Ignore other protocols (ICMP, etc.)
            }
        }
    }

    // Flush remaining TCP connections
    for stream_event in reassembler.flush_all() {
        if let StreamEvent::Data(stream) = stream_event {
            match tls_processor.decrypt_stream(stream.clone()) {
                Ok(decrypted_stream) => {
                    let event = create_tcp_event(stream, decrypted_stream, &capture_path);
                    stats.events_emitted.fetch_add(1, Ordering::Relaxed);
                    let _ = event_tx.send(event);
                }
                Err(_) => {
                    let event = create_tcp_event_encrypted(stream, &capture_path);
                    stats.events_emitted.fetch_add(1, Ordering::Relaxed);
                    let _ = event_tx.send(event);
                }
            }
        }
    }
}

/// Helper functions (reused from shard.rs)
use crate::tcp::{ReassembledStream, StreamDirection};
use crate::tls::DecryptedStream;
use bytes::Bytes;
use prb_core::{Direction, EventSource, NetworkAddr, Payload, Timestamp, TransportKind};

fn create_udp_event(
    packet: &OwnedNormalizedPacket,
    src_port: u16,
    dst_port: u16,
    capture_path: &std::path::Path,
) -> DebugEvent {
    let timestamp = Timestamp::from_nanos(packet.timestamp_us * 1000);
    let source = EventSource {
        adapter: "pcap".to_string(),
        origin: capture_path.to_string_lossy().to_string(),
        network: Some(NetworkAddr {
            src: format!("{}:{}", packet.src_ip, src_port),
            dst: format!("{}:{}", packet.dst_ip, dst_port),
        }),
    };
    let payload = Payload::Raw {
        raw: Bytes::copy_from_slice(&packet.payload),
    };

    DebugEvent::builder()
        .timestamp(timestamp)
        .source(source)
        .transport(TransportKind::RawUdp)
        .direction(Direction::Unknown)
        .payload(payload)
        .build()
}

fn create_tcp_event(
    stream: ReassembledStream,
    decrypted: DecryptedStream,
    capture_path: &std::path::Path,
) -> DebugEvent {
    let timestamp = Timestamp::from_nanos(stream.timestamp_us * 1000);
    let source = EventSource {
        adapter: "pcap".to_string(),
        origin: capture_path.to_string_lossy().to_string(),
        network: Some(NetworkAddr {
            src: format!("{}:{}", stream.src_ip, stream.src_port),
            dst: format!("{}:{}", stream.dst_ip, stream.dst_port),
        }),
    };
    let direction = match stream.direction {
        StreamDirection::ClientToServer => Direction::Outbound,
        StreamDirection::ServerToClient => Direction::Inbound,
    };
    let payload_data = if decrypted.encrypted {
        &stream.data
    } else {
        &decrypted.data
    };
    let payload = Payload::Raw {
        raw: Bytes::copy_from_slice(payload_data),
    };

    let mut builder = DebugEvent::builder()
        .timestamp(timestamp)
        .source(source)
        .transport(TransportKind::RawTcp)
        .direction(direction)
        .payload(payload)
        .metadata("tcp.complete", stream.is_complete.to_string())
        .metadata("tls.encrypted", decrypted.encrypted.to_string());

    if !stream.is_complete {
        builder = builder.warning("TCP stream incomplete (no FIN/RST)");
    }

    builder.build()
}

fn create_tcp_event_encrypted(
    stream: ReassembledStream,
    capture_path: &std::path::Path,
) -> DebugEvent {
    let timestamp = Timestamp::from_nanos(stream.timestamp_us * 1000);
    let source = EventSource {
        adapter: "pcap".to_string(),
        origin: capture_path.to_string_lossy().to_string(),
        network: Some(NetworkAddr {
            src: format!("{}:{}", stream.src_ip, stream.src_port),
            dst: format!("{}:{}", stream.dst_ip, stream.dst_port),
        }),
    };
    let direction = match stream.direction {
        StreamDirection::ClientToServer => Direction::Outbound,
        StreamDirection::ServerToClient => Direction::Inbound,
    };
    let payload = Payload::Raw {
        raw: Bytes::copy_from_slice(&stream.data),
    };

    let mut builder = DebugEvent::builder()
        .timestamp(timestamp)
        .source(source)
        .transport(TransportKind::RawTcp)
        .direction(direction)
        .payload(payload)
        .metadata("tcp.complete", stream.is_complete.to_string())
        .metadata("tls.encrypted", "true");

    if !stream.is_complete {
        builder = builder.warning("TCP stream incomplete (no FIN/RST)");
    }

    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use etherparse::PacketBuilder;

    fn create_pcap_tcp_packet(
        timestamp_us: u64,
        src_ip: [u8; 4],
        dst_ip: [u8; 4],
        src_port: u16,
        dst_port: u16,
        payload: &[u8],
    ) -> PcapPacket {
        let builder = PacketBuilder::ethernet2(
            [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
            [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        )
        .ipv4(src_ip, dst_ip, 64)
        .tcp(src_port, dst_port, 1000, 4096);

        let mut data = Vec::new();
        builder.write(&mut data, payload).unwrap();

        PcapPacket {
            linktype: 1, // Ethernet
            timestamp_us,
            data,
        }
    }

    #[test]
    fn test_streaming_pipeline_basic() {
        let keylog = Arc::new(crate::tls::TlsKeyLog::new());
        let pipeline = StreamingPipeline::new(
            32,
            2,
            PathBuf::from("/test.pcap"),
            keylog,
        );

        let (tx, handle) = pipeline.start();

        // Send 100 synthetic packets
        for i in 0..100 {
            let pkt = create_pcap_tcp_packet(
                1000 + i,
                [192, 168, 1, 1],
                [10, 0, 0, 1],
                12345,
                80,
                format!("packet{}", i).as_bytes(),
            );
            tx.send(pkt).unwrap();
        }

        // Drop sender to signal EOF
        drop(tx);

        // Collect all events
        let events = handle.recv_all();

        // Should have received some events (TCP reassembly may combine packets)
        assert!(!events.is_empty(), "Should receive at least some events");

        let stats = handle.stats();
        assert_eq!(stats.packets_received, 100);
        assert!(stats.packets_normalized > 0);
    }

    #[test]
    fn test_streaming_pipeline_empty() {
        let keylog = Arc::new(crate::tls::TlsKeyLog::new());
        let pipeline = StreamingPipeline::new(
            32,
            2,
            PathBuf::from("/test.pcap"),
            keylog,
        );

        let (tx, handle) = pipeline.start();

        // Drop sender immediately (no packets)
        drop(tx);

        // Collect all events
        let events = handle.recv_all();

        assert_eq!(events.len(), 0, "Empty input should produce no events");

        let stats = handle.stats();
        assert_eq!(stats.packets_received, 0);
        assert_eq!(stats.events_emitted, 0);
    }

    #[test]
    fn test_streaming_pipeline_backpressure() {
        let keylog = Arc::new(crate::tls::TlsKeyLog::new());
        let pipeline = StreamingPipeline::new(
            8,  // Small batch size
            2,
            PathBuf::from("/test.pcap"),
            keylog,
        );

        let (tx, handle) = pipeline.start();

        // Send many packets quickly (should block due to backpressure)
        for i in 0..1000 {
            let pkt = create_pcap_tcp_packet(
                1000 + i,
                [192, 168, 1, 1],
                [10, 0, 0, 1],
                12345,
                80,
                b"test",
            );
            tx.send(pkt).unwrap();
        }

        drop(tx);

        // Should complete without OOM
        let events = handle.recv_all();
        assert!(!events.is_empty());

        let stats = handle.stats();
        assert_eq!(stats.packets_received, 1000);
    }

    #[test]
    fn test_streaming_stats_accuracy() {
        let keylog = Arc::new(crate::tls::TlsKeyLog::new());
        let pipeline = StreamingPipeline::new(
            16,
            2,
            PathBuf::from("/test.pcap"),
            keylog,
        );

        let (tx, handle) = pipeline.start();

        // Send 50 packets
        for i in 0..50 {
            let pkt = create_pcap_tcp_packet(
                1000 + i,
                [192, 168, 1, 1],
                [10, 0, 0, 1],
                12345,
                80,
                b"test",
            );
            tx.send(pkt).unwrap();
        }

        drop(tx);
        let _ = handle.recv_all();

        let stats = handle.stats();
        assert_eq!(stats.packets_received, 50);
        assert_eq!(stats.packets_normalized, 50);
        assert_eq!(stats.packets_routed, 50);
        assert_eq!(stats.fragments, 0);
    }

    #[test]
    fn test_streaming_graceful_shutdown() {
        let keylog = Arc::new(crate::tls::TlsKeyLog::new());
        let pipeline = StreamingPipeline::new(
            32,
            4,
            PathBuf::from("/test.pcap"),
            keylog,
        );

        let (tx, handle) = pipeline.start();

        // Send some packets
        for i in 0..10 {
            let pkt = create_pcap_tcp_packet(
                1000 + i,
                [192, 168, 1, 1],
                [10, 0, 0, 1],
                12345,
                80,
                b"test",
            );
            tx.send(pkt).unwrap();
        }

        // Drop sender
        drop(tx);

        // Should complete cleanly
        let events = handle.recv_all();
        assert!(!events.is_empty());
    }

    #[test]
    fn test_streaming_matches_batch() {
        // This test compares streaming vs batch processing
        // For now, just verify streaming produces events
        let keylog = Arc::new(crate::tls::TlsKeyLog::new());
        let pipeline = StreamingPipeline::new(
            16,
            2,
            PathBuf::from("/test.pcap"),
            Arc::clone(&keylog),
        );

        let (tx, handle) = pipeline.start();

        // Create deterministic test packets
        for i in 0..20 {
            let pkt = create_pcap_tcp_packet(
                1000 + i * 100,
                [192, 168, 1, 1],
                [10, 0, 0, 1],
                12345,
                80,
                format!("packet{}", i).as_bytes(),
            );
            tx.send(pkt).unwrap();
        }

        drop(tx);
        let streaming_events = handle.recv_all();

        // For batch processing, we'd use the existing ParallelPipeline
        // For now, just verify we got events
        assert!(!streaming_events.is_empty());
    }

    #[test]
    fn test_streaming_fragment_counting() {
        // Test that fragments are counted in stats
        // For now, we'll just verify the stats counter exists
        let keylog = Arc::new(crate::tls::TlsKeyLog::new());
        let pipeline = StreamingPipeline::new(
            16,
            2,
            PathBuf::from("/test.pcap"),
            keylog,
        );

        let (tx, handle) = pipeline.start();

        // Send some normal packets
        for i in 0..10 {
            let pkt = create_pcap_tcp_packet(
                1000 + i,
                [192, 168, 1, 1],
                [10, 0, 0, 1],
                12345,
                80,
                b"test",
            );
            tx.send(pkt).unwrap();
        }

        drop(tx);
        let _ = handle.recv_all();

        let stats = handle.stats();
        // Normal packets should have 0 fragments
        assert_eq!(stats.fragments, 0);
    }
}
