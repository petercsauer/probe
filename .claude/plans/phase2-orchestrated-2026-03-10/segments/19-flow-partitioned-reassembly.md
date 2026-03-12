---
segment: 19
title: "Flow-Partitioned TCP Reassembly"
depends_on: [18]
risk: 9
complexity: High
cycle_budget: 4
status: pending
commit_message: "feat(prb-pcap): add flow partitioner and parallel per-shard TCP reassembly"
---

# Subsection 3: Flow-Partitioned TCP Reassembly

## Purpose

Partition normalized TCP packets by flow (5-tuple) and process each partition
with its own `TcpReassembler` instance in parallel. This is the highest-impact
parallelization because TCP reassembly is the most stateful and CPU-intensive
stage.

---

## Architecture

```
Vec<OwnedNormalizedPacket>
         │
         ▼
  ┌──────────────────────┐
  │  Flow Partitioner     │  Hash FlowKey → shard[i]
  │  scatter_by_flow()    │  Preserves per-flow packet order
  └──────────┬───────────┘
             │ Vec<Vec<OwnedNormalizedPacket>>  (one vec per shard)
             ▼
  ┌──────────────────────────────────────────────┐
  │  rayon::par_iter over shards                  │
  │                                               │
  │  Shard 0:                                     │
  │    TcpReassembler::new() ← fresh per shard    │
  │    for packet in shard_packets:               │
  │      reassembler.process_segment(packet)      │
  │    reassembler.flush_all() ← cleanup          │
  │                                               │
  │  Shard 1: (same, independent)                 │
  │  ...                                          │
  │  Shard N: (same, independent)                 │
  └──────────────────┬───────────────────────────┘
                     │ Vec<Vec<ReassembledStream>>
                     ▼
           flatten + collect
```

---

## Segment S3.1: Flow Partitioner

```rust
// crates/prb-pcap/src/parallel/partition.rs

use crate::flow_key::FlowKey;
use crate::normalize::OwnedNormalizedPacket;

pub struct FlowPartitioner {
    num_shards: usize,
}

impl FlowPartitioner {
    pub fn new(num_shards: usize) -> Self {
        assert!(num_shards > 0, "need at least 1 shard");
        Self { num_shards }
    }

    /// Partitions packets into shards by flow key. Packets without a recognized
    /// transport protocol (Other) go to shard 0.
    ///
    /// Within each shard, packets maintain their original relative order
    /// (stable partitioning). This is critical for TCP reassembly correctness.
    pub fn partition(
        &self,
        packets: Vec<OwnedNormalizedPacket>,
    ) -> Vec<Vec<OwnedNormalizedPacket>> {
        let mut shards: Vec<Vec<OwnedNormalizedPacket>> =
            (0..self.num_shards).map(|_| Vec::new()).collect();

        for packet in packets {
            let shard_idx = FlowKey::from_packet(&packet)
                .map(|k| k.shard_index(self.num_shards))
                .unwrap_or(0);
            shards[shard_idx].push(packet);
        }

        shards
    }
}
```

### Why stable partitioning matters

TCP reassembly depends on packet arrival order. If packets within the same
flow arrive out of order, the reassembler may:
1. Fail to track the initial sequence number correctly
2. Emit partial streams at wrong boundaries
3. Misidentify connection direction

By iterating `packets` in order and appending to shard vectors, we preserve
the original capture ordering within each flow.

---

## Segment S3.2: TcpReassembler Adaptation for Owned Packets

The existing `TcpReassembler::process_segment` takes `&NormalizedPacket<'_>`
(borrowed). For the parallel pipeline, we need a variant that works with
`OwnedNormalizedPacket`.

Two approaches:
1. **Duplicate the API** — add `process_owned_segment(&mut self, packet: &OwnedNormalizedPacket)`
2. **Generalize with a trait** — abstract over borrowed vs owned

Choose approach 1 (simpler, avoids trait complexity):

```rust
// crates/prb-pcap/src/tcp.rs — add method

impl TcpReassembler {
    /// Process a TCP segment from an owned normalized packet.
    /// Semantically identical to process_segment but works with owned data.
    pub fn process_owned_segment(
        &mut self,
        packet: &OwnedNormalizedPacket,
    ) -> Result<Vec<StreamEvent>, PcapError> {
        let tcp_info = match &packet.transport {
            TransportInfo::Tcp(info) => info,
            _ => return Ok(Vec::new()),
        };

        // Same logic as process_segment, using packet.src_ip, packet.dst_ip, etc.
        // ... (extracted into shared helper)
    }

    /// Flushes all active connections, emitting any buffered data.
    /// Called at end of shard processing.
    pub fn flush_all(&mut self) -> Vec<StreamEvent> {
        let keys: Vec<ConnectionKey> = self.connections.keys().cloned().collect();
        let mut events = Vec::new();

        for key in keys {
            if let Some(state) = self.connections.remove(&key) {
                if let Some(c2s) = Self::create_flush_event(
                    &key, &state.client_to_server, StreamDirection::ClientToServer,
                ) {
                    events.push(StreamEvent::Data(c2s));
                }
                if let Some(s2c) = Self::create_flush_event(
                    &key, &state.server_to_client, StreamDirection::ServerToClient,
                ) {
                    events.push(StreamEvent::Data(s2c));
                }
            }
        }

        events
    }
}
```

### Shared helper extraction

Factor the core reassembly logic into a private helper that works with
field references rather than the full packet type:

```rust
fn process_segment_inner(
    &mut self,
    src_ip: IpAddr,
    dst_ip: IpAddr,
    tcp_info: &TcpSegmentInfo,
    payload: &[u8],
    timestamp_us: u64,
) -> Result<Vec<StreamEvent>, PcapError> {
    // ... existing logic from process_segment, using passed-in fields
}
```

Both `process_segment` and `process_owned_segment` delegate to this helper.

---

## Segment S3.3: Parallel Shard Processing

```rust
// crates/prb-pcap/src/parallel/shard.rs

use rayon::prelude::*;

pub struct ShardProcessor {
    tls_keylog: Arc<TlsKeyLog>,
    capture_path: PathBuf,
}

impl ShardProcessor {
    /// Processes all shards in parallel. Each shard gets its own
    /// TcpReassembler and TlsStreamProcessor.
    pub fn process_shards(
        &self,
        shards: Vec<Vec<OwnedNormalizedPacket>>,
    ) -> Vec<Vec<DebugEvent>> {
        shards
            .into_par_iter()
            .map(|shard_packets| self.process_single_shard(shard_packets))
            .collect()
    }

    fn process_single_shard(
        &self,
        packets: Vec<OwnedNormalizedPacket>,
    ) -> Vec<DebugEvent> {
        let mut reassembler = TcpReassembler::new();
        let mut tls_processor = TlsStreamProcessor::with_keylog_ref(
            Arc::clone(&self.tls_keylog),
        );
        let mut events = Vec::new();

        for packet in &packets {
            match &packet.transport {
                TransportInfo::Tcp(_) => {
                    match reassembler.process_owned_segment(packet) {
                        Ok(stream_events) => {
                            for stream_event in stream_events {
                                if let StreamEvent::Data(stream) = stream_event {
                                    self.process_stream(
                                        stream,
                                        &mut tls_processor,
                                        &mut events,
                                    );
                                }
                            }
                        }
                        Err(e) => tracing::warn!("TCP reassembly: {}", e),
                    }
                }
                TransportInfo::Udp { src_port, dst_port } => {
                    events.push(create_udp_event(packet, *src_port, *dst_port, &self.capture_path));
                }
                TransportInfo::Other(_) => {}
            }
        }

        // Flush remaining TCP connections
        for stream_event in reassembler.flush_all() {
            if let StreamEvent::Data(stream) = stream_event {
                self.process_stream(stream, &mut tls_processor, &mut events);
            }
        }

        events
    }
}
```

### TlsStreamProcessor with shared keylog

Currently `TlsStreamProcessor` owns a `TlsKeyLog`. For parallel shards, the
keylog must be shared. Add a constructor that takes `Arc<TlsKeyLog>`:

```rust
// crates/prb-pcap/src/tls/mod.rs

pub struct TlsStreamProcessor {
    keylog: Arc<TlsKeyLog>,
}

impl TlsStreamProcessor {
    pub fn new() -> Self {
        Self { keylog: Arc::new(TlsKeyLog::new()) }
    }

    pub fn with_keylog(keylog: TlsKeyLog) -> Self {
        Self { keylog: Arc::new(keylog) }
    }

    pub fn with_keylog_ref(keylog: Arc<TlsKeyLog>) -> Self {
        Self { keylog }
    }
}
```

`TlsKeyLog::lookup` takes `&self` and only reads — already safe to share via
`Arc`. No `Mutex` needed.

---

## Shard Count Selection

| Strategy | Formula | Rationale |
|----------|---------|-----------|
| Default | `2 * num_cpus` | Over-subscription handles uneven shard sizes |
| Conservative | `num_cpus` | Less overhead, good for uniform traffic |
| Large captures | `4 * num_cpus` | More shards = finer granularity, less elephant-flow risk |

The `2 * num_cpus` default is based on:
- rayon's work-stealing handles load imbalance
- Over-subscription ensures no core idles when one shard finishes early
- Stanford Retina uses similar ratios for 100Gbps processing

---

## Edge Cases

| Case | Handling |
|------|---------|
| Single flow (all packets same 5-tuple) | All packets land in one shard; degrades to sequential for that flow. Acceptable — this is the "elephant flow" case. |
| All UDP (no TCP) | No reassembly needed; partition still works but each packet is independent. Could skip sharding. |
| No packets | Empty shards, empty output. |
| Packets without transport (Other) | Go to shard 0. Ignored by reassembler. |
| Shard count > flow count | Some shards empty. rayon handles gracefully. |

---

## Files Changed

| File | Change |
|------|--------|
| `crates/prb-pcap/src/parallel/partition.rs` | New: `FlowPartitioner` |
| `crates/prb-pcap/src/parallel/shard.rs` | New: `ShardProcessor` |
| `crates/prb-pcap/src/tcp.rs` | Add `process_owned_segment`, `flush_all`, extract helper |
| `crates/prb-pcap/src/tls/mod.rs` | Change `TlsStreamProcessor` to use `Arc<TlsKeyLog>` |

---

## Tests

- `test_partition_single_flow` — All packets go to one shard
- `test_partition_two_flows` — Packets split across shards by 5-tuple
- `test_partition_preserves_order` — Within a shard, packet timestamps are
  monotonically non-decreasing
- `test_partition_bidirectional` — A→B and B→A packets land in same shard
- `test_process_owned_segment_matches_borrowed` — Same TCP stream produces
  identical ReassembledStream via both APIs
- `test_flush_all_emits_buffered` — Connections with buffered data emit on flush
- `test_shard_processor_empty_shards` — Empty shards produce no events
- `test_shard_processor_tcp_then_tls` — TCP stream with TLS decrypts correctly
  in shard context
- `test_parallel_shards_match_sequential` — Critical correctness: process same
  capture sequentially and in parallel, compare sorted event lists
