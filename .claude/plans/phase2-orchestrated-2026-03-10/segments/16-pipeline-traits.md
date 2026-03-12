---
segment: 16
title: "Pipeline Trait Refactoring"
depends_on: []
risk: 9
complexity: High
cycle_budget: 4
status: pending
commit_message: "refactor(prb-pcap): add OwnedNormalizedPacket, FlowKey, BatchStage/StreamStage traits, ParallelPipeline orchestrator"
---

# Subsection 1: Pipeline Trait Refactoring

## Purpose

Make pipeline stage types `Send + Sync`-compatible so they can be used across
rayon's thread pool. Introduce a `PipelineStage` trait that standardizes the
interface between stages and enables both batch (rayon) and streaming (channel)
execution modes.

---

## Current Problems

1. **`NormalizedPacket` borrows input data** — `payload: &'a [u8]` prevents
   sending across threads. Need an owned variant for parallel paths.

2. **`PacketNormalizer` is not `Send`** — `defrag_pool` uses `etherparse`'s
   `IpDefragPool` which stores references with interior mutability. Cannot be
   shared across rayon threads.

3. **`TcpReassembler` is a monolith** — Single `HashMap<ConnectionKey, ConnectionState>`
   holds all connections. Must be split into per-flow instances for sharding.

4. **`PcapCaptureAdapter` owns everything** — `process_all_packets` is one
   monolithic method. Stages aren't composable or independently testable.

5. **`CaptureAdapter` returns `&mut self` iterator** — Prevents concurrent
   consumption.

---

## Segment S1.1: Owned Packet Types

### `OwnedNormalizedPacket`

Create an owned variant of `NormalizedPacket` for cross-thread transfer:

```rust
// crates/prb-pcap/src/normalize.rs

#[derive(Debug, Clone)]
pub struct OwnedNormalizedPacket {
    pub timestamp_us: u64,
    pub src_ip: IpAddr,
    pub dst_ip: IpAddr,
    pub transport: TransportInfo,
    pub vlan_id: Option<u16>,
    pub payload: Vec<u8>,
}

impl OwnedNormalizedPacket {
    pub fn from_borrowed(packet: &NormalizedPacket<'_>) -> Self {
        Self {
            timestamp_us: packet.timestamp_us,
            src_ip: packet.src_ip,
            dst_ip: packet.dst_ip,
            transport: packet.transport.clone(),
            vlan_id: packet.vlan_id,
            payload: packet.payload.to_vec(),
        }
    }
}
```

### `FlowKey` — hashable, `Send + Sync`

Extract and promote `ConnectionKey` to a public `FlowKey` type:

```rust
// crates/prb-pcap/src/flow_key.rs (new file)

use std::hash::{Hash, Hasher};
use std::net::IpAddr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FlowKey {
    pub lo_ip: IpAddr,
    pub lo_port: u16,
    pub hi_ip: IpAddr,
    pub hi_port: u16,
    pub protocol: FlowProtocol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowProtocol {
    Tcp,
    Udp,
}

impl FlowKey {
    /// Creates a canonical flow key where (lo, hi) is deterministic regardless
    /// of packet direction. This ensures packets in both directions of a
    /// connection hash to the same shard.
    pub fn from_packet(packet: &OwnedNormalizedPacket) -> Option<Self> {
        let (src_port, dst_port, protocol) = match &packet.transport {
            TransportInfo::Tcp(tcp) => (tcp.src_port, tcp.dst_port, FlowProtocol::Tcp),
            TransportInfo::Udp { src_port, dst_port } => (*src_port, *dst_port, FlowProtocol::Udp),
            TransportInfo::Other(_) => return None,
        };

        let (lo_ip, lo_port, hi_ip, hi_port) =
            if (packet.src_ip, src_port) <= (packet.dst_ip, dst_port) {
                (packet.src_ip, src_port, packet.dst_ip, dst_port)
            } else {
                (packet.dst_ip, dst_port, packet.src_ip, src_port)
            };

        Some(Self { lo_ip, lo_port, hi_ip, hi_port, protocol })
    }

    /// Returns a shard index in [0, num_shards) using deterministic hashing.
    pub fn shard_index(&self, num_shards: usize) -> usize {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        (hasher.finish() as usize) % num_shards
    }
}
```

### Why canonical ordering matters

TCP connections are bidirectional. A packet from `10.0.0.1:50051 → 10.0.0.2:8080`
and a packet from `10.0.0.2:8080 → 10.0.0.1:50051` belong to the same flow.
By sorting the endpoints into (lo, hi) order, both directions hash identically.

---

## Segment S1.2: PipelineStage Trait

Define a trait that all pipeline stages implement:

```rust
// crates/prb-pcap/src/parallel/mod.rs (new module)

pub trait BatchStage: Send + Sync {
    type Input: Send;
    type Output: Send;

    fn process_batch(&self, input: Vec<Self::Input>) -> Vec<Self::Output>;
}

pub trait StreamStage: Send {
    type Input: Send;
    type Output: Send;

    fn process_one(&mut self, input: Self::Input) -> Vec<Self::Output>;
    fn flush(&mut self) -> Vec<Self::Output>;
}
```

`BatchStage` is for stateless stages (normalization, TLS decrypt, protocol
decode) that can process items independently. Takes `&self` — can be shared
across rayon threads.

`StreamStage` is for stateful stages (TCP reassembly) that need per-item
ordering. Takes `&mut self` — one instance per shard.

### Stage implementations

| Stage | Trait | Notes |
|-------|-------|-------|
| `NormalizeStage` | `BatchStage` | Non-fragmented packets only |
| `FragmentCollector` | `StreamStage` | IP fragment reassembly (single-threaded) |
| `TcpReassemblyStage` | `StreamStage` | Per-shard, one per flow partition |
| `TlsDecryptStage` | `BatchStage` | Shares `Arc<TlsKeyLog>` |
| `ProtocolDecodeStage` | `BatchStage` | Creates fresh decoder per stream |
| `EventBuildStage` | `BatchStage` | Stateless stream→event conversion |

---

## Segment S1.3: Pipeline Orchestrator

The `ParallelPipeline` struct replaces the monolithic `process_all_packets`:

```rust
// crates/prb-pcap/src/parallel/orchestrator.rs

pub struct ParallelPipeline {
    num_shards: usize,
    tls_keylog: Arc<TlsKeyLog>,
    capture_path: PathBuf,
}

pub struct PipelineConfig {
    pub jobs: usize,       // 0 = auto-detect, 1 = sequential
    pub batch_size: usize, // packets per normalization batch (default 4096)
    pub shard_count: usize, // 0 = auto (2 * num_cpus)
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            jobs: 0,
            batch_size: 4096,
            shard_count: 0,
        }
    }
}

impl ParallelPipeline {
    pub fn new(config: PipelineConfig, tls_keylog: Arc<TlsKeyLog>, capture_path: PathBuf) -> Self {
        let num_shards = if config.shard_count == 0 {
            std::thread::available_parallelism()
                .map(|n| n.get() * 2)
                .unwrap_or(8)
        } else {
            config.shard_count
        };

        Self { num_shards, tls_keylog, capture_path }
    }

    pub fn run(&self, packets: Vec<PcapPacket>) -> Result<Vec<DebugEvent>, CoreError> {
        // Phase 1: Parallel normalize
        let (normalized, fragments) = self.parallel_normalize(&packets);

        // Phase 1b: Single-threaded fragment reassembly
        let defragged = self.process_fragments(fragments, &packets);
        let all_normalized = [normalized, defragged].concat();

        // Phase 2: Partition by flow
        let shards = self.partition_by_flow(all_normalized);

        // Phase 3: Parallel per-shard processing (TCP reasm + TLS + decode)
        let shard_events: Vec<Vec<DebugEvent>> = shards
            .into_par_iter()
            .map(|shard| self.process_shard(shard))
            .collect();

        // Phase 4: Merge and sort by timestamp
        let mut events: Vec<DebugEvent> = shard_events.into_iter().flatten().collect();
        events.sort_by_key(|e| e.timestamp);

        Ok(events)
    }
}
```

### Adaptive parallelism

For small captures (<10,000 packets), the overhead of rayon's thread pool
startup and synchronization exceeds the benefit. The orchestrator detects this
and falls back to the existing sequential path:

```rust
impl ParallelPipeline {
    const PARALLEL_THRESHOLD: usize = 10_000;

    pub fn run(&self, packets: Vec<PcapPacket>) -> Result<Vec<DebugEvent>, CoreError> {
        if packets.len() < Self::PARALLEL_THRESHOLD || self.num_shards == 1 {
            return self.run_sequential(packets);
        }
        self.run_parallel(packets)
    }
}
```

---

## Files Changed

| File | Change |
|------|--------|
| `crates/prb-pcap/src/normalize.rs` | Add `OwnedNormalizedPacket` |
| `crates/prb-pcap/src/flow_key.rs` | New: `FlowKey`, `FlowProtocol` |
| `crates/prb-pcap/src/parallel/mod.rs` | New: `BatchStage`, `StreamStage` traits |
| `crates/prb-pcap/src/parallel/orchestrator.rs` | New: `ParallelPipeline`, `PipelineConfig` |
| `crates/prb-pcap/src/lib.rs` | Add `pub mod flow_key; pub mod parallel;` |
| `crates/prb-pcap/Cargo.toml` | Add `rayon = "1.11"` |

---

## Tests

- `test_flow_key_canonical_ordering` — (A→B) and (B→A) produce same FlowKey
- `test_flow_key_different_flows` — Different 5-tuples produce different keys
- `test_flow_key_shard_deterministic` — Same FlowKey always maps to same shard
- `test_owned_normalized_roundtrip` — Borrowed→Owned preserves all fields
- `test_pipeline_config_defaults` — Default config uses auto-detect
- `test_parallel_threshold` — <10k packets uses sequential path
- `test_parallel_pipeline_matches_sequential` — Same input produces identical
  output regardless of parallelism level (the critical correctness test)
