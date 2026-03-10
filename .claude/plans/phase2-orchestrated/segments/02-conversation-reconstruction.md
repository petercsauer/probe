---
segment: 2
title: "Conversation & Session Reconstruction"
depends_on: []
risk: 7
complexity: High
cycle_budget: 5
status: pending
commit_message: "feat(prb-core): add conversation reconstruction engine with gRPC/ZMQ/DDS strategies"
---

# Subsection 8: Conversation & Session Reconstruction

## Purpose

Group related `DebugEvent`s into logical **conversations** — gRPC
request/response pairs, ZMQ REQ/REP exchanges, DDS writer/reader topic flows —
and enrich them with latency metrics, error classification, and state tracking.
This is the feature that turns a flat list of packets into an *investigation
tool*: a developer selects any event and immediately sees the full conversation
it belongs to, with timing between each frame annotated.

**Competitive analysis reference**: Recommendation #8 in
`competitive-analysis-2026-03-10.md`. Listed as Phase 2A prerequisite (Weeks 1–4)
alongside the query language, because meaningful TUI views require conversation
grouping.

---

## State of the Art

| Tool | Conversation Model | Limitation |
|------|-------------------|------------|
| **Wireshark** | "Follow TCP Stream" groups by 4-tuple. Stream index assigned on SYN, tracked through FIN/RST. Direction: client (red) vs server (blue). Output: ASCII, hex, YAML. | Byte-level only — no application-message pairing. Cannot show "this gRPC request got this response." |
| **Termshark** | Inherits Wireshark's stream model via tshark. Conversation view shows flow stats. | Same byte-level limitation. |
| **Hubble (Cilium)** | L7 flow records: source pod → dest pod with HTTP method, status, latency. | Pod-level granularity; no per-stream or per-message pairing. |
| **grpcurl / grpc-tools** | Proxy mode captures full request/response pairs. | gRPC-only, no multi-protocol. |
| **hoop.dev** | gRPC observability with "end-to-end context capturing exact request context across client and server with latency breakdowns across hops." | SaaS, not local tooling. |

**Probe's opportunity**: Operate at the *application message* level, not the byte
level. A gRPC conversation = the complete H2 stream lifecycle (HEADERS → DATA →
trailers). A ZMQ conversation = a REQ/REP pair within a connection. A DDS
conversation = all samples from a writer on a topic to its matched readers.
No existing tool does this for multiple protocols in one tool.

---

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│ prb-core                                                     │
│ ├── conversation.rs   Conversation, ConversationMetrics,     │
│ │                     ConversationKind, ConversationState     │
│ ├── engine.rs         ConversationEngine (strategy registry,  │
│ │                     orchestrator, index)                    │
│ ├── flow.rs           Flow (existing, enhanced)              │
│ └── traits.rs         CorrelationStrategy (existing)         │
├──────────────────────────────────────────────────────────────┤
│ prb-grpc                                                     │
│ └── correlation.rs    GrpcCorrelationStrategy                │
├──────────────────────────────────────────────────────────────┤
│ prb-zmq                                                      │
│ └── correlation.rs    ZmqCorrelationStrategy                 │
├──────────────────────────────────────────────────────────────┤
│ prb-dds                                                      │
│ └── correlation.rs    DdsCorrelationStrategy                 │
├──────────────────────────────────────────────────────────────┤
│ prb-cli                                                      │
│ └── cli.rs            `prb conversations` subcommand         │
└──────────────────────────────────────────────────────────────┘
```

**Dependency flow** (no circular deps):
- `prb-core`: defines `CorrelationStrategy` trait, `Flow`, `Conversation`, `ConversationEngine`
- `prb-grpc/zmq/dds`: depend on `prb-core`, implement `CorrelationStrategy`
- `prb-cli`: depends on all, registers strategies with engine at startup

---

## Segment Index

| # | Segment | Location | Est. Lines | Dependencies |
|---|---------|----------|------------|--------------|
| S8.1 | Conversation Model & Engine | `prb-core` | ~350 | — |
| S8.2 | gRPC Correlation Strategy | `prb-grpc` | ~250 | S8.1 |
| S8.3 | ZMQ Correlation Strategy | `prb-zmq` | ~200 | S8.1 |
| S8.4 | DDS Correlation Strategy | `prb-dds` | ~200 | S8.1 |
| S8.5 | Latency Analysis & Error Classification | `prb-core` | ~200 | S8.1 |
| S8.6 | CLI Integration & TUI Conversation View | `prb-cli`, `prb-tui` | ~300 | S8.1–S8.5 |

**Execution order**: S8.1 → (S8.2, S8.3, S8.4 in parallel) → S8.5 → S8.6

**Total**: ~1,500 lines of new code.

---

## Segment S8.1: Conversation Model & Engine

**Files**:
- `crates/prb-core/src/conversation.rs` (new)
- `crates/prb-core/src/engine.rs` (new)
- `crates/prb-core/src/lib.rs` (add modules + re-exports)

### Core Types

```rust
use crate::{DebugEvent, EventId, Timestamp, TransportKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

/// Unique conversation identifier.
/// Format: "{protocol}:{grouping_key}" e.g. "grpc:10.0.0.1:50051->10.0.0.2:8080/s3"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConversationId(pub String);

/// The kind of conversation, protocol-dependent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConversationKind {
    /// Single request, single response (gRPC unary).
    UnaryRpc,
    /// Single request, streaming responses (gRPC server-streaming).
    ServerStreaming,
    /// Streaming requests, single response (gRPC client-streaming).
    ClientStreaming,
    /// Bidirectional streaming (gRPC bidi).
    BidirectionalStreaming,
    /// ZMQ REQ/REP paired exchange.
    RequestReply,
    /// ZMQ PUB/SUB topic channel.
    PubSubChannel,
    /// ZMQ PUSH/PULL one-directional pipeline.
    Pipeline,
    /// DDS writer→reader(s) topic exchange.
    TopicExchange,
    /// Raw TCP connection (when protocol isn't decoded).
    TcpStream,
    /// Fallback for unknown patterns.
    Unknown,
}

/// Lifecycle state of a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConversationState {
    /// Conversation is ongoing (e.g., streaming).
    Active,
    /// Completed successfully (response received, status OK).
    Complete,
    /// Completed with error (gRPC error status, RST_STREAM, etc.).
    Error,
    /// No response within expected time / RST without response.
    Timeout,
    /// Incomplete capture (e.g., mid-stream join).
    Incomplete,
}

/// A reconstructed conversation grouping related events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    /// Unique conversation identifier.
    pub id: ConversationId,
    /// The kind of conversation.
    pub kind: ConversationKind,
    /// Protocol of the conversation.
    pub protocol: TransportKind,
    /// Current lifecycle state.
    pub state: ConversationState,
    /// Ordered event IDs belonging to this conversation.
    pub event_ids: Vec<EventId>,
    /// Computed timing and size metrics.
    pub metrics: ConversationMetrics,
    /// Conversation-level metadata (method, topic, etc.).
    pub metadata: BTreeMap<String, String>,
    /// Human-readable summary line.
    pub summary: String,
}

/// Timing and size metrics for a conversation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversationMetrics {
    /// Timestamp of the first event.
    pub start_time: Option<Timestamp>,
    /// Timestamp of the last event.
    pub end_time: Option<Timestamp>,
    /// Wall-clock duration (end - start).
    pub duration_ns: u64,
    /// Time from first outbound event to first inbound event.
    pub time_to_first_response_ns: Option<u64>,
    /// Number of outbound (request) messages.
    pub request_count: usize,
    /// Number of inbound (response) messages.
    pub response_count: usize,
    /// Total payload bytes across all events.
    pub total_bytes: u64,
    /// Error detail, if conversation ended in error.
    pub error: Option<ConversationError>,
}

/// Error classification for a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationError {
    /// Error kind (e.g., "grpc-status", "rst-stream", "timeout").
    pub kind: String,
    /// Error code (e.g., gRPC status code "14").
    pub code: Option<String>,
    /// Human-readable error message.
    pub message: String,
}
```

### ConversationEngine

```rust
use crate::{CorrelationStrategy, CoreError, DebugEvent, EventId};
use std::collections::HashMap;

/// Orchestrates conversation reconstruction across protocols.
pub struct ConversationEngine {
    strategies: Vec<Box<dyn CorrelationStrategy>>,
}

impl ConversationEngine {
    pub fn new() -> Self {
        Self { strategies: Vec::new() }
    }

    /// Register a protocol-specific correlation strategy.
    pub fn register(&mut self, strategy: Box<dyn CorrelationStrategy>) {
        self.strategies.push(strategy);
    }

    /// Build conversations from a slice of events.
    ///
    /// Each strategy handles events matching its transport. Events not claimed
    /// by any strategy are grouped into fallback TCP/UDP conversations by
    /// network address.
    pub fn build_conversations(&self, events: &[DebugEvent]) -> Result<ConversationSet, CoreError>;

    /// Look up which conversation an event belongs to.
    pub fn conversation_for_event(&self, set: &ConversationSet, event_id: EventId) -> Option<&Conversation>;
}

/// Holds all conversations plus an index for fast lookup.
pub struct ConversationSet {
    pub conversations: Vec<Conversation>,
    /// Maps event ID → conversation index for O(1) lookup.
    event_index: HashMap<EventId, usize>,
}

impl ConversationSet {
    /// Get conversation containing the given event.
    pub fn for_event(&self, event_id: EventId) -> Option<&Conversation> {
        self.event_index.get(&event_id).map(|&idx| &self.conversations[idx])
    }

    /// Get all conversations, sorted by start time.
    pub fn sorted_by_time(&self) -> Vec<&Conversation>;

    /// Filter conversations by protocol.
    pub fn by_protocol(&self, protocol: TransportKind) -> Vec<&Conversation>;

    /// Summary statistics.
    pub fn stats(&self) -> ConversationStats;
}

pub struct ConversationStats {
    pub total: usize,
    pub by_protocol: HashMap<TransportKind, usize>,
    pub by_state: HashMap<ConversationState, usize>,
    pub by_kind: HashMap<ConversationKind, usize>,
}
```

### Engine Algorithm

1. **Partition** events by `transport` field into per-protocol buckets.
2. **Dispatch** each bucket to the registered `CorrelationStrategy` matching
   that transport. The strategy returns `Vec<Flow>`.
3. **Enrich** each `Flow` → `Conversation` by computing metrics (S8.5).
4. **Fallback**: events not claimed by any strategy get grouped into
   `ConversationKind::TcpStream` or `Unknown` by `(src, dst)` network address.
5. **Index**: build `event_index` mapping every `EventId` → conversation index.

### Tests (S8.1)

- Empty event slice → empty ConversationSet
- Single event → single conversation (Incomplete state)
- Events from multiple protocols → partitioned correctly
- `for_event()` returns correct conversation
- `stats()` counts match actual conversations
- Fallback grouping for RawTcp events

---

## Segment S8.2: gRPC Correlation Strategy

**Files**:
- `crates/prb-grpc/src/correlation.rs` (new)
- `crates/prb-grpc/src/lib.rs` (add module + export)

### Grouping Key

gRPC conversations are identified by **connection + H2 stream ID**:

```
GroupingKey = (network.src, network.dst, h2.stream_id)
```

The `network.src` and `network.dst` from `EventSource` identify the TCP
connection (4-tuple from TCP reassembly). The `h2.stream_id` from metadata
identifies the multiplexed stream within that connection.

### Algorithm

```rust
pub struct GrpcCorrelationStrategy;

impl CorrelationStrategy for GrpcCorrelationStrategy {
    fn transport(&self) -> TransportKind { TransportKind::Grpc }

    fn correlate<'a>(&self, events: &'a [DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError> {
        // 1. Group events by (src, dst, h2.stream_id)
        //    Use indexmap::IndexMap for insertion-order preservation.
        //
        // 2. For each group, create a Flow:
        //    - id: "grpc:{src}->{dst}/s{stream_id}"
        //    - events: sorted by timestamp
        //    - metadata: grpc.method, grpc.authority, grpc.status
    }
}
```

### Conversation Kind Detection

After grouping, the engine's enrichment step (S8.5) classifies gRPC conversations:

| Pattern | Kind |
|---------|------|
| 1 Outbound DATA + 1 Inbound DATA + trailers | `UnaryRpc` |
| 1 Outbound DATA + N Inbound DATA + trailers | `ServerStreaming` |
| N Outbound DATA + 1 Inbound DATA + trailers | `ClientStreaming` |
| N Outbound DATA + N Inbound DATA + trailers | `BidirectionalStreaming` |
| Outbound only (no response) | state = `Timeout` |
| Any + RST_STREAM | state = `Error` |

### State Detection

| Condition | State |
|-----------|-------|
| Has `grpc.status == "0"` (OK) event | `Complete` |
| Has `grpc.status != "0"` event | `Error` |
| Has Outbound events, no Inbound | `Timeout` |
| Has Inbound events, no Outbound (mid-capture) | `Incomplete` |
| Stream still open (no trailers, no RST) | `Active` |

### Metadata Extracted

- `grpc.method` → from first event with `:path`
- `grpc.authority` → from first event with `:authority`
- `grpc.status` → from trailers event
- `grpc.message` → from trailers event
- `connection` → `{src} → {dst}`

### Summary Generation

```
"POST /api.v1.Users/Get → OK (12ms)"
"POST /api.v1.Orders/List → UNAVAILABLE (timeout)"
"POST /api.v1.Stream/Watch → server-streaming, 47 messages (2.3s)"
```

### Dependencies

- `indexmap` crate for ordered grouping

### Tests (S8.2)

- Fixture: 2 events (request + response + trailers) on same stream → 1 UnaryRpc conversation
- Fixture: 2 streams interleaved → 2 separate conversations
- Fixture: request without response → Timeout state
- Fixture: trailers-only (error) → Error state with status code
- Fixture: multiple DATA frames → correct streaming kind detection
- Fixture: events from different connections with same stream_id → separate conversations
- Verify `grpc.method`, `grpc.status` in conversation metadata

---

## Segment S8.3: ZMQ Correlation Strategy

**Files**:
- `crates/prb-zmq/src/correlation.rs` (new)
- `crates/prb-zmq/src/lib.rs` (add module + export)

### Grouping Strategy

ZMQ correlation depends on the socket pattern. The `zmq.socket_type` metadata
determines the grouping strategy:

| Socket Pattern | Grouping Key | Conversation Kind |
|---------------|--------------|-------------------|
| PUB/SUB | `zmq.topic` | `PubSubChannel` |
| REQ/REP | `zmq.connection_id` + temporal pairing | `RequestReply` |
| DEALER/ROUTER | `zmq.identity` or `zmq.connection_id` | `RequestReply` |
| PUSH/PULL | `zmq.connection_id` | `Pipeline` |
| PAIR | `zmq.connection_id` | `Unknown` |

### REQ/REP Pairing Algorithm

REQ/REP is strictly lock-step: send-receive-send-receive. Within a connection,
pair consecutive Outbound→Inbound event sequences:

```
Events on connection C: [OUT_1, IN_1, OUT_2, IN_2, OUT_3]
Conversations:
  - C/rr1: OUT_1, IN_1 → Complete
  - C/rr2: OUT_2, IN_2 → Complete
  - C/rr3: OUT_3 → Timeout (no reply)
```

### PUB/SUB Grouping

All events with the same `zmq.topic` value form one `PubSubChannel` conversation.
State is always `Active` (PUB/SUB has no completion concept).

### Algorithm

```rust
pub struct ZmqCorrelationStrategy;

impl CorrelationStrategy for ZmqCorrelationStrategy {
    fn transport(&self) -> TransportKind { TransportKind::Zmq }

    fn correlate<'a>(&self, events: &'a [DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError> {
        // 1. Partition by socket_type
        // 2. PUB/SUB: group by topic → one Flow per topic
        // 3. REQ/REP: group by connection_id, then pair temporally
        // 4. PUSH/PULL: group by connection_id → one Flow per connection
        // 5. DEALER/ROUTER: group by identity, pair temporally
    }
}
```

### Metadata Extracted

- `zmq.socket_type` → from events
- `zmq.topic` → for PUB/SUB
- `zmq.identity` → for DEALER/ROUTER
- `connection` → from `zmq.connection_id`

### Summary Generation

```
"PUB topic=market.data — 142 messages (5.2s)"
"REQ/REP 10.0.0.1:5555→10.0.0.2:5556 — OK (3ms)"
"PUSH/PULL pipeline — 89 messages"
```

### Tests (S8.3)

- PUB/SUB: events with same topic → 1 PubSubChannel
- PUB/SUB: events with different topics → separate conversations
- REQ/REP: alternating OUT/IN → paired conversations
- REQ/REP: trailing OUT without IN → Timeout
- PUSH/PULL: all events on same connection → 1 Pipeline
- Mixed socket types on same connection → separate conversations
- Events with no socket_type → fallback grouping

---

## Segment S8.4: DDS Correlation Strategy

**Files**:
- `crates/prb-dds/src/correlation.rs` (new)
- `crates/prb-dds/src/lib.rs` (add module + export)

### Grouping Strategy

DDS conversations are **topic-centric**. A conversation groups all DATA
submessages from a single writer GUID on a given topic within a domain:

```
GroupingKey = (dds.domain_id, dds.topic_name, dds.writer_guid)
```

### Sequence Tracking

DDS RTPS includes `dds.sequence_number` per writer. The strategy uses this to:
- Detect **gaps** (missing sequence numbers → packet loss)
- Detect **duplicates** (retransmissions in reliable mode)
- Calculate **sample rate** (messages per second)

### Algorithm

```rust
pub struct DdsCorrelationStrategy;

impl CorrelationStrategy for DdsCorrelationStrategy {
    fn transport(&self) -> TransportKind { TransportKind::DdsRtps }

    fn correlate<'a>(&self, events: &'a [DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError> {
        // 1. Group by (domain_id, topic_name, writer_guid)
        // 2. Sort each group by sequence_number
        // 3. Detect gaps and duplicates
        // 4. One Flow per group
    }
}
```

### DDS-Specific Metrics (added in S8.5)

- `sequence_gaps`: number of missing sequence numbers
- `duplicate_count`: retransmitted samples
- `sample_rate`: average messages per second
- `first_sequence` / `last_sequence`: range coverage

### State Detection

| Condition | State |
|-----------|-------|
| Continuous sequence with no gaps | `Complete` |
| Gaps detected in sequence | `Error` (with gap details) |
| No topic name discovered | `Incomplete` |
| Single sample | `Active` |

### Metadata Extracted

- `dds.domain_id` → from events
- `dds.topic_name` → from events (via SEDP discovery)
- `dds.writer_guid` → from events
- `sequence_range` → `{first}..{last}`
- `gap_count` → number of missing sequence numbers

### Summary Generation

```
"Topic=rt/chatter domain=0 writer=01020304... — 256 samples (12.1s, 0 gaps)"
"Topic=sensor/imu domain=1 writer=aabbccdd... — 1024 samples, 3 gaps"
```

### Tests (S8.4)

- Events with same (domain, topic, writer) → 1 conversation
- Events with different writers on same topic → separate conversations
- Events with different domains → separate conversations
- Sequence gap detection (1, 2, 4 → gap at 3)
- Duplicate detection (1, 2, 2, 3 → 1 duplicate)
- Events without topic_name → Incomplete state with warning
- Events without domain_id → still grouped by (topic, writer)

---

## Segment S8.5: Latency Analysis & Error Classification

**Files**:
- `crates/prb-core/src/metrics.rs` (new)
- `crates/prb-core/src/conversation.rs` (extend)

### Metrics Computation

The engine calls `compute_metrics()` on each `Flow` to produce
`ConversationMetrics`. This is protocol-agnostic — it operates on event
timestamps, directions, and metadata.

```rust
/// Compute metrics for a flow. Called by ConversationEngine during enrichment.
pub fn compute_metrics(events: &[&DebugEvent]) -> ConversationMetrics {
    // start_time: min timestamp
    // end_time: max timestamp
    // duration_ns: end - start
    // time_to_first_response_ns: first Inbound timestamp - first Outbound timestamp
    // request_count: count where direction == Outbound
    // response_count: count where direction == Inbound
    // total_bytes: sum of payload sizes
    // error: extracted from protocol-specific metadata
}
```

### Payload Size Calculation

```rust
fn payload_size(payload: &Payload) -> u64 {
    match payload {
        Payload::Raw { raw } => raw.len() as u64,
        Payload::Decoded { raw, .. } => raw.len() as u64,
    }
}
```

### Error Extraction

Protocol-specific error extraction is handled by a trait:

```rust
pub trait ErrorExtractor {
    fn extract_error(events: &[&DebugEvent]) -> Option<ConversationError>;
}
```

Built-in extractors:

| Protocol | Error Source | Error Kind |
|----------|-------------|------------|
| gRPC | `grpc.status != "0"` | `grpc-status` with code + message |
| gRPC | RST_STREAM event | `rst-stream` |
| ZMQ | No response to REQ | `timeout` |
| DDS | Sequence gaps | `sequence-gap` with gap count |
| Any | No Inbound within capture window | `no-response` |

### Aggregate Statistics

For a `ConversationSet`, compute aggregate metrics:

```rust
pub struct AggregateMetrics {
    pub total_conversations: usize,
    pub error_rate: f64,
    pub latency_p50_ns: u64,
    pub latency_p95_ns: u64,
    pub latency_p99_ns: u64,
    pub total_bytes: u64,
    pub conversations_per_second: f64,
}
```

Percentile computation uses a sorted vector of durations — simple and accurate
for the expected dataset sizes (thousands of conversations, not millions).

### Tests (S8.5)

- Two events (OUT then IN, 10ms apart) → duration = 10ms, TTFR = 10ms
- Single OUT event → TTFR = None, request_count = 1, response_count = 0
- gRPC events with `grpc.status = "14"` → error extracted with kind and message
- DDS events with sequence gap → error with gap count
- Aggregate: 3 conversations with known latencies → correct p50/p95/p99
- Zero-duration conversation (same timestamp) → duration = 0, no divide-by-zero

---

## Segment S8.6: CLI Integration & TUI Conversation View

### CLI: `prb conversations` Subcommand

**File**: `crates/prb-cli/src/cli.rs`

```
prb conversations <file> [--protocol grpc|zmq|dds] [--state error|timeout] [--sort latency|time]
```

**Output** (table format):

```
 # │ Protocol │ Kind        │ State    │ Latency │ Messages │ Summary
───┼──────────┼─────────────┼──────────┼─────────┼──────────┼─────────────────────────────────
 1 │ gRPC     │ unary       │ complete │ 12ms    │ 3        │ POST /api.v1.Users/Get → OK
 2 │ gRPC     │ unary       │ error    │ 45ms    │ 3        │ POST /api.v1.Orders/Create → UNAVAILABLE
 3 │ ZMQ      │ pub-sub     │ active   │ 5.2s    │ 142      │ PUB topic=market.data
 4 │ DDS      │ topic       │ complete │ 12.1s   │ 256      │ Topic=rt/chatter domain=0
```

**JSON output** (`--format json`): serializes `ConversationSet` with all
metadata and metrics.

**Detail view** (`prb conversations <file> --id 1`): shows all events in a
single conversation with per-event timing.

### CLI: `prb inspect --conversation <id>`

Extends the existing `inspect` command to filter events by conversation ID.
Uses `ConversationSet::for_event()` index.

### TUI: Conversation View

**Keyboard**: Press `C` on any selected event in the Event List pane to open
the Conversation View overlay.

**Layout**:

```
┌─ Conversation: POST /api.v1.Users/Get ──────────────────────┐
│ Kind: unary-rpc  State: complete  Latency: 12ms             │
├─────────────────────────────────────────────────────────────┤
│   Δ    │ Dir │ Type     │ Size  │ Detail                    │
│  +0ms  │  →  │ HEADERS  │   —   │ POST /api.v1.Users/Get    │
│  +1ms  │  →  │ DATA     │ 42B   │ GetUserRequest {id: 123}  │
│  +11ms │  ←  │ DATA     │ 128B  │ GetUserResponse {name:... │
│  +12ms │  ←  │ TRAILERS │   —   │ grpc-status: 0 (OK)       │
├─────────────────────────────────────────────────────────────┤
│ Metrics: TTFR=11ms Total=170B Req=1 Resp=1                  │
│ Press 'q' to close, 'j/k' to scroll, Enter to inspect event │
└─────────────────────────────────────────────────────────────┘
```

**Features**:
- Delta timestamps relative to conversation start
- Direction arrows (→ outbound, ← inbound)
- Payload size per event
- Scrollable if conversation has many messages (streaming RPCs)
- `Enter` on an event navigates back to Event List with that event selected
- Color-coded: green for OK, red for errors, yellow for warnings

### TUI: Conversation List View

**Keyboard**: Press `L` (list) in the TUI to switch from Event List to
Conversation List view, showing all conversations as a table.

| Column | Content |
|--------|---------|
| # | Conversation index |
| Protocol | gRPC / ZMQ / DDS |
| Kind | unary / server-streaming / pub-sub / topic |
| State | complete / error / timeout (color-coded) |
| Latency | Duration of conversation |
| Messages | Event count |
| Summary | Human-readable summary |

`Enter` on a conversation opens the Conversation View overlay for that
conversation.

### prb-query Integration

Add conversation-level fields to the query language so users can filter by
conversation properties:

```
conversation.kind == "unary-rpc"
conversation.state == "error"
conversation.latency_ms > 100
conversation.protocol == "gRPC"
```

### Tests (S8.6)

- `prb conversations fixtures/grpc_sample.json` produces table output
- `--format json` produces valid JSON matching ConversationSet schema
- `--protocol grpc` filters to gRPC conversations only
- `--state error` filters to error conversations only
- `--sort latency` sorts by latency descending
- Integration test: fixture with mixed protocols → correct conversation count

---

## New Dependencies

| Crate | Version | Used By | Purpose |
|-------|---------|---------|---------|
| `indexmap` | latest | prb-core, prb-grpc, prb-zmq, prb-dds | Insertion-ordered hash map for grouping events while preserving temporal order |

`indexmap` is chosen because:
- 50M+ monthly downloads, trusted and battle-tested
- Preserves insertion order (critical for temporal event grouping)
- O(1) lookup by key, O(1) insertion — same as HashMap but ordered
- Already a transitive dependency (via serde_json)

No other new external dependencies required. All computation is pure Rust
operating on existing `DebugEvent` types.

---

## Key Technical Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Strategy location | Each protocol crate | Keeps correlation logic co-located with decoder knowledge; no circular deps |
| Engine location | prb-core | Engine uses only the trait, not implementations; CLI wires them together |
| Conversation ID format | `{protocol}:{grouping_key}` | Human-readable, unique, stable across runs for same input |
| Grouping key for gRPC | `(src, dst, h2.stream_id)` | Stream IDs are only unique within a connection; need (src,dst) to disambiguate |
| REQ/REP pairing | Temporal within connection | ZMQ REQ/REP is strictly lock-step; temporal ordering is the only reliable signal from captured traffic |
| DDS grouping | `(domain_id, topic_name, writer_guid)` | Per-writer granularity is most useful; readers can be correlated via discovery |
| Percentile computation | Sorted vector | Simple, accurate, O(n log n); sufficient for expected dataset sizes (<100k conversations) |
| Conversation in engine, not strategy | Engine enriches Flow → Conversation | Strategies return simple Flows; engine applies protocol-agnostic metrics uniformly |

---

## Acceptance Criteria

- [ ] `cargo build --workspace` — zero errors, zero warnings
- [ ] `cargo clippy --workspace --all-targets` — zero warnings
- [ ] `cargo test --workspace` — all tests pass
- [ ] `GrpcCorrelationStrategy` groups gRPC events by connection + stream ID
- [ ] `ZmqCorrelationStrategy` groups ZMQ events by socket pattern (PUB/SUB, REQ/REP, PUSH/PULL)
- [ ] `DdsCorrelationStrategy` groups DDS events by domain + topic + writer GUID
- [ ] `ConversationEngine` orchestrates all strategies and produces `ConversationSet`
- [ ] `ConversationSet::for_event()` returns correct conversation for any event ID
- [ ] `ConversationMetrics` computes correct latency, TTFR, byte counts
- [ ] gRPC error states detected: grpc-status, RST_STREAM, timeout
- [ ] DDS sequence gap detection works correctly
- [ ] `prb conversations <file>` produces formatted table output
- [ ] `prb conversations <file> --format json` produces valid JSON
- [ ] Conversation kind correctly classified (unary vs streaming for gRPC)
- [ ] Summary strings are human-readable and accurate
- [ ] 10,000 events processed in < 100ms (no quadratic algorithms)
- [ ] No new `unsafe` code
- [ ] All public types have doc comments
