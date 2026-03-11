//! Live capture data source and TUI integration.

use prb_capture::{CaptureStats, LiveCaptureAdapter};
use prb_core::{CaptureAdapter, DebugEvent};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;

/// Events that can occur in the TUI event loop.
#[derive(Debug)]
pub enum AppEvent {
    /// A keyboard event.
    Key(crossterm::event::KeyEvent),
    /// A tick for periodic UI updates.
    Tick,
    /// Terminal resize event.
    Resize(u16, u16),
    /// A new debug event captured from the network.
    CapturedEvent(Box<DebugEvent>),
    /// Updated capture statistics.
    CaptureStats(CaptureStats),
    /// Capture has stopped (either manually or due to error).
    CaptureStopped,
}

/// Capture state for the control bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureState {
    /// Actively capturing packets.
    Capturing,
    /// Paused (still receiving packets but not displaying).
    Paused,
    /// Capture stopped.
    Stopped,
}

/// Live capture data source that bridges the capture adapter to the TUI.
///
/// This spawns a dedicated OS thread that runs the blocking iterator from
/// LiveCaptureAdapter and forwards events over a tokio channel to the TUI.
pub struct LiveDataSource {
    interface: String,
    stop_flag: Arc<AtomicBool>,
    event_rx: Option<mpsc::Receiver<AppEvent>>,
}

impl LiveDataSource {
    /// Create a new live data source and start capture.
    ///
    /// This immediately starts the capture adapter and spawns background threads
    /// to forward events to the TUI event loop.
    pub fn start(
        mut adapter: LiveCaptureAdapter,
        interface: String,
    ) -> Result<Self, prb_capture::CaptureError> {
        // Start the adapter
        adapter.start()?;

        let (tx, rx) = mpsc::channel(1000);
        let stop_flag = Arc::new(AtomicBool::new(false));

        // Spawn event forwarder thread (OS thread, not tokio task)
        // This is necessary because the adapter's ingest() is blocking
        let tx_events = tx.clone();
        let stop_flag_clone = Arc::clone(&stop_flag);
        thread::spawn(move || {
            capture_event_forwarder(adapter, tx_events, stop_flag_clone);
        });

        Ok(Self {
            interface,
            stop_flag,
            event_rx: Some(rx),
        })
    }

    /// Get the interface name.
    pub fn interface(&self) -> &str {
        &self.interface
    }

    /// Take the event receiver (can only be called once).
    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<AppEvent>> {
        self.event_rx.take()
    }

    /// Signal the capture to stop.
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

/// Forwards events from the capture adapter to the TUI event channel.
///
/// This runs in a dedicated OS thread (not a tokio task) because the adapter's
/// ingest() method is blocking and returns a synchronous iterator.
fn capture_event_forwarder(
    mut adapter: LiveCaptureAdapter,
    tx: mpsc::Sender<AppEvent>,
    stop_flag: Arc<AtomicBool>,
) {
    let mut event_count = 0u64;
    let mut last_stats = std::time::Instant::now();

    for event_result in adapter.ingest() {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }

        match event_result {
            Ok(event) => {
                event_count += 1;

                // Send event to TUI (blocking send from non-async context)
                if tx.blocking_send(AppEvent::CapturedEvent(Box::new(event))).is_err() {
                    tracing::debug!("TUI event channel closed, stopping capture");
                    break;
                }

                // Send stats update every second
                if last_stats.elapsed() >= Duration::from_secs(1) {
                    // TODO: Get real stats from the capture engine
                    let stats = CaptureStats {
                        packets_received: event_count,
                        packets_dropped_kernel: 0,
                        packets_dropped_channel: 0,
                        bytes_received: 0,
                        capture_duration: last_stats.elapsed(),
                        packets_per_second: event_count as f64 / last_stats.elapsed().as_secs_f64(),
                        bytes_per_second: 0.0,
                    };
                    let _ = tx.blocking_send(AppEvent::CaptureStats(stats));
                    last_stats = std::time::Instant::now();
                }
            }
            Err(e) => {
                tracing::warn!("Capture event error: {}", e);
            }
        }
    }

    // Notify TUI that capture has stopped
    let _ = tx.blocking_send(AppEvent::CaptureStopped);
    tracing::info!("Capture event forwarder stopped");
}
