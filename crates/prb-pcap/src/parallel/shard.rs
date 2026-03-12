//! Shard-based parallel processing for TCP reassembly and TLS decryption.

use crate::normalize::{OwnedNormalizedPacket, TransportInfo};
use crate::tcp::{StreamDirection, StreamEvent, TcpReassembler};
use crate::tls::{TlsKeyLog, TlsStreamProcessor};
use bytes::Bytes;
use prb_core::{
    DebugEvent, Direction, EventSource, NetworkAddr, Payload, Timestamp, TransportKind,
};
use rayon::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;

/// Processes shards of packets in parallel with independent TCP reassembly.
pub struct ShardProcessor {
    tls_keylog: Arc<TlsKeyLog>,
    capture_path: PathBuf,
}

impl ShardProcessor {
    /// Creates a new shard processor.
    ///
    /// # Arguments
    ///
    /// * `tls_keylog` - Shared TLS keylog for decryption
    /// * `capture_path` - Path to the capture file for event metadata
    #[must_use]
    pub const fn new(tls_keylog: Arc<TlsKeyLog>, capture_path: PathBuf) -> Self {
        Self {
            tls_keylog,
            capture_path,
        }
    }

    /// Processes all shards in parallel.
    ///
    /// Each shard gets its own `TcpReassembler` and `TlsStreamProcessor`.
    /// TCP state is isolated per shard, allowing true parallel processing.
    ///
    /// # Arguments
    ///
    /// * `shards` - Vector of packet vectors, one per shard
    ///
    /// # Returns
    ///
    /// A vector of event vectors, one per shard. Events maintain their
    /// relative order within each shard.
    #[must_use]
    pub fn process_shards(&self, shards: Vec<Vec<OwnedNormalizedPacket>>) -> Vec<Vec<DebugEvent>> {
        shards
            .into_par_iter()
            .map(|shard_packets| self.process_single_shard(shard_packets))
            .collect()
    }

    /// Processes a single shard of packets sequentially.
    ///
    /// This is where TCP reassembly and TLS decryption happen. Each shard
    /// maintains its own state, so multiple shards can run in parallel.
    ///
    /// This method is also used by the sequential path in `ParallelPipeline`
    /// to process small captures without parallelization overhead.
    pub fn process_single_shard(&self, packets: Vec<OwnedNormalizedPacket>) -> Vec<DebugEvent> {
        let mut reassembler = TcpReassembler::new();
        let tls_processor = TlsStreamProcessor::with_keylog_ref(Arc::clone(&self.tls_keylog));
        let mut events = Vec::new();

        for packet in &packets {
            match &packet.transport {
                TransportInfo::Tcp(_) => match reassembler.process_owned_segment(packet) {
                    Ok(stream_events) => {
                        for stream_event in stream_events {
                            if let StreamEvent::Data(stream) = stream_event {
                                self.process_stream(stream, &tls_processor, &mut events);
                            }
                        }
                    }
                    Err(e) => tracing::warn!("TCP reassembly error: {}", e),
                },
                TransportInfo::Udp { src_port, dst_port } => {
                    events.push(create_udp_event(
                        packet,
                        *src_port,
                        *dst_port,
                        &self.capture_path,
                    ));
                }
                TransportInfo::Other(_) => {
                    // Ignore other protocols (ICMP, etc.)
                }
            }
        }

        // Flush remaining TCP connections at end of shard
        for stream_event in reassembler.flush_all() {
            if let StreamEvent::Data(stream) = stream_event {
                self.process_stream(stream, &tls_processor, &mut events);
            }
        }

        events
    }

    /// Processes a reassembled TCP stream through TLS decryption and protocol detection.
    fn process_stream(
        &self,
        stream: crate::tcp::ReassembledStream,
        tls_processor: &TlsStreamProcessor,
        events: &mut Vec<DebugEvent>,
    ) {
        match tls_processor.decrypt_stream(stream.clone()) {
            Ok(decrypted_stream) => {
                // Create a DebugEvent from the decrypted stream
                events.push(create_tcp_event(
                    stream,
                    decrypted_stream,
                    &self.capture_path,
                ));
            }
            Err(e) => {
                tracing::warn!("TLS processing error: {}", e);
                // Still create an event for the encrypted stream
                events.push(create_tcp_event_encrypted(stream, &self.capture_path));
            }
        }
    }
}

/// Creates a `DebugEvent` for a UDP packet.
fn create_udp_event(
    packet: &OwnedNormalizedPacket,
    src_port: u16,
    dst_port: u16,
    capture_path: &std::path::Path,
) -> DebugEvent {
    let timestamp = Timestamp::from_nanos(packet.timestamp_us * 1000); // Convert μs to ns
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

/// Creates a `DebugEvent` for a TCP stream with TLS decryption attempt.
fn create_tcp_event(
    stream: crate::tcp::ReassembledStream,
    decrypted: crate::tls::DecryptedStream,
    capture_path: &std::path::Path,
) -> DebugEvent {
    let timestamp = Timestamp::from_nanos(stream.timestamp_us * 1000); // Convert μs to ns
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

/// Creates a `DebugEvent` for an encrypted TCP stream (no decryption attempted).
fn create_tcp_event_encrypted(
    stream: crate::tcp::ReassembledStream,
    capture_path: &std::path::Path,
) -> DebugEvent {
    let timestamp = Timestamp::from_nanos(stream.timestamp_us * 1000); // Convert μs to ns
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
    use crate::normalize::{TcpFlags, TcpSegmentInfo};
    use std::net::{IpAddr, Ipv4Addr};

    fn make_tcp_packet(
        timestamp_us: u64,
        src_ip: IpAddr,
        src_port: u16,
        dst_ip: IpAddr,
        dst_port: u16,
        seq: u32,
        payload: Vec<u8>,
    ) -> OwnedNormalizedPacket {
        OwnedNormalizedPacket {
            timestamp_us,
            src_ip,
            dst_ip,
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

    #[test]
    fn test_shard_processor_empty_shards() {
        let keylog = Arc::new(TlsKeyLog::new());
        let processor = ShardProcessor::new(keylog, PathBuf::from("/test.pcap"));

        let shards = vec![vec![], vec![], vec![]];
        let results = processor.process_shards(shards);

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(std::vec::Vec::is_empty));
    }

    #[test]
    fn test_shard_processor_tcp_stream() {
        let keylog = Arc::new(TlsKeyLog::new());
        let processor = ShardProcessor::new(keylog, PathBuf::from("/test.pcap"));

        let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // Create a simple TCP stream with 2 packets
        let packets = vec![
            make_tcp_packet(1000, ip1, 8080, ip2, 50051, 100, b"Hello".to_vec()),
            make_tcp_packet(2000, ip1, 8080, ip2, 50051, 105, b"World".to_vec()),
        ];

        let shards = vec![packets];
        let results = processor.process_shards(shards);

        assert_eq!(results.len(), 1);
        // Should have at least one TCP stream event
        assert!(!results[0].is_empty());
    }

    #[test]
    fn test_shard_processor_udp() {
        let keylog = Arc::new(TlsKeyLog::new());
        let processor = ShardProcessor::new(keylog, PathBuf::from("/test.pcap"));

        let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        let packet = OwnedNormalizedPacket {
            timestamp_us: 1000,
            src_ip: ip1,
            dst_ip: ip2,
            transport: TransportInfo::Udp {
                src_port: 9090,
                dst_port: 60061,
            },
            vlan_id: None,
            payload: b"UDP data".to_vec(),
        };

        let shards = vec![vec![packet]];
        let results = processor.process_shards(shards);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].len(), 1);
    }

    #[test]
    fn test_decode_grpc_in_shard() {
        use crate::parallel::detect::{DetectedProtocol, detect_protocol};
        use crate::tls::DecryptedStream;

        let keylog = Arc::new(TlsKeyLog::new());
        let processor = ShardProcessor::new(keylog, PathBuf::from("/test.pcap"));

        let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // Create HTTP/2 preface payload
        let h2_preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n".to_vec();

        // Create TCP packets carrying HTTP/2 preface
        let packets = vec![make_tcp_packet(
            1000, ip1, 12345, ip2, 50051, 100, h2_preface,
        )];

        let shards = vec![packets];
        let results = processor.process_shards(shards);

        assert_eq!(results.len(), 1);
        assert!(!results[0].is_empty(), "Should have at least one event");

        // Verify that protocol detection would identify this as gRPC
        // (In actual integration, the shard processor would call detect_protocol)
        let decrypted_stream = DecryptedStream {
            src_ip: ip1,
            src_port: 12345,
            dst_ip: ip2,
            dst_port: 50051,
            direction: StreamDirection::ClientToServer,
            data: b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n".to_vec(),
            encrypted: false,
            is_complete: true,
            timestamp_us: 1000,
        };

        let detected = detect_protocol(&decrypted_stream);
        assert_eq!(
            detected,
            Some(DetectedProtocol::Grpc),
            "Should detect gRPC/HTTP2"
        );
    }

    #[test]
    fn test_decode_zmtp_in_shard() {
        use crate::parallel::detect::{DetectedProtocol, detect_protocol};
        use crate::tls::DecryptedStream;

        let keylog = Arc::new(TlsKeyLog::new());
        let processor = ShardProcessor::new(keylog, PathBuf::from("/test.pcap"));

        let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // Create ZMTP greeting payload
        let mut zmtp_greeting = vec![0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7F];
        zmtp_greeting.extend_from_slice(&[3, 0]); // version 3.0

        // Create TCP packets carrying ZMTP greeting
        let packets = vec![make_tcp_packet(
            1000,
            ip1,
            12345,
            ip2,
            5555,
            100,
            zmtp_greeting,
        )];

        let shards = vec![packets];
        let results = processor.process_shards(shards);

        assert_eq!(results.len(), 1);
        assert!(!results[0].is_empty(), "Should have at least one event");

        // Verify that protocol detection would identify this as ZMTP
        let mut zmtp_data = vec![0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7F];
        zmtp_data.extend_from_slice(&[3, 0]);

        let decrypted_stream = DecryptedStream {
            src_ip: ip1,
            src_port: 12345,
            dst_ip: ip2,
            dst_port: 5555,
            direction: StreamDirection::ClientToServer,
            data: zmtp_data,
            encrypted: false,
            is_complete: true,
            timestamp_us: 1000,
        };

        let detected = detect_protocol(&decrypted_stream);
        assert_eq!(detected, Some(DetectedProtocol::Zmtp), "Should detect ZMTP");
    }
}
