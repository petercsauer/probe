---
segment: 1
title: "MCAP Session Storage Layer"
depends_on: []
risk: 4/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(storage): add MCAP session storage with read/write support"
---

# Segment 1: MCAP Session Storage Layer

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Implement the MCAP-backed storage layer that writes and reads DebugEvent sessions, replacing the transient in-memory pipeline from Subsection 1 with persistent file-backed sessions.

**Depends on:** Subsection 1 complete (DebugEvent type, CaptureAdapter trait, error conventions, CLI skeleton with `prb ingest` and `prb inspect` for fixtures)

## Context: Issues Addressed

### S2-1: DebugEvent-to-MCAP Mapping Unspecified

**Core Problem:** The parent plan says "events written to MCAP, readable later" but never specifies how DebugEvents map to MCAP's channel/schema/message model. MCAP requires each message to belong to a channel, each channel to reference a schema, and each message to carry raw bytes in the channel's declared encoding. Without this mapping, the storage layer cannot be implemented.

**Proposed Fix (inlined):**
- Message encoding: `"json"` -- DebugEvents serialized via serde_json. JSON is human-readable, works with Foxglove Studio, avoids bootstrapping protobuf for internal types.
- Channel strategy: One channel per (source_type, source_identifier) pair. Groups related events for efficient reading.
- Schema: Optional for Phase 1. JSON encoding does not strictly require a schema in MCAP.
- Message headers: `log_time` = DebugEvent timestamp (nanoseconds since epoch). `publish_time` = ingest time. `sequence` = monotonic counter per channel.
- Session metadata: MCAP Metadata record "session_info" with source_file, capture_tool, ingest_timestamp, tool_version, command-line arguments.

**Pre-Mortem:** JSON serialization may be slow for large sessions (1M+ events). Channel-per-source creates many channels in sessions with many connections. Memory-mapped reading requires full file accessible; very large MCAP files may exceed address space on 32-bit targets.

### S2-2: Schema Storage and Session Self-Containment (storage side only)

**Core Problem:** The plan does not specify whether protobuf schemas are stored inside MCAP session files (self-contained) or kept as external files. For a debug tool, self-contained sessions are critical: users share session files with teammates.

**Proposed Fix (storage side):** When writing a session, schemas from the registry that were used during decode are stored as MCAP Schema records with `encoding="protobuf"` and `data=FileDescriptorSet bytes`. When reading a session, the reader extracts MCAP Schema records (SchemaRegistry population is Segment 2). Design SessionWriter with `add_schema()` support and SessionReader with schema extraction hooks for Segment 2 integration.

**Pre-Mortem:** Embedding full FileDescriptorSets can be large (100KB+ for complex services). Deduplicate schemas by content hash.

## Scope

- New crate: `crates/storage/` (prb-storage)
- Modified: `crates/cli/` (prb-cli -- add --output to ingest, update inspect to read MCAP)

## Key Files and Context

Subsection 1 produces:
- `crates/core/src/event.rs` -- DebugEvent struct with serde Serialize/Deserialize
- `crates/core/src/traits.rs` -- CaptureAdapter, SchemaResolver, EventNormalizer traits
- `crates/core/src/error.rs` -- thiserror-based error types
- `crates/cli/src/main.rs` -- clap-based CLI with `ingest` and `inspect` subcommands
- `Cargo.toml` workspace root

MCAP Rust API (v0.24.0):
- Writer: `mcap::write::Writer<W: Write + Seek>` with `add_schema()`, `add_channel()`, `write_to_known_channel()`, `write_metadata()`, `finish()`
- Reader: `mcap::MessageStream::new(&mapped_bytes)` returns Iterator of Messages with channel and schema info
- Schema: `{ name: String, encoding: String, data: Cow<[u8]> }`
- Channel: references schema by ID, has topic name and message_encoding
- Metadata: `{ name: String, metadata: BTreeMap<String, String> }`
- Memory-mapped reading is the standard pattern (mmap the file, pass to MessageStream)

Design decisions:
- Message encoding = "json" (serde_json serialization of DebugEvent)
- Channel strategy = one channel per (source_type, source_id)
- Message header: log_time = event timestamp nanos, publish_time = ingest time nanos, sequence = monotonic per channel
- Session metadata = MCAP Metadata record "session_info" with source_file, capture_tool, ingest_timestamp, tool_version
- Compression = zstd (MCAP default feature), configurable via WriteOptions

## Implementation Approach

1. Create `crates/storage/` crate with deps: `mcap = "0.24"`, `serde_json`, plus workspace deps (prb-core, thiserror, tracing, bytes, camino).
2. Implement `SessionWriter<W: Write + Seek>`:
   - Constructor takes a writer and SessionMetadata. Writes metadata record immediately.
   - `write_event(&mut self, event: &DebugEvent)` -- looks up or creates channel for event's source, serializes event to JSON bytes, writes via `write_to_known_channel`.
   - `finish(self)` -- finalizes the MCAP file.
3. Implement `SessionReader`:
   - `open(path: &Path)` -- mmap the file, validate MCAP magic bytes.
   - `events(&self) -> impl Iterator<Item = Result<DebugEvent>>` -- wraps MessageStream, deserializes JSON.
   - `metadata(&self)` -- reads Metadata record "session_info".
   - `channels(&self)` -- list channels with event counts.
4. Extend CLI:
   - `prb ingest` gains `--output <path.mcap>` flag. When provided, writes events to MCAP instead of (or in addition to) stdout.
   - `prb inspect <path.mcap>` reads from MCAP file instead of re-ingesting.
   - `prb inspect` without a path continues to work with piped/fixture input (backward compat).
5. Write tests using tempfile for MCAP round-trip (write events, read them back, assert equality).

## Alternatives Ruled Out

- Protobuf encoding for DebugEvents: adds build-time protobuf compilation, makes debugging harder. JSON is appropriate for Phase 1.
- MessagePack/bincode: not recognized by MCAP ecosystem tools (Foxglove).
- Store schemas as attachments instead of Schema records: Schema records are semantically correct and integrate with MCAP viewers.

## Pre-Mortem Risks

- JSON serialization may be slow for large sessions. Write a benchmark as part of exit criteria.
- Channel proliferation: a capture with 1000 connections creates 1000 channels. Verify MCAP handles this.
- Memory-mapped reads fail if the file is being written concurrently. SessionReader must require the file to be finalized.

## Build and Test Commands

- Build: `cargo build -p prb-storage`
- Test (targeted): `cargo nextest run -p prb-storage`
- Test (regression): `cargo nextest run -p prb-core -p prb-fixture -p prb-cli`
- Test (full gate): `cargo nextest run --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_session_roundtrip`: write 100 DebugEvents, read back, assert all fields match including timestamps.
   - `test_session_metadata`: write session with metadata, read metadata back, assert values match.
   - `test_multi_channel`: write events from 3 different sources, verify 3 channels created, events correctly partitioned.
   - `test_empty_session`: create and finalize an empty session, verify it can be read without error.
   - `test_large_session`: write 10,000 events, read back, verify count and ordering preserved.
   - `test_cli_ingest_output`: run `prb ingest fixture.json --output out.mcap`, verify .mcap file is valid.
   - `test_cli_inspect_mcap`: run `prb inspect session.mcap`, verify events are printed to stdout.
2. **Regression tests:** All Subsection 1 tests pass (prb-core, prb-fixture, prb-cli existing tests).
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo nextest run --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changed files are in `crates/storage/`, `crates/cli/`, and `Cargo.toml` (workspace). No other crates modified.
