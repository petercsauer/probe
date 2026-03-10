//! Live packet capture engine for the PRB universal message debugger.
//!
//! This crate provides live packet capture from network interfaces using libpcap.
//! It includes:
//!
//! - Interface enumeration and selection
//! - BPF filter compilation and application
//! - Dedicated OS thread capture loop with bounded channel delivery
//! - Real-time statistics tracking
//! - Privilege checking (Linux CAP_NET_RAW)
//!
//! # Architecture
//!
//! The capture engine uses a dedicated OS thread (not a tokio task) for the packet
//! capture loop. This is critical for production capture systems because:
//!
//! 1. `cap.next_packet()` must be called continuously without interruption
//! 2. Tokio tasks can be preempted by the scheduler on a loaded runtime
//! 3. Preemption causes packet drops in the kernel ring buffer
//! 4. A real OS thread guarantees the capture loop is never preempted
//!
//! All production capture systems (Hubble, Suricata, Zeek, Wireshark) use this model.
//!
//! # Example
//!
//! ```no_run
//! use prb_capture::{CaptureEngine, CaptureConfig};
//!
//! let config = CaptureConfig::new("eth0")
//!     .with_filter("tcp port 443");
//!
//! let mut engine = CaptureEngine::new(config);
//! engine.start().expect("failed to start capture");
//!
//! // Receive packets
//! if let Some(rx) = engine.receiver() {
//!     for packet in rx.iter() {
//!         println!("Captured {} bytes", packet.data.len());
//!     }
//! }
//!
//! // Stop and get statistics
//! let stats = engine.stop().expect("failed to stop capture");
//! println!("{}", stats);
//! ```

pub mod adapter;
pub mod capture;
pub mod config;
pub mod error;
pub mod interfaces;
pub mod privileges;
pub mod stats;

pub use adapter::LiveCaptureAdapter;
pub use capture::{CaptureEngine, OwnedPacket};
pub use config::CaptureConfig;
pub use error::CaptureError;
pub use interfaces::{InterfaceEnumerator, InterfaceInfo};
pub use privileges::PrivilegeCheck;
pub use stats::CaptureStats;
