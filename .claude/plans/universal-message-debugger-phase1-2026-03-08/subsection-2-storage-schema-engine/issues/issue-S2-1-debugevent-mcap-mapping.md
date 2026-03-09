---
id: "S2-1"
title: "DebugEvent-to-MCAP Mapping Unspecified"
risk: 4/10
addressed_by_segments: [1]
---

# Issue S2-1: DebugEvent-to-MCAP Mapping Unspecified

**Core Problem:**
The parent plan says "events written to MCAP, readable later" but never specifies how DebugEvents map to MCAP's channel/schema/message model. MCAP requires each message to belong to a channel, each channel to reference a schema, and each message to carry raw bytes in the channel's declared encoding. Without this mapping, the storage layer cannot be implemented.

**Root Cause:**
The parent plan defines the storage format (MCAP) and the event model (DebugEvent) but does not bridge the gap between them.

**Proposed Fix:**
Define the mapping as follows:

- **Message encoding:** `"json"` -- DebugEvents are serialized via serde_json. JSON is human-readable, works with Foxglove Studio for visualization, and avoids bootstrapping a protobuf schema for the tool's own internal types.
- **Channel strategy:** One channel per (source_type, source_identifier) pair. For example, a gRPC capture from a specific connection gets its own channel. Fixture data gets a "fixture" channel. This groups related events for efficient reading.
- **Schema:** Optional for Phase 1. The JSON encoding does not strictly require a schema in MCAP. If desired later, a JSON Schema for DebugEvent can be added.
- **Message headers:** `log_time` = DebugEvent timestamp (nanoseconds since epoch). `publish_time` = ingest time. `sequence` = monotonic counter per channel.
- **Session metadata:** Stored as MCAP Metadata records with name="session_info" containing: source file path, capture tool, ingest timestamp, tool version, and command-line arguments.

API sketch:

```rust
pub struct SessionWriter<W: Write + Seek> {
    writer: mcap::write::Writer<W>,
    channels: HashMap<ChannelKey, u16>,
    sequence: HashMap<u16, u32>,
}

impl<W: Write + Seek> SessionWriter<W> {
    pub fn new(writer: W, metadata: SessionMetadata) -> Result<Self>;
    pub fn write_event(&mut self, event: &DebugEvent) -> Result<()>;
    pub fn finish(self) -> Result<()>;
}

pub struct SessionReader { /* memory-mapped MCAP */ }

impl SessionReader {
    pub fn open(path: &Path) -> Result<Self>;
    pub fn events(&self) -> impl Iterator<Item = Result<DebugEvent>>;
    pub fn metadata(&self) -> Result<SessionMetadata>;
    pub fn channels(&self) -> Vec<ChannelInfo>;
}
```

**Existing Solutions Evaluated:**
N/A -- internal design decision about how to map our event model to the MCAP container. MCAP's schema registry spec (mcap.dev/spec/registry) defines the conventions for protobuf, JSON, and other encodings. We follow those conventions.

**Alternatives Considered:**

- Protobuf encoding for DebugEvents (define a .proto for DebugEvent, compile at build time). Rejected for Phase 1: adds build-time protobuf compilation, makes debugging the debugger's own storage harder, and Foxglove would need the .proto to visualize. JSON is simpler for Phase 1; protobuf encoding can be added as an optimization in a later phase.
- MessagePack/bincode encoding. Rejected: not recognized by MCAP ecosystem tools (Foxglove, mcap CLI). Loses human readability.

**Pre-Mortem -- What Could Go Wrong:**

- JSON serialization is slower than binary formats. For large sessions (1M+ events), ingest time may be dominated by serde_json. Mitigation: benchmark and add a `--encoding` flag later.
- Channel-per-source creates many channels in sessions with many connections. MCAP handles this fine (the format supports 65535 channels), but readers must handle the proliferation.
- Memory-mapped reading requires the full file to be accessible. Very large MCAP files may exceed available address space on 32-bit targets (not a concern for the Linux+macOS 64-bit target).

**Risk Factor:** 4/10

**Evidence for Optimality:**

- External evidence: MCAP's format registry (mcap.dev/spec/registry) defines JSON as a supported message encoding with well-defined semantics.
- External evidence: Foxglove Studio, the primary MCAP visualization tool, supports JSON-encoded messages natively.

**Blast Radius:**

- Direct changes: new `prb-storage` crate (SessionWriter, SessionReader, SessionMetadata)
- Potential ripple: CLI commands (`prb ingest` output path, `prb inspect` input path)
