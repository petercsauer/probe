//! Thread-safe pipeline statistics using atomic counters.

use std::sync::atomic::{AtomicU64, Ordering};

/// Atomic pipeline statistics for monitoring streaming pipeline.
///
/// Uses `AtomicU64` for lock-free concurrent updates from multiple threads.
#[derive(Debug)]
pub struct AtomicPipelineStats {
    /// Total packets received from source.
    pub packets_received: AtomicU64,
    /// Packets successfully normalized.
    pub packets_normalized: AtomicU64,
    /// IP fragments dropped (not supported in streaming mode).
    pub fragments: AtomicU64,
    /// Packets routed to shards.
    pub packets_routed: AtomicU64,
    /// Events emitted from all shards.
    pub events_emitted: AtomicU64,
    /// Normalization errors.
    pub normalize_errors: AtomicU64,
}

impl AtomicPipelineStats {
    /// Creates a new stats structure with all counters initialized to zero.
    #[must_use] 
    pub const fn new() -> Self {
        Self {
            packets_received: AtomicU64::new(0),
            packets_normalized: AtomicU64::new(0),
            fragments: AtomicU64::new(0),
            packets_routed: AtomicU64::new(0),
            events_emitted: AtomicU64::new(0),
            normalize_errors: AtomicU64::new(0),
        }
    }

    /// Takes a snapshot of current statistics.
    ///
    /// Uses `Relaxed` ordering since we only need eventual consistency for monitoring,
    /// not strict ordering guarantees.
    pub fn snapshot(&self) -> PipelineStats {
        PipelineStats {
            packets_received: self.packets_received.load(Ordering::Relaxed),
            packets_normalized: self.packets_normalized.load(Ordering::Relaxed),
            fragments: self.fragments.load(Ordering::Relaxed),
            packets_routed: self.packets_routed.load(Ordering::Relaxed),
            events_emitted: self.events_emitted.load(Ordering::Relaxed),
            normalize_errors: self.normalize_errors.load(Ordering::Relaxed),
        }
    }
}

impl Default for AtomicPipelineStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Non-atomic snapshot of pipeline statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineStats {
    pub packets_received: u64,
    pub packets_normalized: u64,
    pub fragments: u64,
    pub packets_routed: u64,
    pub events_emitted: u64,
    pub normalize_errors: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_stats_initialization() {
        let stats = AtomicPipelineStats::new();
        let snapshot = stats.snapshot();

        assert_eq!(snapshot.packets_received, 0);
        assert_eq!(snapshot.packets_normalized, 0);
        assert_eq!(snapshot.fragments, 0);
        assert_eq!(snapshot.packets_routed, 0);
        assert_eq!(snapshot.events_emitted, 0);
        assert_eq!(snapshot.normalize_errors, 0);
    }

    #[test]
    fn test_atomic_stats_increment() {
        let stats = AtomicPipelineStats::new();

        stats.packets_received.fetch_add(100, Ordering::Relaxed);
        stats.packets_normalized.fetch_add(95, Ordering::Relaxed);
        stats.fragments.fetch_add(3, Ordering::Relaxed);
        stats.normalize_errors.fetch_add(2, Ordering::Relaxed);

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.packets_received, 100);
        assert_eq!(snapshot.packets_normalized, 95);
        assert_eq!(snapshot.fragments, 3);
        assert_eq!(snapshot.normalize_errors, 2);
    }

    #[test]
    fn test_snapshot_consistency() {
        let stats = AtomicPipelineStats::new();

        stats.packets_received.fetch_add(10, Ordering::Relaxed);
        let snap1 = stats.snapshot();

        stats.packets_received.fetch_add(5, Ordering::Relaxed);
        let snap2 = stats.snapshot();

        assert_eq!(snap1.packets_received, 10);
        assert_eq!(snap2.packets_received, 15);
    }
}
