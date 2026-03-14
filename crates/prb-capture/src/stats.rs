//! Statistics tracking for live packet capture.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Real-time capture statistics.
#[derive(Debug, Clone)]
pub struct CaptureStats {
    /// Total packets received by the capture engine.
    pub packets_received: u64,

    /// Packets dropped by the kernel (ring buffer overflow).
    pub packets_dropped_kernel: u64,

    /// Packets dropped due to full channel (consumer too slow).
    pub packets_dropped_channel: u64,

    /// Total bytes received.
    pub bytes_received: u64,

    /// Duration of the capture session.
    pub capture_duration: Duration,

    /// Packets per second rate.
    pub packets_per_second: f64,

    /// Bytes per second rate.
    pub bytes_per_second: f64,
}

impl CaptureStats {
    /// Total packets dropped (kernel + channel).
    #[must_use]
    pub const fn total_drops(&self) -> u64 {
        self.packets_dropped_kernel + self.packets_dropped_channel
    }

    /// Packet drop rate as a fraction (0.0 to 1.0).
    #[must_use]
    pub fn drop_rate(&self) -> f64 {
        if self.packets_received == 0 {
            0.0
        } else {
            self.total_drops() as f64 / self.packets_received as f64
        }
    }

    /// Drop rate as a percentage.
    #[must_use]
    pub fn drop_percentage(&self) -> f64 {
        self.drop_rate() * 100.0
    }
}

impl fmt::Display for CaptureStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} packets captured, {} dropped (kernel: {}, channel: {}), {:.1} pps",
            self.packets_received,
            self.total_drops(),
            self.packets_dropped_kernel,
            self.packets_dropped_channel,
            self.packets_per_second,
        )
    }
}

/// Internal atomic statistics counters.
///
/// This is exposed through stats handles to allow polling statistics
/// without holding references to the capture engine or adapter.
#[derive(Debug)]
pub struct CaptureStatsInner {
    pub packets_received: AtomicU64,
    pub packets_dropped_kernel: AtomicU64,
    pub packets_dropped_channel: AtomicU64,
    pub bytes_received: AtomicU64,
}

impl CaptureStatsInner {
    pub const fn new() -> Self {
        Self {
            packets_received: AtomicU64::new(0),
            packets_dropped_kernel: AtomicU64::new(0),
            packets_dropped_channel: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
        }
    }

    /// Take a snapshot of current statistics.
    #[must_use]
    pub fn snapshot(&self, start_time: Instant) -> CaptureStats {
        let now = Instant::now();
        let duration = now.duration_since(start_time);
        let duration_secs = duration.as_secs_f64().max(0.001); // Avoid division by zero

        let received = self.packets_received.load(Ordering::Relaxed);
        let bytes = self.bytes_received.load(Ordering::Relaxed);

        CaptureStats {
            packets_received: received,
            packets_dropped_kernel: self.packets_dropped_kernel.load(Ordering::Relaxed),
            packets_dropped_channel: self.packets_dropped_channel.load(Ordering::Relaxed),
            bytes_received: bytes,
            capture_duration: duration,
            packets_per_second: received as f64 / duration_secs,
            bytes_per_second: bytes as f64 / duration_secs,
        }
    }
}
