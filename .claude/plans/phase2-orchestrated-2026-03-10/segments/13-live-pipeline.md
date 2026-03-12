---
segment: 13
title: "Live Pipeline Integration"
depends_on: [12]
risk: 8
complexity: High
cycle_budget: 4
status: pending
commit_message: "refactor(prb-pcap): extract PipelineCore for streaming reuse, add LiveCaptureAdapter"
---

# S2: Live Pipeline Integration

**Goal**: Refactor `prb-pcap`'s pipeline internals so the normalize→reassemble→TLS→decode
stages can be driven incrementally by a live packet source, not just batch-processed
from a file. Create `LiveCaptureAdapter` implementing the existing `CaptureAdapter` trait.

**Key constraint**: The existing `PcapCaptureAdapter` must continue working unchanged.
We extract shared logic, not replace it.

---

## S2.1: Extract `PipelineCore` — Shared Pipeline Logic

### Problem

`PcapCaptureAdapter::process_all_packets()` does everything: read file, normalize,
reassemble, TLS decrypt, emit events. The normalization→reassembly→TLS logic is
buried inside a method that also owns the file reader. Live capture needs the same
pipeline stages but with a different packet source.

### Solution: `PipelineCore` struct

Extract the stateful pipeline components into a separate struct that accepts packets
one at a time:

```rust
// crates/prb-pcap/src/pipeline_core.rs

pub struct PipelineCore {
    normalizer: PacketNormalizer,
    tcp_reassembler: TcpReassembler,
    tls_processor: TlsStreamProcessor,
    stats: PipelineStats,
}

pub struct ProcessedEvents {
    pub events: Vec<DebugEvent>,
    pub warnings: Vec<String>,
}

impl PipelineCore {
    pub fn new(tls_processor: TlsStreamProcessor) -> Self {
        Self {
            normalizer: PacketNormalizer::new(),
            tcp_reassembler: TcpReassembler::new(),
            tls_processor,
            stats: PipelineStats::default(),
        }
    }

    pub fn with_keylog(keylog_path: &Path) -> Result<Self, PcapError> {
        let keylog = TlsKeyLog::from_file(keylog_path)?;
        Ok(Self::new(TlsStreamProcessor::with_keylog(keylog)))
    }

    /// Process a single raw packet. Returns zero or more DebugEvents.
    ///
    /// This is the hot path — called once per captured packet. Must be
    /// allocation-minimal for high-throughput live capture.
    pub fn process_packet(
        &mut self,
        linktype: u32,
        timestamp_us: u64,
        data: &[u8],
        origin: &str,
    ) -> ProcessedEvents {
        let mut result = ProcessedEvents {
            events: Vec::new(),
            warnings: Vec::new(),
        };

        self.stats.packets_read += 1;

        // Stage 1: Normalize
        let normalized = match self.normalizer.normalize(linktype, timestamp_us, data) {
            Ok(Some(norm)) => norm,
            Ok(None) => return result, // fragment
            Err(e) => {
                self.stats.packets_failed += 1;
                result.warnings.push(format!("normalize failed: {e}"));
                return result;
            }
        };

        // Stage 2: Dispatch by transport
        match &normalized.transport {
            TransportInfo::Tcp(_) => {
                self.process_tcp_segment(&normalized, origin, &mut result);
            }
            TransportInfo::Udp { src_port, dst_port } => {
                self.stats.udp_datagrams += 1;
                let event = create_udp_event(&normalized, *src_port, *dst_port, origin);
                result.events.push(event);
            }
            TransportInfo::Other(_) => {}
        }

        result
    }

    /// Flush idle TCP connections. Call periodically (e.g., every second)
    /// during live capture to emit buffered stream data.
    pub fn flush_idle(&mut self, current_time_us: u64) -> Vec<DebugEvent> {
        let timeout_events = self.tcp_reassembler
            .cleanup_idle_connections(current_time_us);
        // Process any flushed streams through TLS + event creation
        let mut events = Vec::new();
        for event in timeout_events {
            if let StreamEvent::Data(stream) = event {
                if let Some(evt) = self.process_stream(stream) {
                    events.push(evt);
                }
            }
        }
        events
    }

    pub fn stats(&self) -> &PipelineStats { &self.stats }
}
```

### Refactor `PcapCaptureAdapter` to use `PipelineCore`

```rust
// In pipeline.rs — PcapCaptureAdapter::process_all_packets becomes:

fn process_all_packets(&mut self) -> Result<(), CoreError> {
    let mut reader = PcapFileReader::open(&self.capture_path)?;
    let packets = reader.read_all_packets()?;

    let tls_processor = self.build_tls_processor(&reader)?;
    let mut core = PipelineCore::new(tls_processor);

    for packet in &packets {
        let result = core.process_packet(
            packet.linktype,
            packet.timestamp_us,
            &packet.data,
            &self.capture_path.display().to_string(),
        );
        for event in result.events {
            self.event_queue.push_back(Ok(event));
        }
    }

    // Flush remaining TCP streams
    let final_time = packets.last().map(|p| p.timestamp_us).unwrap_or(0);
    for event in core.flush_idle(final_time + 1_000_000) {
        self.event_queue.push_back(Ok(event));
    }

    self.stats = core.stats().clone();
    Ok(())
}
```

This refactoring changes no external behavior — `PcapCaptureAdapter` works identically —
but makes the internal pipeline reusable for live sources.

---

## S2.2: `LiveCaptureAdapter` — Streaming `CaptureAdapter`

### Design

`LiveCaptureAdapter` wraps a `CaptureEngine` (from `prb-capture`) and a
`PipelineCore`, implementing `CaptureAdapter` for live sources. The key
difference from `PcapCaptureAdapter`: instead of reading all packets then
iterating, it continuously receives packets from the capture thread.

### New Trait Method (Optional Extension)

The existing `CaptureAdapter::ingest()` returns a boxed iterator. For live
capture, we also need a streaming variant. Two approaches:

**Option A**: Keep the existing trait, make `ingest()` block-and-yield from
the live stream. The iterator never ends until the capture stops. This works
for NDJSON output but not for TUI (which needs async).

**Option B**: Add an async extension trait:

```rust
// crates/prb-core/src/traits.rs

#[async_trait::async_trait]
pub trait LiveCaptureSource: Send {
    async fn next_event(&mut self) -> Option<Result<DebugEvent, CoreError>>;
    fn stats(&self) -> CaptureStats;
    fn stop(&mut self);
}
```

**Decision**: Use **Option A** for CLI streaming output (NDJSON, pcap save).
Use **Option B** for TUI integration (async event loop needs non-blocking).
`LiveCaptureAdapter` implements both.

### Implementation Sketch

```rust
// crates/prb-capture/src/adapter.rs

pub struct LiveCaptureAdapter {
    engine: CaptureEngine,
    core: PipelineCore,
    event_buffer: VecDeque<Result<DebugEvent, CoreError>>,
    origin: String,
    linktype: u32,
}

impl LiveCaptureAdapter {
    pub fn new(config: CaptureConfig) -> Result<Self, CaptureError> {
        let origin = format!("live:{}", config.interface);
        let tls = match &config.tls_keylog_path {
            Some(path) => TlsStreamProcessor::with_keylog(
                TlsKeyLog::from_file(path).map_err(|e| CaptureError::Other(e.to_string()))?
            ),
            None => TlsStreamProcessor::new(),
        };
        Ok(Self {
            engine: CaptureEngine::new(config),
            core: PipelineCore::new(tls),
            event_buffer: VecDeque::new(),
            origin,
            linktype: 1, // LINKTYPE_ETHERNET, updated from pcap handle
        })
    }

    pub fn start(&mut self) -> Result<(), CaptureError> {
        self.engine.start()?;
        self.linktype = self.engine.datalink()?.0 as u32;
        Ok(())
    }
}

impl CaptureAdapter for LiveCaptureAdapter {
    fn name(&self) -> &str { "live-capture" }

    fn ingest(&mut self) -> Box<dyn Iterator<Item = Result<DebugEvent, CoreError>> + '_> {
        let rx = self.engine.receiver()
            .expect("engine must be started before ingest");

        Box::new(std::iter::from_fn(move || {
            // Drain buffered events first
            if let Some(event) = self.event_buffer.pop_front() {
                return Some(event);
            }

            // Block on next packet from capture thread
            loop {
                match rx.recv() {
                    Ok(packet) => {
                        let result = self.core.process_packet(
                            self.linktype,
                            packet.timestamp_us,
                            &packet.data,
                            &self.origin,
                        );
                        for event in result.events {
                            self.event_buffer.push_back(Ok(event));
                        }
                        if let Some(event) = self.event_buffer.pop_front() {
                            return Some(event);
                        }
                        // Packet produced no events (e.g., TCP fragment), try next
                    }
                    Err(_) => return None, // Channel closed = capture stopped
                }
            }
        }))
    }
}
```

### Async Variant for TUI

```rust
impl LiveCaptureAdapter {
    pub async fn next_event_async(&mut self) -> Option<Result<DebugEvent, CoreError>> {
        if let Some(event) = self.event_buffer.pop_front() {
            return Some(event);
        }

        let rx = self.engine.receiver()?;
        loop {
            // Use tokio::task::spawn_blocking to avoid blocking the runtime
            let packet = {
                let rx = rx.clone();
                tokio::task::spawn_blocking(move || rx.recv_timeout(Duration::from_millis(100)))
                    .await
                    .ok()?
            };

            match packet {
                Ok(pkt) => {
                    let result = self.core.process_packet(
                        self.linktype, pkt.timestamp_us, &pkt.data, &self.origin,
                    );
                    for event in result.events {
                        self.event_buffer.push_back(Ok(event));
                    }
                    if let Some(event) = self.event_buffer.pop_front() {
                        return Some(event);
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    // Flush idle TCP connections periodically
                    let now_us = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_micros() as u64;
                    for event in self.core.flush_idle(now_us) {
                        self.event_buffer.push_back(Ok(event));
                    }
                    if let Some(event) = self.event_buffer.pop_front() {
                        return Some(event);
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return None,
            }
        }
    }
}
```

---

## S2.3: Protocol Decoder Integration

Currently, protocol decoders (`GrpcDecoder`, `ZmqDecoder`, `DdsDecoder`) are not
wired into the PCAP pipeline — they exist as standalone modules used by `prb inspect`.
For live capture, we need automatic protocol detection and decoding.

### Protocol Detection Heuristics

Add to `PipelineCore`:

```rust
fn detect_and_decode(
    &mut self,
    event: DebugEvent,
    decoders: &mut DecoderRegistry,
) -> Vec<DebugEvent> {
    // Check transport kind
    match event.transport {
        TransportKind::RawTcp => {
            // Try gRPC first (HTTP/2 preface: "PRI * HTTP/2.0\r\n")
            if let Payload::Raw { ref raw } = event.payload {
                if raw.starts_with(b"PRI * HTTP/2.0") || self.is_h2_frame(raw) {
                    return decoders.grpc.decode_stream(raw, &self.make_ctx(&event))
                        .unwrap_or_else(|_| vec![event.clone()]);
                }
                // Try ZMQ (ZMTP greeting: 0xFF + 8 bytes + 0x7F)
                if raw.len() >= 10 && raw[0] == 0xFF && raw[9] == 0x7F {
                    return decoders.zmq.decode_stream(raw, &self.make_ctx(&event))
                        .unwrap_or_else(|_| vec![event.clone()]);
                }
            }
            vec![event]
        }
        TransportKind::RawUdp => {
            // Try DDS-RTPS (magic: "RTPS")
            if let Payload::Raw { ref raw } = event.payload {
                if raw.starts_with(b"RTPS") {
                    return decoders.dds.decode_stream(raw, &self.make_ctx(&event))
                        .unwrap_or_else(|_| vec![event.clone()]);
                }
            }
            vec![event]
        }
        _ => vec![event],
    }
}
```

### `DecoderRegistry`

```rust
pub struct DecoderRegistry {
    pub grpc: GrpcDecoder,
    pub zmq: ZmqDecoder,
    pub dds: DdsDecoder,
}

impl Default for DecoderRegistry {
    fn default() -> Self {
        Self {
            grpc: GrpcDecoder::new(),
            zmq: ZmqDecoder::new(),
            dds: DdsDecoder::new(),
        }
    }
}
```

### Integration into `PipelineCore::process_packet`

Protocol decoding becomes an optional final stage. Users opt in via config:

```rust
pub struct PipelineConfig {
    pub enable_protocol_decode: bool,
    pub protocol_decoders: Option<DecoderRegistry>,
}
```

For live capture, protocol decode is enabled by default.
For batch PCAP processing, it remains off (decoders run later in `prb inspect`).

---

## Implementation Checklist

- [ ] Create `crates/prb-pcap/src/pipeline_core.rs`
- [ ] Extract `PipelineCore` with `process_packet()` and `flush_idle()`
- [ ] Refactor `PcapCaptureAdapter` to delegate to `PipelineCore`
- [ ] Verify all existing tests pass after refactoring
- [ ] Implement `LiveCaptureAdapter` struct in `prb-capture`
- [ ] Implement sync `CaptureAdapter` for `LiveCaptureAdapter`
- [ ] Implement async `next_event_async()` for TUI integration
- [ ] Add protocol detection heuristics to `PipelineCore`
- [ ] Create `DecoderRegistry` struct
- [ ] Wire decoders into live capture path
- [ ] Unit test: `PipelineCore::process_packet` produces correct events for TCP/UDP
- [ ] Unit test: `flush_idle` emits buffered TCP streams
- [ ] Integration test: `LiveCaptureAdapter` produces events from loopback
