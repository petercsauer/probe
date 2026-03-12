---
segment: 21
title: "Streaming Pipeline Architecture"
depends_on: [18, 19, 20]
risk: 9
complexity: High
cycle_budget: 4
status: pending
commit_message: "feat(prb-pcap): add channel-based streaming pipeline with micro-batching and backpressure"
---

# Subsection 6: Streaming Pipeline Architecture

## Purpose

Extend the batch-parallel pipeline (S1-S5) with a streaming mode that processes
packets as they arrive via channels. This is the foundation for live capture
(competitive analysis #2) and enables processing to begin before the entire
file is read.

The batch pipeline processes `Vec<PcapPacket>` → `Vec<DebugEvent>`.
The streaming pipeline processes `Stream<PcapPacket>` → `Stream<DebugEvent>`.

Both share the same stage implementations — the difference is orchestration.

---

## Architecture

```
                    ┌──────────────────────┐
                    │  Packet Source        │  File reader or live capture
                    │  (producer thread)    │
                    └──────────┬───────────┘
                               │ crossbeam channel (bounded)
                               ▼
                    ┌──────────────────────┐
                    │  Normalizer Pool      │  N worker threads (rayon)
                    │  (consumers)          │  Pull from input channel,
                    │                       │  push to flow router
                    └──────────┬───────────┘
                               │ crossbeam channel per shard
                    ┌──────────┼──────────────────────┐
                    ▼          ▼                       ▼
              ┌──────────┐ ┌──────────┐         ┌──────────┐
              │ Shard 0  │ │ Shard 1  │  ...    │ Shard N  │
              │ (thread)  │ │ (thread)  │         │ (thread)  │
              │ TCP+TLS   │ │ TCP+TLS   │         │ TCP+TLS   │
              │ +decode   │ │ +decode   │         │ +decode   │
              └─────┬─────┘ └─────┬─────┘         └─────┬─────┘
                    │             │                       │
                    └─────────────┼───────────────────────┘
                                  │ crossbeam channel (MPSC→single consumer)
                                  ▼
                    ┌──────────────────────┐
                    │  Output Collector     │  Receives events from all shards
                    │  (consumer thread)    │  Buffers + sorts by timestamp
                    └──────────────────────┘
```

---

## Segment S6.1: Channel-Based Stage Wiring

### Channel selection: crossbeam-channel

**Why crossbeam-channel over alternatives**:
- 327M downloads, battle-tested in production
- MPMC support (multiple normalizer threads → single flow router)
- Bounded channels for backpressure (prevents OOM on fast sources)
- `select!` macro for multi-channel operations
- Zero-cost when messages are pointers/small structs

**Alternative considered**: kanal (0.1.1) is faster in microbenchmarks but
has a smaller user base (1.2M downloads). We prefer crossbeam for reliability.

### Pipeline wiring

```rust
// crates/prb-pcap/src/parallel/streaming.rs

use crossbeam_channel::{bounded, Receiver, Sender};

pub struct StreamingPipeline {
    config: PipelineConfig,
    tls_keylog: Arc<TlsKeyLog>,
}

pub struct PipelineHandle {
    pub events: Receiver<DebugEvent>,
    pub stats: Arc<AtomicPipelineStats>,
}

impl StreamingPipeline {
    pub fn start(
        &self,
        source: impl Iterator<Item = PcapPacket> + Send + 'static,
    ) -> PipelineHandle {
        let num_shards = self.config.effective_shard_count();
        let batch_size = self.config.batch_size;

        // Channel capacities
        let raw_cap = batch_size * 4;
        let shard_cap = batch_size;
        let output_cap = batch_size * 2;

        // Source → Normalizer channel
        let (raw_tx, raw_rx) = bounded::<PcapPacket>(raw_cap);

        // Per-shard channels (normalizer → shard workers)
        let shard_channels: Vec<(Sender<OwnedNormalizedPacket>, Receiver<OwnedNormalizedPacket>)> =
            (0..num_shards).map(|_| bounded(shard_cap)).collect();

        // Shard → Output channel (all shards merge into one)
        let (event_tx, event_rx) = bounded::<DebugEvent>(output_cap);

        let stats = Arc::new(AtomicPipelineStats::new());

        // Spawn producer thread (reads from source)
        let stats_clone = Arc::clone(&stats);
        std::thread::Builder::new()
            .name("prb-source".into())
            .spawn(move || {
                for packet in source {
                    stats_clone.packets_read.fetch_add(1, Ordering::Relaxed);
                    if raw_tx.send(packet).is_err() {
                        break; // Pipeline shut down
                    }
                }
                drop(raw_tx); // Signal EOF
            })
            .expect("failed to spawn source thread");

        // Spawn normalizer + flow router (rayon scope)
        let shard_senders: Vec<Sender<_>> =
            shard_channels.iter().map(|(tx, _)| tx.clone()).collect();
        let stats_clone = Arc::clone(&stats);
        std::thread::Builder::new()
            .name("prb-normalize".into())
            .spawn(move || {
                Self::normalize_and_route(
                    raw_rx,
                    shard_senders,
                    num_shards,
                    batch_size,
                    stats_clone,
                );
            })
            .expect("failed to spawn normalizer thread");

        // Spawn shard worker threads
        let keylog = Arc::clone(&self.tls_keylog);
        for (shard_id, (_, shard_rx)) in shard_channels.into_iter().enumerate() {
            let event_tx = event_tx.clone();
            let keylog = Arc::clone(&keylog);
            let stats_clone = Arc::clone(&stats);
            std::thread::Builder::new()
                .name(format!("prb-shard-{}", shard_id))
                .spawn(move || {
                    Self::shard_worker(shard_id, shard_rx, event_tx, keylog, stats_clone);
                })
                .expect("failed to spawn shard worker");
        }
        drop(event_tx); // Only shard workers hold senders now

        PipelineHandle {
            events: event_rx,
            stats,
        }
    }
}
```

### Micro-batching in normalizer

Instead of sending one packet at a time through channels (high overhead),
the normalizer collects packets into micro-batches, normalizes them in parallel
with rayon, then routes results to shard channels:

```rust
fn normalize_and_route(
    input: Receiver<PcapPacket>,
    shard_senders: Vec<Sender<OwnedNormalizedPacket>>,
    num_shards: usize,
    batch_size: usize,
    stats: Arc<AtomicPipelineStats>,
) {
    let mut batch = Vec::with_capacity(batch_size);

    loop {
        // Collect a batch (with timeout for responsiveness)
        batch.clear();
        match input.recv() {
            Ok(pkt) => batch.push(pkt),
            Err(_) => break, // Channel closed (EOF)
        }

        // Drain up to batch_size without blocking
        while batch.len() < batch_size {
            match input.try_recv() {
                Ok(pkt) => batch.push(pkt),
                Err(_) => break,
            }
        }

        // Parallel normalize the batch
        let normalized: Vec<_> = batch
            .par_iter()
            .filter_map(|pkt| {
                normalize_stateless(pkt.linktype, pkt.timestamp_us, &pkt.data).ok()
            })
            .filter_map(|r| match r {
                NormalizeResult::Packet(p) => Some(p),
                NormalizeResult::Fragment { .. } => {
                    stats.fragments.fetch_add(1, Ordering::Relaxed);
                    None // Fragments dropped in streaming mode (acceptable trade-off)
                }
            })
            .collect();

        // Route to shards
        for packet in normalized {
            let shard_idx = FlowKey::from_packet(&packet)
                .map(|k| k.shard_index(num_shards))
                .unwrap_or(0);

            if shard_senders[shard_idx].send(packet).is_err() {
                break; // Shard worker shut down
            }
        }
    }

    // Drop shard senders to signal EOF to shard workers
    drop(shard_senders);
}
```

### Fragment handling in streaming mode

IP fragments in streaming mode present a challenge: we can't buffer them
because that would require cross-batch state. Options:
1. Drop fragments (acceptable — <1% of traffic, document the limitation)
2. Route fragments to a dedicated defrag thread

Choose option 1 for simplicity, with a counter so users see the drop rate.
Option 2 can be added later if needed.

---

## Segment S6.2: Shard Worker

Each shard worker runs its own `TcpReassembler` and processes events through
TLS decryption and protocol decode:

```rust
fn shard_worker(
    shard_id: usize,
    input: Receiver<OwnedNormalizedPacket>,
    output: Sender<DebugEvent>,
    keylog: Arc<TlsKeyLog>,
    stats: Arc<AtomicPipelineStats>,
) {
    let mut reassembler = TcpReassembler::new();
    let tls_processor = TlsStreamProcessor::with_keylog_ref(keylog);

    for packet in input {
        match &packet.transport {
            TransportInfo::Tcp(_) => {
                match reassembler.process_owned_segment(&packet) {
                    Ok(stream_events) => {
                        for se in stream_events {
                            if let StreamEvent::Data(stream) = se {
                                stats.tcp_streams.fetch_add(1, Ordering::Relaxed);
                                let decrypted = tls_processor.decrypt_stream(stream)
                                    .unwrap_or_else(|_| DecryptedStream::pass_through(stream));

                                let events = decode_stream(&decrypted);
                                for event in events {
                                    if output.send(event).is_err() {
                                        return; // Output closed
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => tracing::warn!("shard-{}: TCP: {}", shard_id, e),
                }
            }
            TransportInfo::Udp { src_port, dst_port } => {
                stats.udp_datagrams.fetch_add(1, Ordering::Relaxed);
                let event = create_udp_event(&packet, *src_port, *dst_port);
                let _ = output.send(event);
            }
            TransportInfo::Other(_) => {}
        }
    }

    // Flush remaining TCP connections
    for se in reassembler.flush_all() {
        if let StreamEvent::Data(stream) = se {
            if let Ok(decrypted) = tls_processor.decrypt_stream(stream) {
                for event in decode_stream(&decrypted) {
                    let _ = output.send(event);
                }
            }
        }
    }
}
```

---

## Segment S6.3: Atomic Pipeline Statistics

Thread-safe statistics for monitoring parallel pipeline progress:

```rust
// crates/prb-pcap/src/parallel/stats.rs

use std::sync::atomic::{AtomicU64, Ordering};

pub struct AtomicPipelineStats {
    pub packets_read: AtomicU64,
    pub packets_failed: AtomicU64,
    pub fragments: AtomicU64,
    pub tcp_streams: AtomicU64,
    pub udp_datagrams: AtomicU64,
    pub tls_decrypted: AtomicU64,
    pub tls_encrypted: AtomicU64,
    pub events_produced: AtomicU64,
}

impl AtomicPipelineStats {
    pub fn new() -> Self {
        Self {
            packets_read: AtomicU64::new(0),
            packets_failed: AtomicU64::new(0),
            fragments: AtomicU64::new(0),
            tcp_streams: AtomicU64::new(0),
            udp_datagrams: AtomicU64::new(0),
            tls_decrypted: AtomicU64::new(0),
            tls_encrypted: AtomicU64::new(0),
            events_produced: AtomicU64::new(0),
        }
    }

    pub fn snapshot(&self) -> PipelineStats {
        PipelineStats {
            packets_read: self.packets_read.load(Ordering::Relaxed),
            packets_failed: self.packets_failed.load(Ordering::Relaxed),
            tcp_streams: self.tcp_streams.load(Ordering::Relaxed),
            udp_datagrams: self.udp_datagrams.load(Ordering::Relaxed),
            tls_decrypted: self.tls_decrypted.load(Ordering::Relaxed),
            tls_encrypted: self.tls_encrypted.load(Ordering::Relaxed),
        }
    }
}
```

---

## Backpressure Design

Bounded channels provide natural backpressure:

| Channel | Capacity | Backpressure behavior |
|---------|----------|----------------------|
| Source → Normalizer | `batch_size * 4` | Source blocks when normalizer is slow |
| Normalizer → Shard | `batch_size` | Normalizer blocks when shard is slow |
| Shard → Output | `batch_size * 2` | Shard blocks when consumer is slow |

If the output consumer (TUI, file writer) is slower than the pipeline,
backpressure propagates backwards to the source, preventing unbounded memory
growth.

---

## Graceful Shutdown

Shutdown propagates via channel close:
1. Source finishes → drops `raw_tx` → normalizer's `recv()` returns `Err`
2. Normalizer drops all `shard_senders` → shard workers' `recv()` returns `Err`
3. Shard workers flush TCP state → drop `event_tx` → output `recv()` returns `Err`
4. Pipeline consumer sees `Err` → processing complete

For forced shutdown (Ctrl+C), use a shared `AtomicBool` cancel flag:
```rust
let cancel = Arc::new(AtomicBool::new(false));
// In each loop: if cancel.load(Ordering::Relaxed) { break; }
```

---

## When to Use Batch vs. Streaming

| Scenario | Mode | Reason |
|----------|------|--------|
| `prb ingest file.pcap` | Batch | File fully available; batch is simpler |
| `prb ingest file.pcap --stream` | Streaming | Events emitted as processed |
| `prb capture -i eth0` (future) | Streaming | Packets arrive continuously |
| `prb tui file.pcap` | Batch | TUI needs all events for scrolling |
| File > 1GB | Batch + mmap | mmap provides constant memory |

---

## Files Changed

| File | Change |
|------|--------|
| `crates/prb-pcap/src/parallel/streaming.rs` | New: `StreamingPipeline`, `PipelineHandle` |
| `crates/prb-pcap/src/parallel/stats.rs` | New: `AtomicPipelineStats` |
| `crates/prb-pcap/src/parallel/mod.rs` | Add submodules |
| `crates/prb-pcap/Cargo.toml` | Add `crossbeam-channel = "0.5"` |

---

## Tests

- `test_streaming_pipeline_basic` — Feed 100 synthetic packets, receive events
- `test_streaming_pipeline_empty` — Empty source → no events, clean shutdown
- `test_streaming_pipeline_backpressure` — Slow consumer causes source to block
  (verify no OOM with fast producer)
- `test_streaming_stats_accuracy` — Stats counters match expected after processing
- `test_streaming_graceful_shutdown` — Cancel flag causes clean termination
- `test_streaming_matches_batch` — Same input produces same events (sorted by
  timestamp) in both batch and streaming modes
- `test_streaming_fragment_counting` — Fragments are counted in stats.fragments
