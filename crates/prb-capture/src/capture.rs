//! Live packet capture engine.

use crate::config::CaptureConfig;
use crate::error::CaptureError;
use crate::privileges::PrivilegeCheck;
use crate::stats::{CaptureStats, CaptureStatsInner};
use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Instant;

/// An owned packet captured from the network.
#[derive(Debug, Clone)]
pub struct OwnedPacket {
    /// Packet timestamp in microseconds since Unix epoch.
    pub timestamp_us: u64,

    /// Original packet length (may be larger than `data.len()` if truncated).
    pub orig_len: u32,

    /// Packet data.
    pub data: Vec<u8>,
}

impl OwnedPacket {
    /// Convert from a pcap packet reference to owned data.
    #[must_use]
    pub fn from_pcap(packet: &pcap::Packet<'_>) -> Self {
        let ts = packet.header.ts;
        let timestamp_us = ts.tv_sec as u64 * 1_000_000 + ts.tv_usec as u64;
        Self {
            timestamp_us,
            orig_len: packet.header.len,
            data: packet.data.to_vec(),
        }
    }
}

/// Live packet capture engine.
///
/// Spawns a dedicated OS thread that continuously reads packets from a network
/// interface and delivers them over a bounded channel. The thread uses blocking
/// I/O but will never block on channel sends (uses `try_send` with drop counting).
pub struct CaptureEngine {
    config: CaptureConfig,
    rx: Option<Receiver<OwnedPacket>>,
    stop_flag: Arc<AtomicBool>,
    capture_thread: Option<JoinHandle<Result<(), CaptureError>>>,
    stats: Arc<CaptureStatsInner>,
    start_time: Instant,
    linktype: u32,
}

impl CaptureEngine {
    /// Create a new capture engine with the given configuration.
    #[must_use]
    pub fn new(config: CaptureConfig) -> Self {
        Self {
            config,
            rx: None,
            stop_flag: Arc::new(AtomicBool::new(false)),
            capture_thread: None,
            stats: Arc::new(CaptureStatsInner::new()),
            start_time: Instant::now(),
            linktype: 1, // Default to LINKTYPE_ETHERNET, updated after start
        }
    }

    /// Start the capture engine.
    ///
    /// This will:
    /// 1. Check that we have sufficient privileges
    /// 2. Open a pcap handle on the specified interface
    /// 3. Apply any BPF filter
    /// 4. Spawn a dedicated OS thread for the capture loop
    /// 5. Return immediately, with packets available via `receiver()`
    pub fn start(&mut self) -> Result<(), CaptureError> {
        if self.capture_thread.is_some() {
            return Err(CaptureError::AlreadyRunning);
        }

        // Check privileges
        PrivilegeCheck::check(&self.config.interface)?;

        // Open pcap capture device
        let mut cap = pcap::Capture::from_device(self.config.interface.as_str())?
            .snaplen(self.config.snaplen as i32)
            .promisc(self.config.promisc)
            .immediate_mode(self.config.immediate_mode)
            .buffer_size(self.config.buffer_size as i32)
            .timeout(self.config.timeout_ms)
            .open()?;

        // Apply BPF filter if specified
        if let Some(ref filter) = self.config.bpf_filter {
            cap.filter(filter, true).map_err(|e| {
                CaptureError::FilterCompilationFailed(format!("filter '{filter}': {e}"))
            })?;
        }

        // Get the actual link type from the pcap handle
        self.linktype = cap.get_datalink().0 as u32;

        // Create bounded channel for packet delivery
        // Channel size of 8192 packets provides good buffering without excessive memory
        let (tx, rx) = bounded::<OwnedPacket>(8192);

        // Reset state
        self.stop_flag.store(false, Ordering::Relaxed);
        self.start_time = Instant::now();

        // Clone Arc references for the capture thread
        let stop_flag = Arc::clone(&self.stop_flag);
        let stats = Arc::clone(&self.stats);

        // Spawn dedicated OS thread for capture loop
        let handle = thread::spawn(move || capture_loop(cap, tx, stop_flag, stats));

        self.rx = Some(rx);
        self.capture_thread = Some(handle);

        tracing::info!(
            interface = %self.config.interface,
            filter = ?self.config.bpf_filter,
            "capture engine started"
        );

        Ok(())
    }

    /// Stop the capture engine and wait for the thread to finish.
    ///
    /// Returns the final capture statistics.
    pub fn stop(&mut self) -> Result<CaptureStats, CaptureError> {
        if self.capture_thread.is_none() {
            return Ok(self.stats()); // Already stopped
        }

        // Signal the capture thread to stop
        self.stop_flag.store(true, Ordering::Relaxed);

        // Wait for the thread to finish
        if let Some(handle) = self.capture_thread.take() {
            match handle.join() {
                Ok(result) => result?,
                Err(e) => {
                    return Err(CaptureError::Other(format!(
                        "capture thread panicked: {e:?}"
                    )));
                }
            }
        }

        tracing::info!("capture engine stopped");

        Ok(self.stats())
    }

    /// Get the receiver for captured packets.
    ///
    /// Returns None if the engine hasn't been started yet.
    #[must_use]
    pub const fn receiver(&self) -> Option<&Receiver<OwnedPacket>> {
        self.rx.as_ref()
    }

    /// Get the link-layer type of the capture.
    ///
    /// Returns the LINKTYPE value (e.g., 1 for Ethernet, 228 for IPv4, 229 for IPv6).
    /// This is only valid after `start()` has been called.
    #[must_use]
    pub const fn linktype(&self) -> u32 {
        self.linktype
    }

    /// Get a snapshot of current capture statistics.
    #[must_use]
    pub fn stats(&self) -> CaptureStats {
        self.stats.snapshot(self.start_time)
    }
}

impl Drop for CaptureEngine {
    fn drop(&mut self) {
        if self.capture_thread.is_some() {
            let _ = self.stop();
        }
    }
}

/// The capture loop that runs in a dedicated OS thread.
///
/// This continuously calls `cap.next_packet()` in a blocking loop, converts
/// packets to owned data, and delivers them via a bounded channel. If the
/// channel is full, packets are dropped and counted.
fn capture_loop(
    mut cap: pcap::Capture<pcap::Active>,
    tx: Sender<OwnedPacket>,
    stop: Arc<AtomicBool>,
    stats: Arc<CaptureStatsInner>,
) -> Result<(), CaptureError> {
    let mut packet_count = 0u64;

    while !stop.load(Ordering::Relaxed) {
        match cap.next_packet() {
            Ok(packet) => {
                // Convert to owned packet
                let owned = OwnedPacket::from_pcap(&packet);
                let data_len = owned.data.len();

                // Update statistics
                stats.packets_received.fetch_add(1, Ordering::Relaxed);
                stats
                    .bytes_received
                    .fetch_add(data_len as u64, Ordering::Relaxed);

                // Try to send (non-blocking)
                if let Err(TrySendError::Full(_)) = tx.try_send(owned) {
                    stats
                        .packets_dropped_channel
                        .fetch_add(1, Ordering::Relaxed);
                }

                packet_count += 1;

                // Poll kernel statistics every 1000 packets to reduce overhead
                if packet_count.is_multiple_of(1000)
                    && let Ok(pcap_stats) = cap.stats()
                {
                    stats
                        .packets_dropped_kernel
                        .store(u64::from(pcap_stats.dropped), Ordering::Relaxed);
                }
            }
            Err(pcap::Error::TimeoutExpired) => {
                // No packet received within timeout, check stop flag
                continue;
            }
            Err(e) => {
                tracing::error!(error = %e, "pcap error in capture loop");
                return Err(CaptureError::Pcap(e));
            }
        }
    }

    // Final stats poll before exiting
    if let Ok(pcap_stats) = cap.stats() {
        stats
            .packets_dropped_kernel
            .store(u64::from(pcap_stats.dropped), Ordering::Relaxed);
    }

    Ok(())
}
