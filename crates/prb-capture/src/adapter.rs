//! Live capture adapter implementing the CaptureAdapter trait.
//!
//! This module wraps the CaptureEngine and PipelineCore to provide a streaming
//! CaptureAdapter implementation for live network capture.

use crate::{CaptureConfig, CaptureEngine, CaptureError};
use prb_core::{CaptureAdapter, CoreError, DebugEvent};
use prb_pcap::{PipelineCore, TlsStreamProcessor};
use std::collections::VecDeque;
use std::time::Duration;

/// Live capture adapter that implements CaptureAdapter for streaming packet sources.
///
/// This adapter combines a CaptureEngine (which reads packets from a network interface)
/// with a PipelineCore (which normalizes, reassembles, and decrypts packets into DebugEvents).
///
/// # Architecture
///
/// The CaptureEngine runs in a dedicated OS thread, continuously reading packets from
/// libpcap and delivering them over a bounded channel. The adapter pulls from this channel
/// and feeds packets through the PipelineCore incrementally.
///
/// # Example
///
/// ```no_run
/// use prb_capture::{CaptureConfig, LiveCaptureAdapter};
/// use prb_core::CaptureAdapter;
///
/// let config = CaptureConfig::new("lo0")
///     .with_filter("tcp port 8080");
///
/// let mut adapter = LiveCaptureAdapter::new(config).unwrap();
/// adapter.start().unwrap();
///
/// // Consume events as they arrive
/// for event in adapter.ingest() {
///     println!("Event: {:?}", event);
/// }
/// ```
pub struct LiveCaptureAdapter {
    engine: CaptureEngine,
    core: PipelineCore,
    event_buffer: VecDeque<Result<DebugEvent, CoreError>>,
    origin: String,
    linktype: u32,
    started: bool,
}

impl LiveCaptureAdapter {
    /// Creates a new live capture adapter with the given configuration.
    ///
    /// # Arguments
    /// * `config` - Capture configuration specifying interface, filter, TLS keys, etc.
    ///
    /// # Errors
    /// Returns an error if TLS keylog file cannot be loaded.
    pub fn new(config: CaptureConfig) -> Result<Self, CaptureError> {
        let origin = format!("live:{}", config.interface);

        // Build TLS processor from optional keylog path
        let tls = if let Some(ref keylog_path) = config.tls_keylog_path {
            PipelineCore::with_keylog(keylog_path)
                .map_err(|e| CaptureError::Other(format!("failed to load TLS keylog: {}", e)))?
        } else {
            PipelineCore::new(TlsStreamProcessor::new())
        };

        Ok(Self {
            engine: CaptureEngine::new(config),
            core: tls,
            event_buffer: VecDeque::new(),
            origin,
            linktype: 1, // Default to LINKTYPE_ETHERNET, updated after start
            started: false,
        })
    }

    /// Starts the capture engine.
    ///
    /// This must be called before calling `ingest()`. It spawns a background thread
    /// that continuously reads packets from the network interface.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Insufficient privileges (not root or missing CAP_NET_RAW)
    /// - Interface doesn't exist
    /// - BPF filter compilation fails
    pub fn start(&mut self) -> Result<(), CaptureError> {
        self.engine.start()?;

        // Get the actual link type from the pcap handle
        // LINKTYPE_ETHERNET = 1 is the most common
        self.linktype = 1; // TODO: Extract from engine if possible

        self.started = true;
        tracing::info!(interface = %self.origin, "live capture adapter started");
        Ok(())
    }

    /// Stops the capture engine and returns final statistics.
    ///
    /// This blocks until the capture thread exits cleanly.
    pub fn stop(&mut self) -> Result<crate::CaptureStats, CaptureError> {
        self.started = false;
        self.engine.stop()
    }

    /// Returns a reference to the current pipeline statistics.
    pub fn pipeline_stats(&self) -> &prb_pcap::PipelineStats {
        self.core.stats()
    }

    /// Flushes idle TCP connections.
    ///
    /// Call this periodically (e.g., every second) to emit buffered data from
    /// idle connections. For live capture, this ensures low-latency event delivery.
    pub fn flush_idle(&mut self) {
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        for event in self.core.flush_idle(now_us) {
            self.event_buffer.push_back(Ok(event));
        }
    }
}

impl CaptureAdapter for LiveCaptureAdapter {
    fn name(&self) -> &str {
        "live-capture"
    }

    fn ingest(&mut self) -> Box<dyn Iterator<Item = Result<DebugEvent, CoreError>> + '_> {
        if !self.started {
            // Return an error iterator if not started
            return Box::new(std::iter::once(Err(CoreError::Adapter(
                "capture engine not started - call start() first".to_string(),
            ))));
        }

        // Create an iterator that pulls from the capture channel and processes packets
        // We can't use from_fn with a closure that captures self, so we use a struct iterator
        Box::new(LiveIngestIterator { adapter: self })
    }
}

/// Iterator that pulls packets from the capture engine and processes them through the pipeline.
struct LiveIngestIterator<'a> {
    adapter: &'a mut LiveCaptureAdapter,
}

impl<'a> Iterator for LiveIngestIterator<'a> {
    type Item = Result<DebugEvent, CoreError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Drain buffered events first
        if let Some(event) = self.adapter.event_buffer.pop_front() {
            return Some(event);
        }

        // Block on next packet from capture thread with timeout
        loop {
            // Get receiver inside loop to avoid holding a borrow
            let rx = match self.adapter.engine.receiver() {
                Some(rx) => rx,
                None => {
                    return Some(Err(CoreError::Adapter(
                        "capture engine receiver not available".to_string(),
                    )));
                }
            };

            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(packet) => {
                    // Process packet through pipeline
                    let result = self.adapter.core.process_packet(
                        self.adapter.linktype,
                        packet.timestamp_us,
                        &packet.data,
                        &self.adapter.origin,
                    );

                    // Buffer events
                    for event in result.events {
                        self.adapter.event_buffer.push_back(Ok(event));
                    }

                    // Log warnings
                    for warning in result.warnings {
                        tracing::warn!("{}", warning);
                    }

                    // Return next event if available
                    if let Some(event) = self.adapter.event_buffer.pop_front() {
                        return Some(event);
                    }
                    // Packet produced no events, try next
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    // Periodically flush idle TCP connections
                    self.adapter.flush_idle();
                    if let Some(event) = self.adapter.event_buffer.pop_front() {
                        return Some(event);
                    }
                    // No events yet, continue waiting
                }
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    // Capture stopped - flush any remaining events
                    self.adapter.flush_idle();
                    if let Some(event) = self.adapter.event_buffer.pop_front() {
                        return Some(event);
                    }
                    // No more events
                    return None;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_live_adapter_not_started() {
        // Test that adapter returns error if ingest() called before start()
        let config = CaptureConfig::new("lo0");
        let mut adapter = LiveCaptureAdapter::new(config).unwrap();

        let mut iter = adapter.ingest();
        let result = iter.next().unwrap();

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not started"));
    }
}
