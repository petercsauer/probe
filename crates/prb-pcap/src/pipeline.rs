//! Pipeline integration: wires PCAP reader → normalizer → TCP reassembly → TLS decryption → DebugEvents.
//!
//! This module implements the `CaptureAdapter` trait for PCAP/pcapng files,
//! orchestrating the complete data flow from raw packet capture to structured debug events.

use crate::normalize::{NormalizedPacket, PacketNormalizer, TransportInfo};
use crate::reader::PcapFileReader;
use crate::tcp::{ReassembledStream, StreamEvent, TcpReassembler};
use crate::tls::{DecryptedStream, TlsKeyLog, TlsStreamProcessor};
use bytes::Bytes;
use prb_core::{
    CaptureAdapter, CoreError, DebugEvent, Direction, EventSource, NetworkAddr, Payload,
    Timestamp, TransportKind,
};
use std::collections::VecDeque;
use std::net::IpAddr;
use std::path::PathBuf;

/// Statistics for the pipeline processing.
#[derive(Debug, Default, Clone)]
pub struct PipelineStats {
    /// Total packets read from capture file.
    pub packets_read: u64,
    /// Packets that failed to normalize (parse errors).
    pub packets_failed: u64,
    /// TCP streams reassembled.
    pub tcp_streams: u64,
    /// UDP datagrams processed.
    pub udp_datagrams: u64,
    /// TLS streams decrypted successfully.
    pub tls_decrypted: u64,
    /// TLS streams that remained encrypted (no keys or decryption failed).
    pub tls_encrypted: u64,
}

/// PCAP capture adapter implementing the `CaptureAdapter` trait.
///
/// This adapter processes PCAP/pcapng files through a multi-stage pipeline:
/// 1. Read packets with `PcapFileReader`
/// 2. Normalize packets with `PacketNormalizer` (handle link types, IP defrag)
/// 3. Reassemble TCP streams with `TcpReassembler`
/// 4. Decrypt TLS streams with `TlsStreamProcessor`
/// 5. Convert to `DebugEvent` format
///
/// UDP datagrams bypass reassembly and TLS processing, converting directly to events.
pub struct PcapCaptureAdapter {
    /// Path to the PCAP/pcapng file.
    capture_path: PathBuf,
    /// Optional path to TLS keylog file (SSLKEYLOGFILE format).
    tls_keylog_path: Option<PathBuf>,
    /// Buffered events ready to emit.
    event_queue: VecDeque<Result<DebugEvent, CoreError>>,
    /// Processing statistics.
    stats: PipelineStats,
    /// Whether processing has been initialized.
    initialized: bool,
}

impl PcapCaptureAdapter {
    /// Creates a new PCAP capture adapter.
    ///
    /// # Arguments
    /// * `capture_path` - Path to the PCAP/pcapng file
    /// * `tls_keylog_path` - Optional path to TLS keylog file for decryption
    pub fn new(capture_path: PathBuf, tls_keylog_path: Option<PathBuf>) -> Self {
        Self {
            capture_path,
            tls_keylog_path,
            event_queue: VecDeque::new(),
            stats: PipelineStats::default(),
            initialized: false,
        }
    }

    /// Returns a reference to the processing statistics.
    pub fn stats(&self) -> &PipelineStats {
        &self.stats
    }

    /// Processes all packets through the pipeline and populates the event queue.
    fn process_all_packets(&mut self) -> Result<(), CoreError> {
        // Open the PCAP file
        let mut reader = PcapFileReader::open(&self.capture_path)
            .map_err(|e| CoreError::Adapter(format!("failed to open PCAP file: {}", e)))?;

        // Read all packets
        let packets = reader
            .read_all_packets()
            .map_err(|e| CoreError::Adapter(format!("failed to read packets: {}", e)))?;

        tracing::info!(
            "Read {} packets from {}",
            packets.len(),
            self.capture_path.display()
        );

        // Initialize TLS processor with keylog
        let mut tls_processor = if let Some(ref keylog_path) = self.tls_keylog_path {
            // Load keylog file
            let keylog = TlsKeyLog::from_file(keylog_path).map_err(|e| {
                CoreError::Adapter(format!("failed to load TLS keylog: {}", e))
            })?;
            tracing::info!("Loaded {} TLS keys from keylog", keylog.len());
            TlsStreamProcessor::with_keylog(keylog)
        } else {
            // Also check for embedded TLS keys in pcapng DSB blocks
            let embedded_keys = reader.tls_keys();
            if !embedded_keys.is_empty() {
                tracing::info!(
                    "Found {} embedded TLS keys in pcapng DSB blocks",
                    embedded_keys.len()
                );
                // Convert TlsKeyStore to TlsKeyLog
                let keylog = TlsKeyLog::new();
                // Note: TlsKeyStore doesn't expose iteration, so we'll create a new processor
                // and manually add keys if needed. For now, just use an empty keylog.
                // TODO: Enhance TlsKeyStore to support iteration or conversion.
                TlsStreamProcessor::with_keylog(keylog)
            } else {
                TlsStreamProcessor::new()
            }
        };

        // Initialize packet normalizer and TCP reassembler
        let mut normalizer = PacketNormalizer::new();
        let mut tcp_reassembler = TcpReassembler::new();

        // Process packets through the pipeline
        for packet in &packets {
            self.stats.packets_read += 1;

            // Stage 1: Normalize packet (handle link types, IP defragmentation)
            let normalized = match normalizer.normalize(packet.linktype, packet.timestamp_us, &packet.data) {
                Ok(Some(norm)) => norm,
                Ok(None) => {
                    // Fragment waiting for more data - skip
                    continue;
                }
                Err(e) => {
                    // Parse error - log warning and continue
                    self.stats.packets_failed += 1;
                    tracing::warn!("Failed to normalize packet: {}", e);
                    continue;
                }
            };

            // Stage 2: Dispatch based on transport protocol
            match &normalized.transport {
                TransportInfo::Tcp(_) => {
                    // Process TCP segment through reassembler
                    let stream_events = match tcp_reassembler.process_segment(&normalized) {
                        Ok(events) => events,
                        Err(e) => {
                            tracing::warn!("TCP reassembly error: {}", e);
                            continue;
                        }
                    };

                    // Process stream events
                    for stream_event in stream_events {
                        match stream_event {
                            StreamEvent::Data(stream) => {
                                self.stats.tcp_streams += 1;
                                self.process_tcp_stream(stream, &mut tls_processor);
                            }
                            StreamEvent::GapSkipped { gap_size, .. } => {
                                tracing::warn!("TCP gap skipped: {} bytes", gap_size);
                            }
                            StreamEvent::Timeout { .. } => {
                                tracing::debug!("TCP connection timeout");
                            }
                        }
                    }
                }
                TransportInfo::Udp { src_port, dst_port } => {
                    // UDP datagram - convert directly to DebugEvent
                    self.stats.udp_datagrams += 1;
                    self.process_udp_datagram(&normalized, *src_port, *dst_port);
                }
                TransportInfo::Other(_) => {
                    // Ignore other protocols (ICMP, etc.)
                }
            }
        }

        // Flush any remaining TCP connections
        let current_time = packets
            .last()
            .map(|p| p.timestamp_us)
            .unwrap_or(0);
        let timeout_events = tcp_reassembler.cleanup_idle_connections(current_time + 1_000_000);
        for event in timeout_events {
            tracing::debug!("Flushed idle TCP connection: {:?}", event);
        }

        tracing::info!(
            "Pipeline complete: {} packets, {} TCP streams, {} UDP datagrams, {} TLS decrypted, {} TLS encrypted, {} failed",
            self.stats.packets_read,
            self.stats.tcp_streams,
            self.stats.udp_datagrams,
            self.stats.tls_decrypted,
            self.stats.tls_encrypted,
            self.stats.packets_failed
        );

        Ok(())
    }

    /// Processes a reassembled TCP stream through TLS decryption and emits DebugEvents.
    fn process_tcp_stream(
        &mut self,
        stream: ReassembledStream,
        tls_processor: &mut TlsStreamProcessor,
    ) {
        // Attempt TLS decryption
        let decrypted = match tls_processor.process_stream(stream) {
            Ok(dec) => dec,
            Err(e) => {
                tracing::warn!("TLS processing error: {}", e);
                return;
            }
        };

        // Update stats
        if decrypted.encrypted {
            self.stats.tls_encrypted += 1;
        } else {
            self.stats.tls_decrypted += 1;
        }

        // Convert to DebugEvent
        let event = self.create_debug_event_from_stream(decrypted);
        self.event_queue.push_back(Ok(event));
    }

    /// Processes a UDP datagram and emits a DebugEvent.
    fn process_udp_datagram(
        &mut self,
        normalized: &NormalizedPacket,
        src_port: u16,
        dst_port: u16,
    ) {
        let event = DebugEvent::builder()
            .timestamp(Timestamp::from_nanos(normalized.timestamp_us * 1000))
            .source(EventSource {
                adapter: "pcap".to_string(),
                origin: self.capture_path.display().to_string(),
                network: Some(NetworkAddr {
                    src: format!("{}:{}", normalized.src_ip, src_port),
                    dst: format!("{}:{}", normalized.dst_ip, dst_port),
                }),
            })
            .transport(TransportKind::RawUdp)
            .direction(self.infer_direction(normalized.src_ip, src_port))
            .payload(Payload::Raw {
                raw: Bytes::copy_from_slice(normalized.payload),
            })
            .build();

        self.event_queue.push_back(Ok(event));
    }

    /// Creates a DebugEvent from a decrypted stream.
    fn create_debug_event_from_stream(&self, stream: DecryptedStream) -> DebugEvent {
        let transport = TransportKind::RawTcp;
        let tls_decrypted = !stream.encrypted;

        DebugEvent::builder()
            .timestamp(Timestamp::from_nanos(stream.timestamp_us * 1000))
            .source(EventSource {
                adapter: "pcap".to_string(),
                origin: self.capture_path.display().to_string(),
                network: Some(NetworkAddr {
                    src: format!("{}:{}", stream.src_ip, stream.src_port),
                    dst: format!("{}:{}", stream.dst_ip, stream.dst_port),
                }),
            })
            .transport(transport)
            .direction(self.infer_direction(stream.src_ip, stream.src_port))
            .metadata("pcap.tls_decrypted", tls_decrypted.to_string())
            .payload(Payload::Raw {
                raw: Bytes::from(stream.data),
            })
            .build()
    }

    /// Infers message direction based on port heuristics.
    ///
    /// Common server ports:
    /// - 80, 443: HTTP/HTTPS
    /// - 50051: gRPC default
    /// - 5555: ZMQ common
    /// - 7400-7500: DDS RTPS
    fn infer_direction(&self, _src_ip: IpAddr, src_port: u16) -> Direction {
        // Well-known server ports
        let server_ports = [80, 443, 50051, 5555, 8080, 8443, 9090];

        if server_ports.contains(&src_port) || (7400..=7500).contains(&src_port) {
            // Server to client (or DDS RTPS range)
            Direction::Outbound
        } else {
            // Client to server (or unknown)
            Direction::Inbound
        }
    }
}

impl CaptureAdapter for PcapCaptureAdapter {
    fn name(&self) -> &str {
        "pcap"
    }

    fn ingest(&mut self) -> Box<dyn Iterator<Item = Result<DebugEvent, CoreError>> + '_> {
        // Initialize on first call
        if !self.initialized {
            self.initialized = true;
            if let Err(e) = self.process_all_packets() {
                // Push error to queue and return
                self.event_queue.push_back(Err(e));
            }
        }

        // Return iterator over queued events
        Box::new(std::iter::from_fn(|| self.event_queue.pop_front()))
    }
}

#[cfg(test)]
mod tests {
    // Tests will be added in separate integration test files
}
