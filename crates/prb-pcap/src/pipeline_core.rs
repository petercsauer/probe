//! Stateful packet processing pipeline shared between batch and streaming adapters.
//!
//! This module extracts the core normalize → reassemble → TLS decrypt → emit logic
//! from the PCAP adapter so it can be reused for live capture sources.

use crate::normalize::{PacketNormalizer, TransportInfo};
use crate::tcp::{ReassembledStream, StreamEvent, TcpReassembler};
use crate::tls::{DecryptedStream, TlsKeyLog, TlsStreamProcessor};
use crate::{PcapError, PipelineStats};
use bytes::Bytes;
use prb_core::{DebugEvent, DecodeContext, Direction, EventSource, NetworkAddr, Payload, Timestamp, TransportKind};
use prb_detect::{DecoderRegistry, StreamKey, TransportLayer};
use std::net::IpAddr;
use std::path::Path;

/// Result from processing a single packet through the pipeline.
#[derive(Debug, Default)]
pub struct ProcessedEvents {
    /// Zero or more DebugEvents produced from this packet.
    pub events: Vec<DebugEvent>,
    /// Non-fatal warnings encountered during processing.
    pub warnings: Vec<String>,
}

/// Stateful pipeline core for packet processing.
///
/// This struct maintains the state required to process packets incrementally:
/// - PacketNormalizer: handles link-layer encapsulation and IP defragmentation
/// - TcpReassembler: reconstructs TCP streams from segments
/// - TlsStreamProcessor: decrypts TLS streams using keylog
/// - DecoderRegistry: detects protocols and decodes messages
/// - PipelineStats: tracks processing metrics
///
/// The pipeline can be used in two modes:
/// - Batch: process all packets from a file, then flush remaining streams
/// - Streaming: process packets one-at-a-time as they arrive, with periodic flush_idle() calls
pub struct PipelineCore {
    normalizer: PacketNormalizer,
    tcp_reassembler: TcpReassembler,
    tls_processor: TlsStreamProcessor,
    decoder_registry: DecoderRegistry,
    stats: PipelineStats,
}

impl PipelineCore {
    /// Creates a new pipeline with the given TLS processor and decoder registry.
    pub fn new(tls_processor: TlsStreamProcessor, decoder_registry: DecoderRegistry) -> Self {
        Self {
            normalizer: PacketNormalizer::new(),
            tcp_reassembler: TcpReassembler::new(),
            tls_processor,
            decoder_registry,
            stats: PipelineStats::default(),
        }
    }

    /// Creates a new pipeline with TLS keylog loaded from a file.
    pub fn with_keylog(keylog_path: &Path, decoder_registry: DecoderRegistry) -> Result<Self, PcapError> {
        let keylog = TlsKeyLog::from_file(keylog_path)
            .map_err(|e| PcapError::TlsKey(format!("failed to load TLS keylog: {}", e)))?;
        Ok(Self::new(TlsStreamProcessor::with_keylog(keylog), decoder_registry))
    }

    /// Get a mutable reference to the decoder registry for configuration.
    pub fn decoder_registry_mut(&mut self) -> &mut DecoderRegistry {
        &mut self.decoder_registry
    }

    /// Process a single raw packet through the pipeline.
    ///
    /// This is the hot path — called once per captured packet. It performs:
    /// 1. Normalization (link-layer parsing, IP defragmentation)
    /// 2. Transport dispatch (TCP reassembly or direct UDP processing)
    /// 3. TLS decryption (for TCP streams)
    /// 4. DebugEvent creation
    ///
    /// # Arguments
    /// * `linktype` - PCAP link-layer type (e.g., 1 for Ethernet)
    /// * `timestamp_us` - Packet capture timestamp in microseconds since epoch
    /// * `data` - Raw packet data including link-layer header
    /// * `origin` - Human-readable origin string (e.g., file path or "live:eth0")
    ///
    /// # Returns
    /// A ProcessedEvents struct containing zero or more DebugEvents and any warnings.
    pub fn process_packet(
        &mut self,
        linktype: u32,
        timestamp_us: u64,
        data: &[u8],
        origin: &str,
    ) -> ProcessedEvents {
        let mut result = ProcessedEvents::default();

        self.stats.packets_read += 1;

        // Stage 1: Normalize packet (handle link types, IP defragmentation)
        // We need to convert to owned data to avoid borrow checker issues
        let owned_normalized = match self.normalizer.normalize(linktype, timestamp_us, data) {
            Ok(Some(norm)) => {
                // Convert borrowed NormalizedPacket to owned
                crate::OwnedNormalizedPacket::from_normalized(&norm)
            }
            Ok(None) => {
                // Fragment waiting for more data - no events yet
                return result;
            }
            Err(e) => {
                // Parse error - log warning and continue
                self.stats.packets_failed += 1;
                result.warnings.push(format!("normalize failed: {}", e));
                return result;
            }
        };

        // Stage 2: Dispatch based on transport protocol
        match &owned_normalized.transport {
            TransportInfo::Tcp(_) => {
                self.process_tcp_segment(&owned_normalized, origin, &mut result);
            }
            TransportInfo::Udp { src_port, dst_port } => {
                self.stats.udp_datagrams += 1;
                self.process_udp_datagram(&owned_normalized, *src_port, *dst_port, origin, &mut result);
            }
            TransportInfo::Other(_) => {
                // Ignore other protocols (ICMP, etc.)
            }
        }

        result
    }

    /// Process a TCP segment through reassembly, TLS decryption, and event creation.
    fn process_tcp_segment(
        &mut self,
        normalized: &crate::OwnedNormalizedPacket,
        origin: &str,
        result: &mut ProcessedEvents,
    ) {
        // Convert to borrowed form for TCP reassembler
        let borrowed = normalized.as_normalized();

        // Process TCP segment through reassembler
        let stream_events = match self.tcp_reassembler.process_segment(&borrowed) {
            Ok(events) => events,
            Err(e) => {
                result.warnings.push(format!("TCP reassembly error: {}", e));
                return;
            }
        };

        // Process stream events
        for stream_event in stream_events {
            match stream_event {
                StreamEvent::Data(stream) => {
                    self.stats.tcp_streams += 1;
                    if let Some(event) = self.process_tcp_stream(stream, origin) {
                        result.events.push(event);
                    }
                }
                StreamEvent::GapSkipped { gap_size, .. } => {
                    result.warnings.push(format!("TCP gap skipped: {} bytes", gap_size));
                }
                StreamEvent::Timeout { .. } => {
                    tracing::debug!("TCP connection timeout");
                }
            }
        }
    }

    /// Process a UDP datagram through protocol detection and decoding.
    fn process_udp_datagram(
        &mut self,
        normalized: &crate::OwnedNormalizedPacket,
        src_port: u16,
        dst_port: u16,
        origin: &str,
        result: &mut ProcessedEvents,
    ) {
        // Build stream key for decoder routing
        let stream_key = StreamKey::new(
            format!("{}:{}", normalized.src_ip, src_port),
            format!("{}:{}", normalized.dst_ip, dst_port),
            TransportLayer::Udp,
        );

        // Build decode context
        let ctx = DecodeContext::new()
            .with_src_addr(&format!("{}:{}", normalized.src_ip, src_port))
            .with_dst_addr(&format!("{}:{}", normalized.dst_ip, dst_port))
            .with_timestamp(Timestamp::from_nanos(normalized.timestamp_us * 1000))
            .with_metadata("pcap.origin", origin.to_string());

        // Route through decoder registry
        match self.decoder_registry.process_datagram(stream_key, &normalized.payload, &ctx) {
            Ok(events) if !events.is_empty() => {
                self.stats.protocol_decoded += events.len() as u64;
                for event in events {
                    result.events.push(event);
                }
            }
            Ok(_) | Err(_) => {
                // Fallback: raw UDP event
                self.stats.protocol_fallback += 1;
                let event = create_udp_event(normalized, src_port, dst_port, origin);
                result.events.push(event);
            }
        }
    }

    /// Process a reassembled TCP stream through TLS decryption and protocol decoding.
    fn process_tcp_stream(&mut self, stream: ReassembledStream, origin: &str) -> Option<DebugEvent> {
        // Attempt TLS decryption
        let decrypted = match self.tls_processor.process_stream(stream) {
            Ok(dec) => dec,
            Err(e) => {
                tracing::warn!("TLS processing error: {}", e);
                return None;
            }
        };

        // Update TLS stats
        if decrypted.encrypted {
            self.stats.tls_encrypted += 1;
        } else {
            self.stats.tls_decrypted += 1;
        }

        // Build stream key for decoder routing
        let stream_key = StreamKey::new(
            format!("{}:{}", decrypted.src_ip, decrypted.src_port),
            format!("{}:{}", decrypted.dst_ip, decrypted.dst_port),
            TransportLayer::Tcp,
        );

        // Build decode context
        let ctx = DecodeContext::new()
            .with_src_addr(&format!("{}:{}", decrypted.src_ip, decrypted.src_port))
            .with_dst_addr(&format!("{}:{}", decrypted.dst_ip, decrypted.dst_port))
            .with_timestamp(Timestamp::from_nanos(decrypted.timestamp_us * 1000))
            .with_metadata("pcap.tls_decrypted", (!decrypted.encrypted).to_string())
            .with_metadata("pcap.origin", origin.to_string());

        // Route through decoder registry
        match self.decoder_registry.process_stream(stream_key, &decrypted.data, &ctx) {
            Ok(events) if !events.is_empty() => {
                self.stats.protocol_decoded += events.len() as u64;
                // Return the first event (multi-event streams will be handled in future work)
                Some(events.into_iter().next().unwrap())
            }
            Ok(_) | Err(_) => {
                // No events produced or decode failed — emit raw fallback
                self.stats.protocol_fallback += 1;
                Some(create_tcp_event(&decrypted, origin))
            }
        }
    }

    /// Flush idle TCP connections.
    ///
    /// Call this periodically (e.g., every second) during live capture to emit
    /// buffered stream data from connections that have gone idle. For batch
    /// processing, call once at the end with a far-future timestamp.
    ///
    /// # Arguments
    /// * `current_time_us` - Current time in microseconds since epoch
    ///
    /// # Returns
    /// Vector of DebugEvents from flushed connections.
    pub fn flush_idle(&mut self, current_time_us: u64) -> Vec<DebugEvent> {
        let timeout_events = self.tcp_reassembler.cleanup_idle_connections(current_time_us);

        let mut events = Vec::new();
        for event in timeout_events {
            if let StreamEvent::Data(stream) = event {
                // We don't have the origin string here, use a placeholder
                if let Some(evt) = self.process_tcp_stream(stream, "flushed") {
                    events.push(evt);
                }
            }
        }
        events
    }

    /// Returns a reference to the processing statistics.
    pub fn stats(&self) -> &PipelineStats {
        &self.stats
    }
}

/// Creates a DebugEvent from a UDP datagram.
fn create_udp_event(
    normalized: &crate::OwnedNormalizedPacket,
    src_port: u16,
    dst_port: u16,
    origin: &str,
) -> DebugEvent {
    DebugEvent::builder()
        .timestamp(Timestamp::from_nanos(normalized.timestamp_us * 1000))
        .source(EventSource {
            adapter: "pcap".to_string(),
            origin: origin.to_string(),
            network: Some(NetworkAddr {
                src: format!("{}:{}", normalized.src_ip, src_port),
                dst: format!("{}:{}", normalized.dst_ip, dst_port),
            }),
        })
        .transport(TransportKind::RawUdp)
        .direction(infer_direction(normalized.src_ip, src_port))
        .payload(Payload::Raw {
            raw: Bytes::copy_from_slice(&normalized.payload),
        })
        .build()
}

/// Creates a DebugEvent from a TCP stream (potentially decrypted).
fn create_tcp_event(stream: &DecryptedStream, origin: &str) -> DebugEvent {
    let transport = TransportKind::RawTcp;
    let tls_decrypted = !stream.encrypted;

    DebugEvent::builder()
        .timestamp(Timestamp::from_nanos(stream.timestamp_us * 1000))
        .source(EventSource {
            adapter: "pcap".to_string(),
            origin: origin.to_string(),
            network: Some(NetworkAddr {
                src: format!("{}:{}", stream.src_ip, stream.src_port),
                dst: format!("{}:{}", stream.dst_ip, stream.dst_port),
            }),
        })
        .transport(transport)
        .direction(infer_direction(stream.src_ip, stream.src_port))
        .metadata("pcap.tls_decrypted", tls_decrypted.to_string())
        .payload(Payload::Raw {
            raw: Bytes::from(stream.data.clone()),
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
fn infer_direction(_src_ip: IpAddr, src_port: u16) -> Direction {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_core_stats() {
        let registry = DecoderRegistry::new();
        let mut core = PipelineCore::new(TlsStreamProcessor::new(), registry);
        let stats = core.stats();
        assert_eq!(stats.packets_read, 0);
        assert_eq!(stats.packets_failed, 0);
        assert_eq!(stats.tcp_streams, 0);
        assert_eq!(stats.udp_datagrams, 0);
    }

    #[test]
    fn test_process_packet_invalid() {
        let registry = DecoderRegistry::new();
        let mut core = PipelineCore::new(TlsStreamProcessor::new(), registry);

        // Process an invalid packet (too short)
        let result = core.process_packet(1, 1000000, &[0xAA; 10], "test");

        // Should produce no events but increment failed counter
        assert_eq!(result.events.len(), 0);
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(core.stats().packets_read, 1);
        assert_eq!(core.stats().packets_failed, 1);
    }
}
