---
segment: 12
title: "Capture Engine (prb-capture)"
depends_on: []
risk: 8
complexity: High
cycle_budget: 4
status: pending
commit_message: "feat(prb-capture): add live packet capture engine with pcap, BPF, and stats"
---

# S1: Capture Engine (`prb-capture` crate)

**Goal**: New crate providing live packet capture via the `pcap` crate. Handles
interface enumeration, BPF filter compilation, a dedicated OS thread capture loop
with bounded channel delivery, and real-time statistics.

**References**: Hubble (eBPF + ring buffer), Suricata (AF_PACKET per-thread rings),
Zeek (dedicated capture threads), tcpdump (PACKET_MMAP via libpcap).

---

## S1.1: Crate Scaffold + `CaptureConfig`

### Workspace Registration

Add to workspace `Cargo.toml`:

```toml
# In [workspace] members:
"crates/prb-capture",

# In [workspace.dependencies]:
pcap = { version = "2.4", features = ["capture-stream"] }
crossbeam-channel = "0.5"
tokio = { version = "1", features = ["rt", "sync", "macros", "time"] }
```

### `crates/prb-capture/Cargo.toml`

```toml
[package]
name = "prb-capture"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
prb-core = { path = "../prb-core" }
pcap = { workspace = true }
crossbeam-channel = { workspace = true }
tokio = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }

[target.'cfg(target_os = "linux")'.dependencies]
caps = "0.5"

[dev-dependencies]
tempfile = { workspace = true }
```

### `crates/prb-capture/src/lib.rs`

```rust
pub mod capture;
pub mod config;
pub mod error;
pub mod interfaces;
pub mod privileges;
pub mod stats;

pub use capture::CaptureEngine;
pub use config::CaptureConfig;
pub use error::CaptureError;
pub use interfaces::{InterfaceEnumerator, InterfaceInfo};
pub use privileges::PrivilegeCheck;
pub use stats::CaptureStats;
```

### `CaptureConfig` struct

```rust
pub struct CaptureConfig {
    pub interface: String,
    pub bpf_filter: Option<String>,
    pub snaplen: u32,
    pub promisc: bool,
    pub immediate_mode: bool,
    pub buffer_size: u32,
    pub timeout_ms: i32,
    pub tls_keylog_path: Option<PathBuf>,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            interface: String::new(),
            bpf_filter: None,
            snaplen: 65535,
            promisc: true,
            immediate_mode: true,
            buffer_size: 16 * 1024 * 1024, // 16 MB
            timeout_ms: 1000,
            tls_keylog_path: None,
        }
    }
}
```

### `CaptureError` enum

```rust
#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("pcap error: {0}")]
    Pcap(#[from] pcap::Error),

    #[error("interface not found: {0}")]
    InterfaceNotFound(String),

    #[error("insufficient privileges: {message}\n\nFix: {remediation}")]
    InsufficientPrivileges {
        message: String,
        remediation: String,
    },

    #[error("BPF filter compilation failed: {0}")]
    FilterCompilationFailed(String),

    #[error("capture channel closed")]
    ChannelClosed,

    #[error("capture already running")]
    AlreadyRunning,

    #[error("{0}")]
    Other(String),
}
```

---

## S1.2: `CaptureEngine` — OS Thread Capture Loop

The core component. Opens a live pcap handle, spawns a dedicated OS thread that
calls `cap.next_packet()` in a blocking loop, and delivers owned packet data over
a bounded crossbeam channel.

### Why a dedicated OS thread (not a tokio task)?

Every production capture system (Hubble, Suricata, Zeek, Wireshark) uses a
dedicated thread or process for packet capture. The reason: `cap.next_packet()`
must be called continuously without interruption. A tokio task can be preempted
by the scheduler if other tasks are running. On a loaded runtime, this causes
packet drops in the kernel ring buffer. A real OS thread guarantees the capture
loop is never preempted by user-space scheduling.

### Owned Packet

```rust
pub struct OwnedPacket {
    pub timestamp_us: u64,
    pub orig_len: u32,
    pub data: Vec<u8>,
}

impl OwnedPacket {
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
```

### `CaptureEngine`

```rust
use crossbeam_channel::{Receiver, Sender, TrySendError};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

pub struct CaptureEngine {
    config: CaptureConfig,
    rx: Option<Receiver<OwnedPacket>>,
    stop_flag: Arc<AtomicBool>,
    capture_thread: Option<JoinHandle<Result<(), CaptureError>>>,
    stats: Arc<CaptureStatsInner>,
}

struct CaptureStatsInner {
    packets_received: AtomicU64,
    packets_dropped_kernel: AtomicU64,
    packets_dropped_channel: AtomicU64,
    bytes_received: AtomicU64,
}

impl CaptureEngine {
    pub fn new(config: CaptureConfig) -> Self { /* ... */ }

    pub fn start(&mut self) -> Result<(), CaptureError> {
        // 1. Check privileges (S6)
        // 2. Open pcap handle with config
        // 3. Apply BPF filter if present
        // 4. Create crossbeam::bounded(8192) channel
        // 5. Spawn OS thread with capture loop
        // 6. Store rx, thread handle, stop flag
    }

    pub fn stop(&mut self) -> Result<(), CaptureError> {
        // 1. Set stop_flag to true
        // 2. Join capture thread
        // 3. Return final stats
    }

    pub fn receiver(&self) -> Option<&Receiver<OwnedPacket>> {
        self.rx.as_ref()
    }

    pub fn stats(&self) -> CaptureStats { /* snapshot from atomics */ }
}
```

### Capture Thread Loop

```rust
fn capture_loop(
    mut cap: pcap::Capture<pcap::Active>,
    tx: Sender<OwnedPacket>,
    stop: Arc<AtomicBool>,
    stats: Arc<CaptureStatsInner>,
) -> Result<(), CaptureError> {
    while !stop.load(Ordering::Relaxed) {
        match cap.next_packet() {
            Ok(packet) => {
                let owned = OwnedPacket::from_pcap(&packet);
                stats.packets_received.fetch_add(1, Ordering::Relaxed);
                stats.bytes_received.fetch_add(owned.data.len() as u64, Ordering::Relaxed);

                if let Err(TrySendError::Full(_)) = tx.try_send(owned) {
                    stats.packets_dropped_channel.fetch_add(1, Ordering::Relaxed);
                }
            }
            Err(pcap::Error::TimeoutExpired) => continue,
            Err(e) => return Err(CaptureError::Pcap(e)),
        }

        // Periodically poll kernel stats
        if let Ok(pcap_stats) = cap.stats() {
            stats.packets_dropped_kernel
                .store(pcap_stats.dropped as u64, Ordering::Relaxed);
        }
    }

    Ok(())
}
```

**Critical design choices**:
- `try_send()` (non-blocking): capture thread never blocks waiting for consumers.
  If channel is full, packet is dropped and counted. This is the Hubble/Suricata model.
- `timeout_ms: 1000`: pcap returns `TimeoutExpired` after 1s of no packets,
  allowing the loop to check the stop flag. Without timeout, `next_packet()` blocks
  forever on quiet interfaces.
- `pcap.stats()` polled every packet to track kernel drops (PACKET_MMAP ring
  overflows). In practice, poll less frequently (every 1000 packets) to reduce overhead.

---

## S1.3: `InterfaceEnumerator`

Wraps `pcap::Device::list()` with user-friendly interface information.

```rust
pub struct InterfaceInfo {
    pub name: String,
    pub description: Option<String>,
    pub addresses: Vec<std::net::IpAddr>,
    pub is_up: bool,
    pub is_running: bool,
    pub is_loopback: bool,
    pub is_wireless: bool,
}

pub struct InterfaceEnumerator;

impl InterfaceEnumerator {
    pub fn list() -> Result<Vec<InterfaceInfo>, CaptureError> {
        let devices = pcap::Device::list()?;
        Ok(devices.into_iter().map(InterfaceInfo::from_device).collect())
    }

    pub fn find(name: &str) -> Result<InterfaceInfo, CaptureError> {
        Self::list()?
            .into_iter()
            .find(|i| i.name == name)
            .ok_or_else(|| CaptureError::InterfaceNotFound(name.to_string()))
    }

    pub fn default_device() -> Result<InterfaceInfo, CaptureError> {
        let device = pcap::Device::lookup()?.ok_or_else(|| {
            CaptureError::Other("no default capture device found".into())
        })?;
        Ok(InterfaceInfo::from_device(device))
    }
}
```

### Display Format for `--list-interfaces`

```
Interface   Status   Addresses                   Description
─────────   ──────   ─────────────────────────   ───────────
eth0        UP       192.168.1.100, fe80::1      Ethernet adapter
lo          UP       127.0.0.1, ::1              Loopback
wlan0       UP       192.168.1.101               Wi-Fi adapter
docker0     DOWN     172.17.0.1                  Docker bridge
```

---

## S1.4: `CaptureStats`

Real-time statistics exposed to UI and CLI output.

```rust
#[derive(Debug, Clone)]
pub struct CaptureStats {
    pub packets_received: u64,
    pub packets_dropped_kernel: u64,
    pub packets_dropped_channel: u64,
    pub bytes_received: u64,
    pub capture_duration: std::time::Duration,
    pub packets_per_second: f64,
    pub bytes_per_second: f64,
}

impl CaptureStats {
    pub fn total_drops(&self) -> u64 {
        self.packets_dropped_kernel + self.packets_dropped_channel
    }

    pub fn drop_rate(&self) -> f64 {
        if self.packets_received == 0 {
            0.0
        } else {
            self.total_drops() as f64 / self.packets_received as f64
        }
    }
}

impl std::fmt::Display for CaptureStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
```

### Stats Polling

The capture thread updates atomic counters. The `CaptureEngine::stats()` method
takes a snapshot:

```rust
impl CaptureEngine {
    pub fn stats(&self) -> CaptureStats {
        let now = std::time::Instant::now();
        let duration = now.duration_since(self.start_time);
        let received = self.stats.packets_received.load(Ordering::Relaxed);

        CaptureStats {
            packets_received: received,
            packets_dropped_kernel: self.stats.packets_dropped_kernel.load(Ordering::Relaxed),
            packets_dropped_channel: self.stats.packets_dropped_channel.load(Ordering::Relaxed),
            bytes_received: self.stats.bytes_received.load(Ordering::Relaxed),
            capture_duration: duration,
            packets_per_second: received as f64 / duration.as_secs_f64().max(0.001),
            bytes_per_second: self.stats.bytes_received.load(Ordering::Relaxed) as f64
                / duration.as_secs_f64().max(0.001),
        }
    }
}
```

---

## Implementation Checklist

- [ ] Create `crates/prb-capture/` directory with `Cargo.toml`
- [ ] Register in workspace `Cargo.toml`
- [ ] Implement `CaptureConfig` with defaults
- [ ] Implement `CaptureError` enum
- [ ] Implement `OwnedPacket` struct
- [ ] Implement `CaptureEngine` with start/stop lifecycle
- [ ] Implement capture thread loop with `try_send` + drop counting
- [ ] Implement `InterfaceEnumerator` wrapping `pcap::Device::list()`
- [ ] Implement `CaptureStats` with atomic snapshot
- [ ] Add `InterfaceInfo::from_device` conversion
- [ ] Unit test: `CaptureConfig::default()` has sane values
- [ ] Unit test: `CaptureError` display messages include remediation
- [ ] Unit test: `OwnedPacket::from_pcap` timestamp conversion
- [ ] Integration test: `InterfaceEnumerator::list()` returns at least loopback
