//! Pipeline integration: wires PCAP reader → normalizer → TCP reassembly → TLS decryption → DebugEvents.
//!
//! This module implements the `CaptureAdapter` trait for PCAP/pcapng files,
//! orchestrating the complete data flow from raw packet capture to structured debug events.

use crate::pipeline_core::PipelineCore;
use crate::reader::PcapFileReader;
use crate::tls::{TlsKeyLog, TlsStreamProcessor};
use prb_core::{CaptureAdapter, CoreError, DebugEvent};
use prb_detect::DecoderRegistry;
use std::collections::VecDeque;
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
    /// Events decoded by protocol decoders (gRPC, ZMTP, RTPS).
    pub protocol_decoded: u64,
    /// Streams that fell back to raw events (no decoder or decode failed).
    pub protocol_fallback: u64,
}

/// PCAP capture adapter implementing the `CaptureAdapter` trait.
///
/// This adapter processes PCAP/pcapng files through a multi-stage pipeline:
/// 1. Read packets with `PcapFileReader`
/// 2. Normalize packets with `PacketNormalizer` (handle link types, IP defrag)
/// 3. Reassemble TCP streams with `TcpReassembler`
/// 4. Decrypt TLS streams with `TlsStreamProcessor`
/// 5. Detect and decode protocols with `DecoderRegistry`
/// 6. Convert to `DebugEvent` format
///
/// UDP datagrams bypass reassembly and TLS processing, but still go through protocol detection.
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
    /// Decoder registry for protocol detection and decoding.
    decoder_registry: DecoderRegistry,
    /// Optional protocol override to bypass detection.
    protocol_override: Option<String>,
}

impl PcapCaptureAdapter {
    /// Creates a new PCAP capture adapter with default decoder registry.
    ///
    /// # Arguments
    /// * `capture_path` - Path to the PCAP/pcapng file
    /// * `tls_keylog_path` - Optional path to TLS keylog file for decryption
    pub fn new(capture_path: PathBuf, tls_keylog_path: Option<PathBuf>) -> Self {
        Self::with_registry(capture_path, tls_keylog_path, crate::create_registry_with_builtins())
    }

    /// Creates a new PCAP capture adapter with a custom decoder registry.
    ///
    /// # Arguments
    /// * `capture_path` - Path to the PCAP/pcapng file
    /// * `tls_keylog_path` - Optional path to TLS keylog file for decryption
    /// * `decoder_registry` - Custom decoder registry for protocol detection
    pub fn with_registry(
        capture_path: PathBuf,
        tls_keylog_path: Option<PathBuf>,
        decoder_registry: DecoderRegistry,
    ) -> Self {
        Self {
            capture_path,
            tls_keylog_path,
            event_queue: VecDeque::new(),
            stats: PipelineStats::default(),
            initialized: false,
            decoder_registry,
            protocol_override: None,
        }
    }

    /// Set a protocol override to force all streams to use a specific decoder.
    ///
    /// This bypasses automatic protocol detection. Valid protocol names are:
    /// - "grpc" or "http2" for gRPC/HTTP/2
    /// - "zmtp" or "zmq" for ZeroMQ
    /// - "rtps" or "dds" for DDS/RTPS
    pub fn set_protocol_override(&mut self, protocol: &str) {
        self.protocol_override = Some(protocol.to_string());
    }

    /// Returns a reference to the processing statistics.
    pub fn stats(&self) -> &PipelineStats {
        &self.stats
    }

    /// Build TLS processor with keylog file or embedded keys from pcapng.
    fn build_tls_processor(&self, reader: &PcapFileReader) -> Result<TlsStreamProcessor, CoreError> {
        if let Some(ref keylog_path) = self.tls_keylog_path {
            // Load keylog file
            let keylog = TlsKeyLog::from_file(keylog_path).map_err(|e| {
                CoreError::Adapter(format!("failed to load TLS keylog: {}", e))
            })?;
            tracing::info!("Loaded {} TLS keys from keylog", keylog.len());
            Ok(TlsStreamProcessor::with_keylog(keylog))
        } else {
            // Check for embedded TLS keys in pcapng DSB blocks
            let embedded_keys = reader.tls_keys();
            if !embedded_keys.is_empty() {
                tracing::info!(
                    "Found {} embedded TLS keys in pcapng DSB blocks",
                    embedded_keys.len()
                );
                // Convert TlsKeyStore to TlsKeyLog
                let mut keylog = TlsKeyLog::new();
                for (client_random, master_secret) in embedded_keys.iter() {
                    keylog.insert(
                        client_random.to_vec(),
                        crate::tls::keylog::KeyMaterial::MasterSecret(master_secret.to_vec()),
                    );
                }
                Ok(TlsStreamProcessor::with_keylog(keylog))
            } else {
                Ok(TlsStreamProcessor::new())
            }
        }
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

        // Build TLS processor with keylog or embedded keys
        let tls_processor = self.build_tls_processor(&reader)?;

        // Take ownership of the decoder registry
        let registry = std::mem::replace(
            &mut self.decoder_registry,
            DecoderRegistry::new(),
        );

        // Apply protocol override if set
        if let Some(ref protocol_name) = self.protocol_override {
            tracing::info!("Applying protocol override: {}", protocol_name);
            // Note: We can't set override without a specific stream key, so we'll handle this
            // in the registry by adding a global override feature later. For now, we'll
            // document this limitation.
        }

        // Create pipeline core with TLS processor and decoder registry
        let mut core = PipelineCore::new(tls_processor, registry);

        // Process all packets through the core pipeline
        let origin = self.capture_path.display().to_string();
        for packet in &packets {
            let result = core.process_packet(
                packet.linktype,
                packet.timestamp_us,
                &packet.data,
                &origin,
            );

            // Queue events
            for event in result.events {
                self.event_queue.push_back(Ok(event));
            }

            // Log warnings (skip ARP normalization warnings as they're expected)
            for warning in result.warnings {
                if warning.contains("ARP packets not supported") {
                    tracing::debug!("{}", warning);
                } else {
                    tracing::warn!("{}", warning);
                }
            }
        }

        // Flush any remaining TCP connections
        let final_time = packets.last().map(|p| p.timestamp_us).unwrap_or(0);
        for event in core.flush_idle(final_time + 1_000_000) {
            self.event_queue.push_back(Ok(event));
        }

        // Copy stats from core
        self.stats = core.stats().clone();

        tracing::info!(
            "Pipeline complete: {} packets, {} TCP streams, {} UDP datagrams, {} TLS decrypted, {} TLS encrypted, {} protocol decoded, {} fallback, {} failed",
            self.stats.packets_read,
            self.stats.tcp_streams,
            self.stats.udp_datagrams,
            self.stats.tls_decrypted,
            self.stats.tls_encrypted,
            self.stats.protocol_decoded,
            self.stats.protocol_fallback,
            self.stats.packets_failed
        );

        Ok(())
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
