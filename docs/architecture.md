# Architecture

PRB is a Rust workspace organized as 19 crates with clear separation of concerns. This document describes the system design, data flow, and key abstractions.

## Layered Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                         prb-cli                                  │
│                  (CLI entry point, command dispatch)              │
├──────────┬──────────┬──────────┬──────────┬──────────┬───────────┤
│ prb-tui  │prb-export│prb-query │prb-storage│ prb-ai  │           │
│ (TUI)    │(CSV,HAR) │(filters) │(MCAP)    │(LLM)    │           │
├──────────┴──────────┴──────────┴──────────┴──────────┘           │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                      prb-core                               │ │
│  │  DebugEvent, CaptureAdapter, ProtocolDecoder, Conversation  │ │
│  │  Engine, TraceContext, CorrelationStrategy                   │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
├──────────────────────────────────────────────────────────────────┤
│                      Ingestion Layer                              │
│  ┌────────────┐  ┌────────────┐  ┌─────────────┐                │
│  │prb-fixture │  │ prb-pcap   │  │ prb-capture  │                │
│  │(JSON)      │  │(PCAP/pcapng│  │(live libpcap)│                │
│  └────────────┘  │ TCP reasm  │  └─────────────┘                │
│                  │ TLS decrypt│                                   │
│                  └────────────┘                                   │
├──────────────────────────────────────────────────────────────────┤
│                     Protocol Layer                                │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐ │
│  │ prb-detect │  │ prb-grpc   │  │  prb-zmq   │  │  prb-dds   │ │
│  │(detection) │  │(HTTP/2)    │  │(ZMTP)      │  │(RTPS)      │ │
│  └────────────┘  └────────────┘  └────────────┘  └────────────┘ │
├──────────────────────────────────────────────────────────────────┤
│                      Plugin Layer                                 │
│  ┌────────────────┐  ┌──────────────────┐  ┌──────────────────┐ │
│  │ prb-plugin-api │  │prb-plugin-native │  │ prb-plugin-wasm  │ │
│  │(contract, DTOs)│  │(.so/.dylib)      │  │(.wasm)           │ │
│  └────────────────┘  └──────────────────┘  └──────────────────┘ │
├──────────────────────────────────────────────────────────────────┤
│                     Schema Layer                                  │
│  ┌────────────┐  ┌────────────┐                                  │
│  │ prb-schema │  │ prb-decode │                                  │
│  │(registry)  │  │(protobuf)  │                                  │
│  └────────────┘  └────────────┘                                  │
└──────────────────────────────────────────────────────────────────┘
```

## Crate Responsibilities

| Crate | Role |
|-------|------|
| `prb-core` | Central data model (`DebugEvent`, `Payload`, `TransportKind`), core traits (`CaptureAdapter`, `ProtocolDecoder`, `CorrelationStrategy`, `SchemaResolver`), conversation engine, trace context parsing, metrics |
| `prb-cli` | Binary entry point, command dispatch, argument parsing via clap |
| `prb-fixture` | `CaptureAdapter` for JSON fixture files (testing and offline analysis) |
| `prb-pcap` | PCAP/pcapng file reading, packet normalization, TCP stream reassembly, TLS decryption, parallel pipeline |
| `prb-capture` | Live packet capture via libpcap with async channel output |
| `prb-detect` | Protocol auto-detection engine: port mapping, magic-byte inspection, heuristic analysis, `DecoderRegistry` |
| `prb-grpc` | gRPC/HTTP2 frame decoder, HPACK header decompression, protobuf payload extraction |
| `prb-zmq` | ZMTP protocol decoder: greeting, handshake, command, and message frames |
| `prb-dds` | DDS-RTPS protocol decoder: RTPS header, submessages, serialized data |
| `prb-schema` | Protobuf schema registry: load `.proto` and `.desc` files, resolve message types |
| `prb-decode` | Protobuf decoding: wire-format (best-effort without schema) and schema-backed with full field names |
| `prb-storage` | MCAP-based session persistence: write and read event streams with embedded schemas |
| `prb-query` | Event filter language: parser (nom-based), AST, evaluator |
| `prb-tui` | Interactive terminal UI via ratatui: event list, decode tree, hex dump, timeline panes |
| `prb-export` | Export to CSV, HAR, OTLP JSON, HTML, and Parquet |
| `prb-ai` | LLM-powered event explanation (Ollama, OpenAI) -- currently disabled |
| `prb-plugin-api` | Stable plugin contract: DTOs, FFI types, version validation, `prb_export_plugin!` macro |
| `prb-plugin-native` | Native shared library plugin loader (`.so`/`.dylib`/`.dll`) |
| `prb-plugin-wasm` | WebAssembly plugin runtime via wasmtime |

## Data Flow

### Offline PCAP Ingestion

```
PCAP file
  │
  ▼
PcapFileReader ─── reads packets from file
  │
  ▼
PacketNormalizer ─── link-layer handling, IP defragmentation
  │
  ├──► TCP packets ──► TcpReassembler ──► ordered byte streams
  │                         │
  │                         ▼
  │                    TlsStreamProcessor ─── decrypt if keylog/DSB available
  │                         │
  │                         ▼
  │                    DecoderRegistry.detect() ─── identify protocol
  │                         │
  │                         ▼
  │                    ProtocolDecoder.decode_stream() ─── decode to DebugEvent
  │
  └──► UDP datagrams ──► DecoderRegistry.detect() ──► decode
  │
  ▼
DebugEvent stream
  │
  ├──► stdout (NDJSON)
  ├──► MCAP file
  └──► ConversationEngine ──► grouped conversations
```

### Parallel Pipeline

For large captures, PRB shards processing by network flow:

```
Packets
  │
  ▼
normalize_stateless() ─── per-packet, no shared state
  │
  ▼
FlowPartitioner ─── hash by (src_ip, dst_ip, src_port, dst_port, proto)
  │
  ├──► Shard 0: TcpReassembler ► TLS ► Detect ► Decode
  ├──► Shard 1: TcpReassembler ► TLS ► Detect ► Decode
  ├──► Shard 2: ...
  └──► Shard N: ...
  │
  ▼
Merge results ──► ordered DebugEvent Vec
```

Worker count is controlled via `--jobs N` or the `PRB_JOBS` environment variable.

### Live Capture

```
Network interface
  │
  ▼
CaptureEngine (libpcap) ─── raw packet channel
  │
  ▼
LiveCaptureAdapter ─── pulls packets from channel
  │
  ▼
PipelineCore ─── same stages as offline: normalize ► reassemble ► decrypt ► detect ► decode
  │
  ▼
DebugEvent iterator
  │
  ├──► TUI (if --tui)
  ├──► stdout summary / NDJSON
  └──► file output (NDJSON / MCAP)
```

## Key Abstractions

### DebugEvent

The central type. Every decoder, adapter, and exporter speaks `DebugEvent`:

```rust
pub struct DebugEvent {
    pub id: EventId,                           // Monotonic unique ID
    pub timestamp: Timestamp,                   // Nanosecond precision
    pub source: EventSource,                    // Adapter, origin, network addrs
    pub transport: TransportKind,               // gRPC, ZMQ, DDS-RTPS, raw TCP/UDP
    pub direction: Direction,                   // Inbound, Outbound, Unknown
    pub payload: Payload,                       // Raw bytes or decoded fields
    pub metadata: BTreeMap<String, String>,     // Protocol-specific key-value pairs
    pub correlation_keys: Vec<CorrelationKey>,  // Stream ID, topic, trace context
    pub sequence: Option<u64>,                  // Ordering within a stream
    pub warnings: Vec<String>,                  // Non-fatal parse warnings
}
```

### CaptureAdapter

Trait for input sources. Implementors produce iterators of `DebugEvent`:

```rust
pub trait CaptureAdapter {
    fn name(&self) -> &str;
    fn ingest(&mut self) -> Box<dyn Iterator<Item = Result<DebugEvent, CoreError>> + '_>;
}
```

Built-in implementations: `JsonFixtureAdapter`, `PcapCaptureAdapter`, `LiveCaptureAdapter`.

### ProtocolDecoder

Trait for protocol-specific decoders. Each decoder consumes a raw byte stream and produces events:

```rust
pub trait ProtocolDecoder: Send {
    fn protocol(&self) -> TransportKind;
    fn decode_stream(&mut self, data: &[u8], ctx: &DecodeContext) -> Result<Vec<DebugEvent>, CoreError>;
}
```

Built-in implementations: gRPC/HTTP2, ZMTP, DDS-RTPS. External decoders are loaded via the plugin system.

### DecoderRegistry

Central registry that maps detected protocols to decoder factories:

```rust
pub trait DecoderFactory: Send + Sync {
    fn create_decoder(&self) -> Box<dyn ProtocolDecoder>;
}
```

Detection proceeds in layers: port mapping, magic-byte inspection, then heuristic analysis. The highest-confidence match wins.

### ConversationEngine

Groups events into conversations using per-protocol `CorrelationStrategy` implementations:

```rust
pub trait CorrelationStrategy {
    fn transport(&self) -> TransportKind;
    fn correlate<'a>(&self, events: &'a [DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError>;
}
```

Conversations are built from correlation keys (HTTP/2 stream IDs, ZMQ topics, DDS topic names, OpenTelemetry trace contexts) with fallback to address-pair grouping.

## Plugin System

PRB supports two plugin types:

- **Native plugins** -- Rust crates compiled to shared libraries (`.so`/`.dylib`/`.dll`). Use the `prb_export_plugin!` macro for FFI boilerplate.
- **WASM plugins** -- Rust crates compiled to `.wasm`. Data is exchanged via JSON serialization over the WASM boundary.

Both plugin types implement the same logical contract: `info()`, `detect()`, `create_decoder()`, `decode()`. See [Plugin Development](plugin-development.md).

## Trace Correlation

PRB extracts OpenTelemetry trace context from protocol headers using multiple formats:

- **W3C traceparent** -- `traceparent: 00-<trace_id>-<span_id>-<flags>`
- **B3 single-header** -- `b3: <trace_id>-<span_id>-<sampled>`
- **B3 multi-header** -- `X-B3-TraceId`, `X-B3-SpanId`, `X-B3-Sampled`
- **Jaeger** -- `uber-trace-id: <trace_id>:<span_id>:<parent_id>:<flags>`

Extracted contexts are stored as `CorrelationKey::TraceContext` and in event metadata (`otel.trace_id`, `otel.span_id`). Events can be filtered by trace/span ID and grouped into trace trees.
