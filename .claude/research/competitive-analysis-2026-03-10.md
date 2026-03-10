# Live Network Packet Capture in Rust — State of the Art (March 2026)

Comprehensive research report covering libraries, architectures, and production patterns
for building a live capture and protocol-debugging tool in Rust.

---

## 1. Rust Pcap & Networking Libraries

### 1.1 `pcap` crate (v2.4.0) — Primary Recommendation

| Attribute       | Details |
|-----------------|---------|
| **Crate**       | [`pcap`](https://crates.io/crates/pcap) v2.4.0 |
| **Wraps**       | libpcap (Linux/macOS) / Npcap (Windows) |
| **Downloads**   | 4.18M total, ~71k/month |
| **License**     | MIT OR Apache-2.0 |
| **Last update** | 2025-11-26 |

**Key features**:
- Device enumeration and selection (`Device::list()`, `Device::lookup()`)
- Live capture with configurable snaplen, promiscuous mode, timeout, buffer size
- BPF filter compilation and application (`capture.filter("tcp port 443", true)`)
- Send/inject packets into interfaces
- Read/write pcap and pcapng savefiles
- `capture-stream` feature flag: async `Stream` via tokio + futures
- `lending-iter` feature flag: zero-copy iteration via GATs

**Live capture pattern**:
```rust
use pcap::{Device, Capture};

let mut cap = Capture::from_device("eth0")?
    .promisc(true)
    .snaplen(65535)
    .timeout(1000)       // ms; 0 = block forever
    .immediate_mode(true) // bypass kernel buffering for low latency
    .open()?;

cap.filter("tcp port 443", true)?;  // BPF filter

while let Ok(packet) = cap.next_packet() {
    // packet.header: timestamp, caplen, origlen
    // packet.data: &[u8] (borrowed from ring buffer)
}
```

**Async (tokio) pattern** — built-in via `capture-stream` feature:
```rust
use pcap::{Device, Capture};
use futures::StreamExt;

let cap = Capture::from_device("eth0")?
    .promisc(true)
    .immediate_mode(true)
    .open()?
    .setnonblock()?;

let mut stream = cap.stream(pcap::Codec)?;

while let Some(Ok(packet)) = stream.next().await {
    // packet is owned; safe to move across await points
}
```

**Verdict**: Best starting point for prb. Mature, cross-platform, well-maintained.
The `capture-stream` feature gives native tokio integration. Uses PACKET_MMAP
under the hood on Linux (via libpcap ≥1.5), so you get the kernel ring buffer
for free.

### 1.2 `async-pcap` (v0.1.6)

Simpler async alternative — spawns a dedicated OS thread, delivers packets via
unbounded async channel as `Vec<u8>`. Less control than pcap's built-in stream,
but trivially drops into existing tokio applications. Published October 2025.

**Trade-off**: Extra allocation per packet (`Vec<u8>` copy). Fine for ≤100k pps
analysis workloads, but the pcap crate's native `capture-stream` is preferred
for higher throughput.

### 1.3 `pnet` / `pnet_packet` (v0.35.0)

| Attribute | Details |
|-----------|---------|
| **Crate** | [`pnet`](https://crates.io/crates/pnet) / `pnet_packet` v0.35.0 |
| **Approach** | Raw socket capture + packet construction |
| **Cross-platform** | Linux, macOS, Windows (via WinPcap) |
| **Last update** | 2024-05 |

Provides both capture (via raw sockets / libpcap backend) and parsing. The parsing
side (`pnet_packet`) uses proc-macro-generated zero-copy packet structs. Supports
ARP, Ethernet, ICMP, ICMPv6, IPv4, IPv6, TCP, UDP, VLAN, GRE.

**Best for**: When you need raw socket access (send/receive) without libpcap, or
when you want pnet's macro-based packet definitions for building/modifying packets.

**Not recommended for prb capture** because the pcap crate gives better
filter support, better async integration, and more active maintenance.

### 1.4 `etherparse` (v0.19.0) — Recommended for Parsing

| Attribute | Details |
|-----------|---------|
| **Crate** | [`etherparse`](https://crates.io/crates/etherparse) v0.19.0 |
| **Downloads** | 5.6M total |
| **License** | MIT OR Apache-2.0 |
| **Last update** | 2025-08-03 |

**Key features**:
- Zero-allocation parsing (no heap, no syscalls)
- Two modes: **slicing** (lazy, zero-copy field access) vs **full parsing** (all headers into structs)
- IP defragmentation support
- Protocols: Ethernet II, 802.1Q VLAN, MACsec, IPv4/v6, TCP, UDP, ICMP/v6, ARP, Linux SLL
- PacketBuilder for constructing packets

**Slicing pattern** (preferred for live capture — fast path):
```rust
use etherparse::SlicedPacket;

let sliced = SlicedPacket::from_ethernet(packet_data)?;
if let Some(ip) = sliced.ip() { /* ... */ }
if let Some(transport) = sliced.transport() { /* ... */ }
let payload = sliced.payload; // application-layer bytes
```

**Verdict**: Superior to `pnet_packet` for prb's parsing needs. Zero-allocation
design is critical for high-throughput live capture. Already in prb's workspace
dependencies. The slicing API is ideal for a filter-then-decode pipeline.

### 1.5 `smoltcp` (MSRV: Rust 1.80)

Embedded TCP/IP stack — not a capture library. Useful if you need to implement
your own protocol stack (e.g., for tap/tun virtual interfaces or userspace TCP).
Has raw socket, TAP interface, and loopback backends plus middleware for
tracing and fault injection.

**Not relevant for prb's live capture** but worth knowing about for virtual
interface testing scenarios.

### 1.6 AF_XDP / XDP Bindings (Advanced)

For kernel-bypass zero-copy capture at 10-100 Gbps:

| Crate | Version | Stars | Notes |
|-------|---------|-------|-------|
| `quick_afxdp` | 0.4.0 (Dec 2025) | — | Wraps libbpf. ~6.5M pps/core on 10G NIC. Zero-copy mode. |
| `xsk-rs` | latest | 53 | Direct AF_XDP socket manipulation. |
| `aya` | latest (Mar 2026) | 4,352 | Full eBPF framework. XDP programs, maps, ring buffers. |

**AF_XDP architecture**:
- UMEM: pre-allocated shared memory region between kernel and userspace
- 4 ring buffers per socket: RX, TX, Fill, Completion
- Kernel delivers packets directly into UMEM frames; userspace polls RX ring
- Zero system calls in the hot path (busy-poll mode)
- ~6.5M pps single-core; linear scaling with cores

**When to use**: Only needed if prb targets sustained >1M pps capture. For
typical debugging workloads (1k–100k pps), libpcap's PACKET_MMAP is sufficient.

### 1.7 `aya` — eBPF Framework (v0.13+)

| Attribute | Details |
|-----------|---------|
| **Crate** | [`aya`](https://crates.io/crates/aya) |
| **GitHub** | 4,352 stars, updated 2026-03-08 |
| **Approach** | Pure Rust eBPF (no libbpf/bcc dependency) |

**Capabilities relevant to packet capture**:
- XDP programs for line-rate packet inspection/filtering
- TC (traffic control) programs for egress capture
- Maps: HashMap, RingBuf, PerfEventArray, XskMap
- Async support (tokio and async-std)
- BTF for portable eBPF programs across kernel versions

**Pattern**: Write eBPF in Rust (via `aya-bpf`), load with `aya` userspace.
Use `PerfEventArray` or `RingBuf` maps to stream packets to userspace.

**Verdict**: Excellent for building Hubble-like observability. Overkill for
prb's initial live capture feature, but the right choice if you later want
in-kernel filtering, flow tracking, or DNS/HTTP visibility without full packet
capture overhead.

---

## 2. State-of-the-Art Solutions — Architecture Study

### 2.1 Hubble (Cilium) — eBPF-Based Flow Visibility

**Architecture**:
```
┌─────────────────────────────────────────────┐
│  Kernel (per-node)                          │
│  eBPF programs attached to TC/XDP hooks     │
│  → capture packet metadata at L3/L4/L7      │
│  → write to perf event / ring buffer maps   │
└──────────────┬──────────────────────────────┘
               │ perf events
┌──────────────▼──────────────────────────────┐
│  Hubble Server (embedded in Cilium agent)   │
│  - Parses events into Flow objects          │
│  - Enriches with K8s context (pod, ns, svc) │
│  - Stores in user-space ring buffer         │
│  - Serves gRPC API on :4244                 │
└──────────────┬──────────────────────────────┘
               │ gRPC streams
┌──────────────▼──────────────────────────────┐
│  Hubble Relay                               │
│  - Aggregates all node Hubble servers       │
│  - Unified cluster-wide gRPC on :4245       │
└──────────────┬──────────────────────────────┘
               │
    ┌──────────▼──────────┐
    │  Hubble UI / CLI    │
    │  hubble observe     │
    └─────────────────────┘
```

**Key design decisions**:
- **eBPF in-kernel filtering**: Only metadata flows to userspace, not full packets
- **Ring buffer with configurable capacity**: Backpressure = oldest flows evicted
- **gRPC streaming**: `GetFlows` RPC returns `stream Flow` with filter support
- **Visibility scope**: L3/L4 always; L7 (HTTP, gRPC, Kafka, DNS) when enabled via CiliumNetworkPolicy

**Lessons for prb**:
1. Ring buffer (evict-oldest) is the standard backpressure strategy for observability
2. gRPC streaming is the natural API for live flow/packet data
3. Enrichment (adding context to raw events) should be a separate pipeline stage
4. Filter at capture time to reduce volume — BPF for L3/L4, application-level for L7

### 2.2 Wireshark / tshark — Dissector Pipeline

**Architecture**:
```
libpcap/npcap → capture engine → dissector pipeline → tap system → UI/stats
```

**Dissector pipeline**:
1. Frame dissector identifies link type
2. Each dissector processes its layer, registers subdissectors for next layer
3. Dissectors are chained via "dissector tables" (port → dissector, ethertype → dissector)
4. Each dissector populates a protocol tree (hierarchical field data)
5. **Tap interface**: dissectors call `tap_queue_packet()` to expose data to analysis plugins

**Key patterns**:
- **Lazy dissection**: tshark can skip expensive dissection for filtered-out packets
- **Two-pass architecture**: First pass builds packet index; second pass does full dissection
- **Display filters vs capture filters**: BPF at capture; Wireshark's own filter language for display
- **Protocol heuristics**: When port-based identification fails, try each protocol's heuristic check

**Lessons for prb**:
1. Separate capture filtering (BPF, cheap) from display filtering (application, expensive)
2. Protocol identification should be layered: port → heuristic → deep inspection
3. The tap/callback pattern is powerful for pluggable analysis without coupling to the decode pipeline
4. Two-pass is valuable for files but not needed for live streaming

### 2.3 tcpdump / libpcap — PACKET_MMAP

**Architecture**:
```
NIC → driver → PACKET_MMAP ring buffer (kernel) → mmap'd to userspace → libpcap → tcpdump
```

**PACKET_MMAP (TPACKET_V3)**:
- Kernel allocates a ring of blocks; each block contains multiple frames
- `mmap()` maps ring into userspace — no copy on read
- `poll()` to wait for new packets; then walk frames in block
- `tp_status` field per frame indicates kernel-owned vs user-owned
- Block-based retirement: kernel fills a block, marks it ready; userspace processes block, returns it

**Performance**:
- TPACKET_V3 (libpcap ≥1.5): variable-length frames, block-level retirement, reduced syscalls
- Eliminates per-packet `recvfrom()` overhead
- Typical: 1-2M pps on commodity hardware without drops

**Lessons for prb**:
1. libpcap already uses PACKET_MMAP — no need to implement it manually
2. `immediate_mode(true)` is critical for low-latency live capture (flushes per-packet rather than batching)
3. `snaplen` should be large enough for your deepest protocol (gRPC frames can be large)
4. The pcap crate exposes all of these controls

### 2.4 Retina (Stanford) — 100+ Gbps in Rust

**Architecture**:
```
NIC → DPDK (kernel bypass) → Retina core → filter tree → callbacks → output
```

**Key design**:
- Written in Rust with `unsafe` DPDK FFI for line-rate packet reception
- **Compile-time filter optimization**: subscription specifications are compiled into an optimized filter tree that determines which packets to process and how deeply
- **Zero-copy packet pipeline**: packets processed in-place from DPDK mbufs
- **Layered callbacks**: applications subscribe at packet level, connection level, or session level
- **Memory management**: packet data pinned in DPDK hugepage memory

**Performance**: 100-160 Gbps on commodity hardware (single server, 100GbE NIC)

**Lessons for prb**:
1. Subscription-based filtering where the framework only does work proportional to what's subscribed
2. Connection-level and session-level abstractions (not just raw packets) are what users want
3. Compile-time optimization of filter pipelines is powerful but complex
4. For prb's scale, libpcap is fine — Retina's approach is for ISP/backbone monitoring

### 2.5 Retina (Microsoft) — Kubernetes Network Observability

Note: There are two projects named "Retina". Microsoft's Retina is a Kubernetes
network observability platform (Go + eBPF), distinct from Stanford's Retina.

**Architecture**:
- eBPF plugins collect kernel-level network events
- Plugin Manager orchestrates lifecycle: Initialize → Start → Stop
- Data converted to Cilium `flow` objects
- Two control planes: Standard (custom enricher) or Hubble (integrates with Cilium Hubble)
- `packetparser` plugin uses `BPF_MAP_TYPE_PERF_EVENT_ARRAY` for kernel→userspace transfer
- Enrichment adds Kubernetes context (pod, namespace, labels) to flows

**Lessons for prb**:
1. Plugin architecture with standard interface enables extensibility
2. Perf event arrays are the standard eBPF data transfer mechanism
3. Enrichment as a separate pipeline stage is a universal pattern

### 2.6 Zeek (formerly Bro) — Cluster Architecture

**Architecture** (Zeek 8.x, 2025-2026):
```
NIC → AF_PACKET / PF_RING → Workers (protocol analysis)
                                  │
                           ZeroMQ pub/sub
                                  │
                    ┌─────────────┼────────────┐
                    │             │             │
                 Manager      Loggers       Proxies
```

**Key features**:
- **Workers**: Perform all protocol analysis; horizontally scalable
- **ZeroMQ cluster backend** (new in Zeek 8.0, default in 8.1): Replaced Broker for inter-node communication
- **Topic-based pub/sub**: Workers broadcast events without routing through proxies
- **Storage Framework**: SQLite or Redis backends for state persistence
- **Packet distribution**: AF_PACKET `cluster_flow` mode ensures both sides of a connection reach the same worker

**Lessons for prb**:
1. Separate capture threads from analysis workers — this is universal across all production tools
2. Flow-based load balancing (hash on 5-tuple) keeps connections coherent
3. ZeroMQ/pub-sub is the modern pattern for distributing events
4. Zeek's script-based analysis is powerful but slow — Rust's compiled approach is a major advantage

### 2.7 Suricata — High-Performance IDS Capture

**Architecture**:
```
NIC → AF_PACKET (per-thread sockets with ring buffers)
    → Workers mode: each thread runs full detection pipeline
    → cluster_flow: 5-tuple hash distributes packets to threads
```

**Key configuration** (Suricata 8.x):
- One AF_PACKET socket per worker thread
- Ring buffer with configurable descriptor count (typically 1024)
- `poll()` loop processes ring frames until kernel-owned frame encountered
- RSS queue count = worker thread count for optimal distribution
- Symmetric hashing (Toeplitz) ensures bidirectional flow affinity

**Performance tuning**:
- Disable NIC offloading (except RX/TX checksum)
- Pin workers to CPU cores
- Use `cluster_qm` mode when NIC RSS is properly configured
- Recent patches fixed data races between kernel frame init and worker polling

**Lessons for prb**:
1. Workers mode (each thread = full pipeline) is simpler and faster than splitting capture/decode
2. Ring buffer sizing matters — use backpressure models: buffer enough for burst, accept loss at sustained overload
3. CPU pinning and NUMA awareness matter for high-throughput
4. For prb's scale, a single capture thread feeding worker channels is sufficient

### 2.8 Scapy — AsyncSniffer

Python-based; uses `AsyncSniffer` which spawns a background thread. Callback-based
with `prn` parameter. Suffers from memory leaks on repeated instantiation and
thread-safety issues. Relevant only as a UX reference — Scapy's interactive
dissection model is excellent for debugging.

**Lesson for prb**: Callback-based live sniffing with real-time display is what
users expect. The UX should feel like `scapy.sniff(prn=lambda p: p.summary())`.

---

## 3. Key Technical Challenges & Solutions

### 3.1 BPF Filter Compilation and Application

**How it works in the pcap crate**:
```rust
let mut cap = Capture::from_device("eth0")?.open()?;
cap.filter("tcp port 443 and host 10.0.0.1", true)?;
// `true` = optimize the BPF program
// libpcap compiles the filter expression to BPF bytecode
// kernel applies BPF program to each packet before copying to userspace
```

**Available Rust crates for BPF**:
- `pcap` crate: Built-in `filter()` method — uses libpcap's `pcap_compile` + `pcap_setfilter`
- `bpf` crate (v0.1.4): Attach raw BPF programs to sockets (Linux)
- `aya`: Full eBPF framework for writing custom kernel programs
- `rscap`: Rust-native capture with BPF support on Linux/macOS

**Recommendation for prb**: Use pcap's built-in `filter()` for standard BPF filters.
This compiles tcpdump-syntax filter expressions to optimized BPF bytecode. For
advanced per-flow filtering, apply application-level filters in userspace after
parsing headers with etherparse.

### 3.2 Zero-Copy Packet Capture

**Hierarchy of approaches** (increasing performance, increasing complexity):

| Level | Mechanism | pps (single core) | Rust support |
|-------|-----------|-------------------|--------------|
| 1 | `AF_PACKET` + `recvfrom()` | ~300k | `pcap` crate (default on old libpcap) |
| 2 | `PACKET_MMAP` (TPACKET_V3) | ~1-2M | `pcap` crate (libpcap ≥1.5 auto-uses) |
| 3 | `AF_XDP` | ~6.5M | `quick_afxdp`, `xsk-rs` |
| 4 | DPDK (full kernel bypass) | ~15M+ | FFI bindings (Retina-Stanford uses this) |

**For prb**: Level 2 (PACKET_MMAP via libpcap) is the right choice. It's automatic
when using the pcap crate on Linux with modern libpcap. Setting `immediate_mode(true)`
ensures per-packet delivery for interactive use.

### 3.3 Ring Buffer Designs for High-Throughput

**Pattern 1: Kernel ring buffer (PACKET_MMAP)**
- Handled by libpcap — transparent to the application
- Configurable via `cap.buffer_size(bytes)` in the pcap crate

**Pattern 2: Application-level ring buffer for flow events**
Following Hubble's model:
```rust
use std::sync::Arc;
use parking_lot::Mutex;

struct RingBuffer<T> {
    data: Vec<Option<T>>,
    write_pos: usize,
    capacity: usize,
}

impl<T> RingBuffer<T> {
    fn push(&mut self, item: T) {
        self.data[self.write_pos % self.capacity] = Some(item);
        self.write_pos += 1;
        // Oldest items silently evicted — this IS the backpressure strategy
    }
}
```

**Pattern 3: Channel-based pipeline (recommended for prb)**
```rust
// Bounded channel = explicit backpressure
let (tx, rx) = crossbeam_channel::bounded::<CapturedPacket>(8192);

// Capture thread — never blocks on send; drop if full
std::thread::spawn(move || {
    while let Ok(packet) = cap.next_packet() {
        if tx.try_send(packet.to_owned()).is_err() {
            metrics.dropped += 1;
        }
    }
});

// Worker thread(s) — decode and produce events
while let Ok(pkt) = rx.recv() {
    let event = decode_packet(&pkt);
    event_tx.send(event).ok();
}
```

**Recommendation**: Use `crossbeam-channel` bounded channels between pipeline stages.
This gives backpressure (bounded), contention-free MPMC, and better performance than
`std::sync::mpsc`. Size the bound based on expected burst (e.g., 8192 packets ≈
10ms at 1M pps).

### 3.4 Real-Time Protocol Decoding Pipeline

**Recommended architecture for prb**:
```
┌──────────────┐   crossbeam    ┌──────────────────┐   tokio::mpsc   ┌────────────┐
│ Capture      │   bounded(8k)  │ Decode Workers   │   unbounded     │ TUI / API  │
│ Thread       ├───────────────►│ (thread pool)    ├────────────────►│ Renderer   │
│              │                │                  │                 │            │
│ pcap + BPF   │                │ etherparse →     │                 │ ratatui    │
│ immediate    │                │ TCP reassembly → │                 │ or gRPC    │
│ mode         │                │ TLS decrypt →    │                 │ stream     │
│              │                │ gRPC/proto decode│                 │            │
└──────────────┘                └──────────────────┘                 └────────────┘
```

**Pipeline stages**:
1. **Capture**: Single thread, pcap crate, BPF filter, immediate mode
2. **Parse**: etherparse `SlicedPacket` — zero-alloc header extraction
3. **Reassemble**: TCP stream reassembly (prb already has this in `prb-pcap`)
4. **Decrypt**: TLS decryption where keys available (prb already has this)
5. **Decode**: Protocol-specific decoders (gRPC, ZMQ, DDS — prb already has these)
6. **Display**: TUI rendering or gRPC stream emission

### 3.5 Terminal UI for Live Streaming

**Crate stack**:
- `ratatui` v0.29+ — immediate-mode TUI framework
- `crossterm` — cross-platform terminal backend

**Architecture pattern for live capture TUI**:
```rust
use ratatui::prelude::*;
use crossterm::event::{self, Event, KeyCode};
use tokio::sync::mpsc;

struct App {
    packets: Vec<PacketSummary>,    // ring buffer of recent packets
    selected: usize,
    detail_view: Option<PacketDetail>,
    filter: String,
    stats: CaptureStats,
}

// Event loop with separate rates for data and rendering
enum AppEvent {
    Packet(PacketSummary),
    Key(KeyCode),
    Tick,                           // UI refresh interval
}

async fn run(mut rx: mpsc::Receiver<AppEvent>) {
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    loop {
        terminal.draw(|f| ui(f, &app))?;
        match rx.recv().await {
            Some(AppEvent::Packet(p)) => app.push_packet(p),
            Some(AppEvent::Key(k)) => handle_key(&mut app, k),
            Some(AppEvent::Tick) => {},
            None => break,
        }
    }
}
```

**Key patterns**:
- Separate tick rate (UI refresh, e.g. 30fps) from data rate (packets arrive continuously)
- Use `crossterm::event::EventStream` for async key input
- Ring buffer the display list (keep last N packets, evict oldest)
- Table widget for packet list; Paragraph/Block widgets for detail view
- Sparkline widget for packet rate graph

### 3.6 Async Capture with Tokio Integration

**Option A: pcap's built-in `capture-stream` (recommended)**
```toml
[dependencies]
pcap = { version = "2.4", features = ["capture-stream"] }
```
Uses tokio's `AsyncFd` to poll the pcap file descriptor. True async — no extra thread.
Requires `setnonblock()` on the capture handle.

**Option B: Dedicated capture thread + channel**
```rust
let (tx, mut rx) = tokio::sync::mpsc::channel(8192);

std::thread::spawn(move || {
    while let Ok(packet) = cap.next_packet() {
        let owned = OwnedPacket::from(packet);
        if tx.blocking_send(owned).is_err() { break; }
    }
});

// In async context:
while let Some(pkt) = rx.recv().await {
    process(pkt).await;
}
```

**Option B is preferred** when:
- You need to do blocking operations (e.g., pcap with timeout mode)
- You want isolation between capture and async processing
- You're using bounded channels for backpressure

**Option A is preferred** when:
- You want minimal overhead and no extra thread
- Your processing is lightweight and won't cause backpressure
- You're already in a tokio context

**Recommendation for prb**: Option B (dedicated thread + channel). The capture
thread should be a real OS thread to ensure it never misses packets due to
async task scheduling. The rest of the pipeline can be async.

### 3.7 Privilege Management

**Linux capabilities** (preferred over running as root):
```bash
# After building:
sudo setcap cap_net_raw,cap_net_admin=eip target/release/prb

# Verify:
getcap target/release/prb
# target/release/prb cap_net_admin,cap_net_raw=eip
```

**Required capabilities**:
- `CAP_NET_RAW`: Create raw/packet sockets (required for libpcap)
- `CAP_NET_ADMIN`: Set promiscuous mode, configure interfaces

**Privilege dropping pattern**:
```rust
fn setup_capture() -> Result<Capture<Active>> {
    // Open capture (requires CAP_NET_RAW)
    let cap = Capture::from_device("eth0")?
        .promisc(true)
        .open()?;

    // Drop privileges after opening the capture device
    // (file descriptor persists even after capability drop)
    drop_privileges()?;

    Ok(cap)
}

fn drop_privileges() -> Result<()> {
    use caps::{CapSet, Capability};
    caps::clear(None, CapSet::Effective)?;
    caps::clear(None, CapSet::Permitted)?;
    Ok(())
}
```

**Container deployment**:
```yaml
securityContext:
  capabilities:
    add: ["NET_RAW", "NET_ADMIN"]
    drop: ["ALL"]
```

**Filesystem caveat**: Capabilities fail on `nosuid` mounts (common for encrypted
home dirs on Ubuntu). Deploy binaries to `/usr/local/bin` or similar.

**Recommendation for prb**:
1. Detect if running as root or with capabilities at startup
2. If neither, print a clear error message with the `setcap` command to run
3. After opening capture device, drop all capabilities
4. Support `--interface` flag to enumerate available interfaces (requires CAP_NET_ADMIN)

### 3.8 Interface Enumeration and Selection

```rust
use pcap::Device;

fn list_interfaces() -> Vec<InterfaceInfo> {
    Device::list()
        .unwrap_or_default()
        .into_iter()
        .map(|d| InterfaceInfo {
            name: d.name.clone(),
            description: d.desc.unwrap_or_default(),
            addresses: d.addresses.iter().map(|a| a.addr).collect(),
            flags: d.flags,
            is_loopback: d.flags.is_loopback(),
            is_up: d.flags.is_up(),
            is_running: d.flags.is_running(),
        })
        .collect()
}
```

The pcap crate provides `Device::list()` which returns all interfaces with
addresses, flags (up/running/loopback/wireless), and descriptions.

---

## 4. Recommended Crate Matrix for prb

### Core Dependencies

| Purpose | Crate | Version | Notes |
|---------|-------|---------|-------|
| Packet capture | `pcap` | 2.4 | Features: `capture-stream` |
| Packet parsing | `etherparse` | 0.19.0 | Already in workspace |
| BPF filters | (via `pcap`) | — | `cap.filter("...", true)` |
| Async runtime | `tokio` | 1.x | Features: `full` |
| Channels | `crossbeam-channel` | 0.5 | Bounded MPMC for pipeline |
| TUI | `ratatui` | 0.29+ | Immediate-mode rendering |
| Terminal backend | `crossterm` | 0.28+ | Cross-platform terminal I/O |
| gRPC | `tonic` | 0.12+ | For streaming API |
| Protobuf | `prost` | 0.13+ | Compile-time codegen |
| Dynamic protobuf | `prost-reflect` | 0.14+ | Runtime message decoding |
| gRPC reflection | `tonic-reflection` | 0.12+ | Service discovery |

### Supporting Dependencies

| Purpose | Crate | Version | Notes |
|---------|-------|---------|-------|
| Serialization | `serde` | 1 | Already in workspace |
| Error handling | `thiserror` | 2 | Already in workspace |
| Logging | `tracing` | 0.1 | Already in workspace |
| CLI | `clap` | 4 | Already in workspace |
| Linux capabilities | `caps` | 0.5 | Privilege management |
| Metrics | `metrics` | 0.24 | Capture stats, drop counts |

### Future / Advanced (not needed initially)

| Purpose | Crate | Version | Notes |
|---------|-------|---------|-------|
| eBPF | `aya` | 0.13+ | In-kernel filtering, flow tracking |
| AF_XDP | `quick_afxdp` | 0.4 | Zero-copy kernel bypass |
| Raw sockets | `pnet` | 0.35 | Only if needed for injection |
| Async pcap | `async-pcap` | 0.1.6 | Simpler alternative to capture-stream |

---

## 5. Architecture Patterns & Recommendations

### 5.1 Recommended Pipeline Architecture for prb

```
                    ┌─────────────────────────────────────────────────────────────┐
                    │                     prb-capture crate                       │
                    │                                                             │
User selects ──────►  Interface ──► pcap::Capture  ──► BPF filter               │
interface          │  Enumeration   (live, promisc,    (compiled from             │
                    │                immediate_mode)    user expression)           │
                    └───────────────────┬─────────────────────────────────────────┘
                                        │  OS Thread (blocking pcap::next_packet)
                                        │
                              crossbeam::bounded(8192)
                                        │
                    ┌───────────────────▼─────────────────────────────────────────┐
                    │                  prb-pcap crate (existing)                  │
                    │                                                             │
                    │  etherparse::SlicedPacket ──► NormalizedPacket              │
                    │                               │                             │
                    │                    ┌──────────┴──────────┐                  │
                    │                    │                     │                  │
                    │               UDP packets          TCP segments             │
                    │                    │                     │                  │
                    │                    │            TCP Reassembly              │
                    │                    │            (existing)                   │
                    │                    │                     │                  │
                    │                    │            TLS Decrypt                 │
                    │                    │            (existing, if keys)         │
                    │                    │                     │                  │
                    │                    ▼                     ▼                  │
                    │              ┌─────────────────────────────────┐            │
                    │              │  Protocol Decoders (existing)   │            │
                    │              │  gRPC │ ZMQ │ DDS │ raw        │            │
                    │              └──────────────┬──────────────────┘            │
                    └─────────────────────────────┼──────────────────────────────┘
                                                  │
                                     tokio::mpsc::channel(4096)
                                                  │
                    ┌─────────────────────────────▼──────────────────────────────┐
                    │                    Output Layer                             │
                    │                                                             │
                    │  ┌─────────────┐  ┌─────────────┐  ┌───────────────────┐  │
                    │  │ TUI (live)  │  │ File (pcap) │  │ gRPC stream API   │  │
                    │  │ ratatui +   │  │ append to   │  │ tonic server      │  │
                    │  │ crossterm   │  │ savefile    │  │ GetPackets stream │  │
                    │  └─────────────┘  └─────────────┘  └───────────────────┘  │
                    └────────────────────────────────────────────────────────────┘
```

### 5.2 How Production Tools Handle Backpressure

| Tool | Strategy | Mechanism |
|------|----------|-----------|
| **Hubble** | Evict oldest | Fixed-size ring buffer; newest overwrites oldest |
| **tcpdump** | Drop at kernel | PACKET_MMAP ring full → kernel drops + counts |
| **Suricata** | Drop at ring | AF_PACKET ring full → frame skipped |
| **Wireshark** | Drop at capture | libpcap reports drops via `pcap_stats()` |
| **Zeek** | Drop at capture | AF_PACKET drops; Zeek logs drop counts |

**Universal pattern**: All production tools accept packet loss under sustained overload.
They size buffers for expected burst durations and report drop statistics. None use
unbounded queues or apply TCP-style backpressure to the network.

**For prb**:
1. Kernel ring buffer (via libpcap) handles burst absorption — configure with `cap.buffer_size()`
2. Application channel (crossbeam bounded) handles pipeline backpressure — `try_send()` + drop count
3. Display ring buffer handles UI backpressure — keep last N events, evict oldest
4. Expose drop statistics at every stage: kernel drops (`pcap_stats()`), channel drops, display drops

### 5.3 Ring Buffer vs Channel-Based Designs

**Ring buffer** (single-writer, single/multi-reader):
- Pro: Lock-free, cache-friendly, fixed memory
- Pro: Natural for "last N items" windowed display
- Con: No backpressure signal to producer
- Best for: Final display buffer, event storage

**Channels** (MPSC/MPMC):
- Pro: Backpressure via bounded send
- Pro: Multiple consumers (MPMC) for parallel decode
- Pro: Clean ownership transfer (Rust-friendly)
- Con: Allocation overhead (unless using arena)
- Best for: Pipeline stages where you want to signal overload

**Recommendation**: Use channels between pipeline stages (capture → decode → output)
and a ring buffer for the final display layer (last N packets shown in TUI).

### 5.4 Multi-Threaded Capture + Decode Pipeline

**Simple model** (recommended for prb):
```
1 capture thread → bounded channel → N decode tasks (tokio) → event channel → 1 TUI thread
```

**Advanced model** (for >100k pps):
```
M capture threads (one per interface/RSS queue)
    → M bounded channels
    → flow-based routing (hash on 5-tuple)
    → N decode workers (pinned to cores)
    → event aggregation
    → 1 output thread
```

For prb's debugging use case, the simple model is appropriate. The capture thread
should be a real OS thread (not a tokio task) to guarantee it doesn't miss packets.
Decode can use tokio tasks since it's CPU-bound but not latency-critical.

### 5.5 gRPC/Protobuf Dynamic Decoding

For decoding captured gRPC traffic where proto definitions may not be known at
compile time, use `prost-reflect`:

```rust
use prost_reflect::{DescriptorPool, DynamicMessage};

// Load proto descriptors at runtime
let pool = DescriptorPool::decode(proto_descriptor_bytes)?;
let msg_desc = pool.get_message_by_name("mypackage.MyRequest")?;

// Decode captured gRPC body (after HTTP/2 frame + gRPC length-prefix stripping)
let message = DynamicMessage::decode(msg_desc, payload_bytes)?;

// Access fields dynamically
let field = message.get_field_by_name("user_id").unwrap();
println!("user_id = {}", field.as_str().unwrap());

// Or serialize to JSON for display
let json = serde_json::to_string_pretty(&message)?;
```

This is critical for prb's gRPC debugging: users provide `.proto` files or
file descriptor sets, and prb decodes captured gRPC bodies into human-readable
form in real time.

---

## 6. Summary: Architecture Recommendations for prb Live Capture

### Phase 1: Basic Live Capture (new `prb-capture` crate)

1. **Add `pcap` v2.4 dependency** with `capture-stream` feature
2. **Capture thread**: Dedicated OS thread using `pcap::Capture` in blocking mode
   with `immediate_mode(true)` and optional BPF filter
3. **Channel to existing pipeline**: `crossbeam-channel::bounded(8192)` feeds
   `NormalizedPacket` to the existing `prb-pcap` decode pipeline
4. **Interface selection**: CLI flag `--interface` / `-i` with `Device::list()` for enumeration
5. **Privilege check**: Detect capabilities at startup, print `setcap` instructions if missing
6. **Statistics**: Track and display kernel drops (`pcap_stats()`) and channel drops

### Phase 2: TUI Live View

1. **Add `ratatui` + `crossterm`** dependencies
2. **Packet list**: Table widget showing timestamp, src/dst, protocol, summary
3. **Detail view**: Selected packet expanded to show decoded protocol fields
4. **Live rate**: Sparkline showing packets/second
5. **Filtering**: Application-level display filter (in addition to BPF capture filter)
6. **Event loop**: Separate tick rate (30fps render) from packet rate

### Phase 3: Streaming API

1. **Add `tonic` + `prost`** dependencies
2. **gRPC service**: `GetPackets` RPC returning `stream PacketEvent`
3. **Filter in request**: Client specifies BPF-like filter in the RPC
4. **Backpressure**: gRPC flow control handles slow clients; server-side ring buffer

### Future: Advanced Capture

- **eBPF via `aya`**: In-kernel packet filtering, flow tracking, L7 visibility
- **AF_XDP via `quick_afxdp`**: Zero-copy for >1M pps workloads
- **Multi-interface capture**: One thread per interface with flow-based routing

---

## Appendix A: Version Matrix (as of March 2026)

| Crate | Latest | MSRV | License |
|-------|--------|------|---------|
| `pcap` | 2.4.0 | stable | MIT/Apache-2.0 |
| `etherparse` | 0.19.0 | stable | MIT/Apache-2.0 |
| `pnet_packet` | 0.35.0 | stable | MIT/Apache-2.0 |
| `ratatui` | 0.29.x | 1.74+ | MIT |
| `crossterm` | 0.28.x | 1.58+ | MIT |
| `tonic` | 0.12.x | 1.70+ | MIT |
| `prost` | 0.13.x | 1.70+ | Apache-2.0 |
| `prost-reflect` | 0.14.x | 1.70+ | Apache-2.0 |
| `aya` | 0.13.x | 1.80+ | MIT/Apache-2.0 |
| `quick_afxdp` | 0.4.0 | stable | MIT |
| `crossbeam-channel` | 0.5.x | 1.61+ | MIT/Apache-2.0 |
| `tokio` | 1.x | 1.70+ | MIT |
| `async-pcap` | 0.1.6 | stable | MIT |
| `caps` | 0.5.x | stable | MIT/Apache-2.0 |

## Appendix B: Key Differences from prb's Current pcap-parser Approach

prb currently uses `pcap-parser` (v0.17.0) for **offline** pcap file parsing.
For **live** capture, the approach is fundamentally different:

| Aspect | Offline (current) | Live (new) |
|--------|-------------------|------------|
| **Crate** | `pcap-parser` | `pcap` |
| **Source** | File on disk | Network interface |
| **Timing** | Process at file read speed | Real-time, packet arrival rate |
| **Backpressure** | N/A (file waits) | Ring buffers + drop counting |
| **Privileges** | None needed | `CAP_NET_RAW` + `CAP_NET_ADMIN` |
| **BPF filters** | N/A (filter in software) | Kernel-level BPF before copy |
| **Threading** | Single-threaded OK | Dedicated capture thread required |

The existing `prb-pcap` normalize + decode pipeline can be reused unchanged.
The new `prb-capture` crate would provide the live packet source that feeds
into the same `NormalizedPacket` → decoder pipeline.
