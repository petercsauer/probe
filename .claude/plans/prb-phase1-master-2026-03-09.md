# PRB Universal Message Debugger — Phase 1 Master Plan

**Goal:** Build a CLI tool (`prb`) that ingests PCAP/pcapng captures and JSON fixtures, decodes gRPC/ZMTP/DDS-RTPS messages, stores sessions in MCAP format, and supports offline analysis and replay.
**Generated:** 2026-03-09
**Rules version:** 2026-03-08
**Entry point:** B (Enrich Existing Plan — cursor plans for subsections 1-4 exist)
**Status:** Ready for execution (Subsections 1-4); Subsection 5 needs deep-plan

---

## Overview

Phase 1 is a purely offline analysis CLI. All I/O is file-based. There is no live capture, no async runtime, and no network sockets in Phase 1. The tool reads PCAP/pcapng or JSON fixture files, decodes them, and writes results to MCAP session files or stdout.

The work is decomposed into 5 subsections. Subsections 1-4 have full deep-plans (segment briefs, exit criteria, commands) in `.cursor/plans/`. Subsection 5 (Analysis & Replay) requires a deep-plan before execution.

**Ordering strategy:** Dependency-order (topological). No parallelism at the subsection level — each one defines types consumed by the next.

**Build environment:** Rust/Cargo workspace. See `.claude/commands/devcontainer-exec.md` for all build/test commands.

**Walking skeleton:** After Subsection 1, `prb ingest fixtures/sample.json | prb inspect --format table` works end-to-end. Each subsequent subsection adds capability without breaking the skeleton.

---

## Dependency Diagram

```
Subsection 1: Foundation & Core Model
        │
        ▼
Subsection 2: Storage & Schema Engine
        │
        ▼
Subsection 3: Network Capture Pipeline
        │
        ▼
Subsection 4: Protocol Decoders
        │
        ▼
Subsection 5: Analysis & Replay  ← NEEDS DEEP-PLAN
```

---

## Workspace Crate Map

| Crate | Subsection | Purpose |
|-------|-----------|---------|
| `prb-core` | 1 | DebugEvent type, core traits, error conventions |
| `prb-fixture` | 1 | JSON fixture CaptureAdapter |
| `prb-cli` | 1+ | CLI binary (`prb`) using clap |
| `prb-storage` | 2 | MCAP SessionWriter/SessionReader |
| `prb-schema` | 2 | Protobuf SchemaRegistry (prost-reflect, protox) |
| `prb-decode` | 2 | Schema-backed + wire-format protobuf decode |
| `prb-pcap` | 3 | PCAP/pcapng file reading (pcap-parser v0.17) |
| `prb-tcp` | 3 | TCP stream reassembly (smoltcp Assembler) |
| `prb-tls` | 3 | TLS decryption from SSLKEYLOGFILE (ring v0.17) |
| `prb-grpc` | 4 | gRPC/HTTP2 decoder (h2-sans-io v0.1.0) |
| `prb-zmq` | 4 | ZMQ/ZMTP decoder (custom ~300 lines) |
| `prb-dds` | 4 | DDS/RTPS decoder (rtps-parser) |
| `prb-correlation` | 5 | Correlation engine |
| `prb-replay` | 5 | Replay engine (structured stdout + timing) |

---

## Segment 1: Foundation & Core Model

> **Execution method:** Use `/orchestrate` on `.cursor/plans/universal-message-debugger-phase1-2026-03-08/subsection-1-foundation-core-model.md`. That plan contains 3 sequential segments; execute each as an `iterative-builder` subagent per the orchestration protocol.

**Goal:** Establish the Cargo workspace, canonical DebugEvent model, core extension traits, error conventions, JSON fixture adapter, and a walking-skeleton CLI.

**Depends on:** None

**Cursor plan:** `.cursor/plans/universal-message-debugger-phase1-2026-03-08/subsection-1-foundation-core-model.md`
**Status:** Ready for execution

**Sub-segments (from cursor plan):**
- S1.1: Workspace + Core Types — `prb-core` crate, DebugEvent struct, error types (10 cycles, Low, risk 2/10)
- S1.2: Traits + Fixture Adapter — `prb-fixture` crate, CaptureAdapter/ProtocolDecoder/SchemaResolver/EventNormalizer/CorrelationStrategy traits (15 cycles, Medium, risk 3/10)
- S1.3: CLI + Walking Skeleton — `prb-cli` binary, `prb ingest` + `prb inspect` commands (15 cycles, Medium, risk 3/10)

**Key technical decisions (from cursor plan):**
- All 5 traits are synchronous for Phase 1 (async fn in traits not dyn-safe)
- `CaptureAdapter::ingest()` returns `Box<dyn Iterator<Item = Result<DebugEvent, CoreError>> + '_>`
- Error handling: `thiserror` in `prb-core`/`prb-fixture`, `anyhow` in `prb-cli`
- Rust edition 2024, resolver 3 (requires Rust ≥ 1.85)
- JSON fixture format: `{ "version": 1, "events": [...] }` with `payload_base64` or `payload_utf8`

**Build/test commands:**
- Build: `cargo build -p prb-core && cargo build -p prb-fixture && cargo build -p prb-cli`
- Test targeted: `cargo nextest run -p prb-core` / `cargo nextest run -p prb-fixture`
- Regression: `cargo nextest run --workspace`
- Full gate: `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

**Exit criteria (subsection-level):**
1. `cargo build --workspace` succeeds (no `prb-storage` yet — only core, fixture, cli)
2. `cargo nextest run -p prb-core` all pass
3. `cargo nextest run -p prb-fixture` all pass
4. Walking skeleton works: `prb ingest fixtures/sample.json | prb inspect --format table` prints events
5. `cargo clippy --workspace -- -D warnings` clean

**Risk factor:** 3/10
**Estimated complexity:** Medium
**Commit message:** `feat(foundation): workspace, DebugEvent model, core traits, fixture adapter, walking skeleton CLI`

---

## Segment 2: Storage & Schema Engine

> **Execution method:** Use `/orchestrate` on `.cursor/plans/universal-message-debugger-phase1-2026-03-08/subsection-2-storage-schema-engine.md`. That plan contains 4 segments; Segments 3 and 4 (schema-backed and wire-format decode) are independent and can run as parallel iterative-builder subagents.

**Goal:** Persistent MCAP-backed storage, protobuf schema registry, schema-backed decode, and wire-format schema-less decode.

**Depends on:** Segment 1 complete (DebugEvent, traits, CLI skeleton)

**Cursor plan:** `.cursor/plans/universal-message-debugger-phase1-2026-03-08/subsection-2-storage-schema-engine.md`
**Status:** Planning (issue briefs complete; verify segment briefs are finalized before executing)

**Sub-segments (from cursor plan):**
- S2.1: MCAP Session Storage Layer — `prb-storage` crate, SessionWriter/SessionReader (15 cycles, Medium, risk 4/10)
- S2.2: Protobuf Schema Registry — `prb-schema` crate, SchemaRegistry, .desc and .proto loading via protox (15 cycles, Medium, risk 3/10)
- S2.3: Schema-backed Decode — `prb-decode`, DynamicMessage::decode via prost-reflect (20 cycles, High, risk 5/10) [parallel with S2.4]
- S2.4: Wire-format Decode — `prb-decode`, custom wire-format decoder with multi-interpretation output (15 cycles, Medium, risk 3/10) [parallel with S2.3]

**Key technical decisions (from cursor plan):**
- MCAP encoding: `"json"` (serde_json for DebugEvent) — human-readable, Foxglove-compatible
- Channel strategy: one channel per (source_type, source_identifier) pair
- Schemas embedded in MCAP as Schema records with `encoding="protobuf"` and FileDescriptorSet bytes — sessions are self-contained
- prost-reflect v0.16.3 for schema-backed decode (`DynamicMessage::decode(descriptor, buf)`)
- protox v0.9.1 for runtime .proto compilation (no protoc needed)
- mcap v0.24.0 for storage
- Wire-format disambiguation: show primary interpretation + alternatives for all wire types (not just type 2)
- Note: `protobuf-decode` crate does NOT exist; use custom implementation (~200-350 lines)

**Build/test commands:**
- Build: `cargo build -p prb-storage && cargo build -p prb-schema && cargo build -p prb-decode`
- Test targeted: `cargo nextest run -p prb-storage` / `cargo nextest run -p prb-schema` / `cargo nextest run -p prb-decode`
- Regression: `cargo nextest run -p prb-core -p prb-fixture -p prb-cli` (no regressions in subsection 1)
- Full gate: `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

**Exit criteria (subsection-level):**
1. MCAP round-trip: write DebugEvents to session file, read back, assert equality
2. Schema registry: load .desc and .proto files; `resolve()` returns correct descriptor
3. Schema-backed decode: decode a known protobuf bytes with schema → matching DebugEvent
4. Wire-format decode: decode bytes without schema → multi-interpretation output (uint/sint/bool for varints, etc.)
5. CLI: `prb ingest ... --output session.mcap` writes MCAP; `prb inspect session.mcap` reads it
6. `cargo nextest run --workspace` all pass
7. `cargo clippy --workspace -- -D warnings` clean

**Risk factor:** 5/10
**Estimated complexity:** High
**Commit message:** `feat(storage): MCAP storage, protobuf schema registry, schema-backed and wire-format decode`

---

## Segment 3: Network Capture Pipeline

> **Execution method:** Use `/orchestrate` on `.cursor/plans/universal-message-debugger-phase1-2026-03-08/subsection-3-network-capture-pipeline.md`. That plan contains 5 strictly sequential segments.

**Goal:** Complete pipeline from raw PCAP/pcapng files to reassembled, optionally TLS-decrypted TCP/UDP byte streams ready for protocol decoders.

**Depends on:** Segment 2 complete (storage + schema working; DebugEvent can be persisted)

**Cursor plan:** `.cursor/plans/universal-message-debugger-phase1-2026-03-08/subsection-3-network-capture-pipeline.md`
**Status:** Ready for execution

**Sub-segments (from cursor plan):**
- S3.1: PCAP/pcapng Reader — `prb-pcap` crate, auto-detect format, DSB key extraction (10 cycles, Low, risk 3/10)
- S3.2: Packet Normalization — Ethernet/SLL/SLL2 demux, IP defrag, VLAN stripping (15 cycles, Medium, risk 4/10)
- S3.3: TCP Reassembly — smoltcp Assembler, stream → byte buffer, UDP passthrough (15 cycles, Medium, risk 5/10)
- S3.4: TLS Decryption — `prb-tls` crate, SSLKEYLOGFILE import, ring AES-GCM/ChaCha20 (20 cycles, High, risk 8/10)
- S3.5: Pipeline Integration + CLI — wire pcap→normalize→reassemble→decrypt→emit, `prb ingest capture.pcap --tls-keylog keys.log` (15 cycles, Medium, risk 4/10)

**Key technical decisions (from cursor plan):**
- pcap-parser v0.17.0 — handles both pcap and pcapng, DSB blocks, multiple interfaces
- etherparse v0.19.0 — MUST be v0.19+ for `defrag::IpDefragPool`; supports SLL v1 but NOT SLL2
- SLL2 (linktype 276): thin custom parser needed (~40 lines); etherparse does not support it
- smoltcp v0.12+ for TCP reassembly (12K+ stars, `storage::Assembler`)
- TLS scope: AEAD suites only (AES-GCM, ChaCha20-Poly1305) in Phase 1; CBC-mode deferred
- pcapsql-core v0.3.1 as reference implementation for TLS architecture
- Wireshark sample captures from wiki.wireshark.org/SampleCaptures as test fixtures

**Build/test commands:**
- Build: `cargo build -p prb-pcap && cargo build -p prb-tcp && cargo build -p prb-tls`
- Test targeted: `cargo nextest run -p prb-pcap` / `cargo nextest run -p prb-tcp` / `cargo nextest run -p prb-tls`
- Regression: `cargo nextest run -p prb-core -p prb-fixture -p prb-storage -p prb-schema -p prb-decode`
- Full gate: `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

**Exit criteria (subsection-level):**
1. Both pcap and pcapng sample files parse without error
2. IP fragments reassemble correctly (test with etherparse defrag pool)
3. TCP streams reassemble to byte-accurate payload (test with known capture)
4. TLS session with known key log decrypts to expected plaintext
5. `prb ingest capture.pcapng --output session.mcap` writes events
6. `prb ingest capture.pcapng --tls-keylog keys.log --output session.mcap` decrypts TLS
7. `cargo nextest run --workspace` all pass
8. `cargo clippy --workspace -- -D warnings` clean

**Risk factor:** 8/10 (TLS decryption is the high-risk segment)
**Estimated complexity:** High
**Commit message:** `feat(network): PCAP/pcapng pipeline with IP defrag, TCP reassembly, and TLS decryption`

---

## Segment 4: Protocol Decoders

> **Execution method:** Use `/orchestrate` on `.cursor/plans/universal-message-debugger-phase1-2026-03-08/subsection-4-protocol-decoders.md`. That plan has 3 segments; Segments 2 (ZMTP) and 3 (DDS/RTPS) are independent and can run as parallel iterative-builder subagents after Segment 1 (gRPC).

**Goal:** Decode gRPC/HTTP2, ZMQ/ZMTP, and DDS/RTPS byte streams into DebugEvents using the protocol dispatch infrastructure established by the gRPC decoder.

**Depends on:** Segment 3 complete (reassembled TCP/UDP byte streams available)

**Cursor plan:** `.cursor/plans/universal-message-debugger-phase1-2026-03-08/subsection-4-protocol-decoders.md`
**Status:** Ready for execution

**Sub-segments (from cursor plan):**
- S4.1: gRPC/HTTP2 Decoder — `prb-grpc` crate, h2-sans-io frame parsing, HPACK, gRPC message framing, compression, trailers (20 cycles, High, risk 6/10)
- S4.2: ZMTP Decoder — `prb-zmq` crate, custom ZMTP 3.x parser (~300 lines), greeting+handshake+traffic frames (15 cycles, Medium, risk 4/10) [parallel with S4.3]
- S4.3: DDS/RTPS Decoder — `prb-dds` crate, rtps-parser (avoiding full dust_dds stack), SEDP topic discovery (15 cycles, Medium, risk 5/10) [parallel with S4.2]

**Key technical decisions (from cursor plan):**
- h2-sans-io v0.1.0 pinned exactly (`"=0.1.0"`) — very new crate (107 downloads, created 2026-02-15); fallback is fluke-h2-parse + fluke-hpack
- HPACK graceful degradation: when context missing (mid-stream capture), log warning + fall back to payload-only analysis
- No `zmtp` crate (last updated 2016, dead); custom ZMTP 3.0/3.1 parser (RFC 23/RFC 37)
- rzmq v0.5.13 as behavioral reference for ZMTP (MPL-2.0, do not use directly)
- ZMTP mid-stream limitation: captures starting mid-connection cannot determine mechanism; document clearly
- DDS topic name extraction requires observing SEDP discovery; cache discovered topics in decoder state

**Build/test commands:**
- Build: `cargo build -p prb-grpc && cargo build -p prb-zmq && cargo build -p prb-dds`
- Test targeted: `cargo nextest run -p prb-grpc` / `cargo nextest run -p prb-zmq` / `cargo nextest run -p prb-dds`
- Regression: `cargo nextest run -p prb-core -p prb-fixture -p prb-storage -p prb-schema -p prb-decode -p prb-pcap -p prb-tcp -p prb-tls`
- Full gate: `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

**Exit criteria (subsection-level):**
1. gRPC: decode a known gRPC message from captured HTTP/2 frames → correct DebugEvent with method path and protobuf payload
2. gRPC compression: handle gzip/deflate compressed messages
3. gRPC trailers: extract status code from trailers-only frames
4. ZMTP: decode READY command and message frames from a known ZMTP capture
5. DDS/RTPS: decode discovered topics and data samples from a known DDS capture
6. Protocol dispatch: `prb ingest` with a mixed capture routes packets to correct decoders
7. `cargo nextest run --workspace` all pass
8. `cargo clippy --workspace -- -D warnings` clean

**Risk factor:** 6/10
**Estimated complexity:** High
**Commit message:** `feat(decoders): gRPC/HTTP2, ZMTP, and DDS/RTPS protocol decoders`

---

## Segment 5: Analysis & Replay  ⚠️ NEEDS DEEP-PLAN

> **Execution method:** BLOCKED — this subsection requires a deep-plan before execution. Run `/deep-plan` on this subsection first (Entry Point A, Fresh Goal).

**Goal:** Correlation engine that groups DebugEvents into logical flows across connections, and a replay engine that emits structured stdout output with timing for scripting and testing.

**Depends on:** Segment 4 complete (all protocol decoders working)

**Cursor plan:** None — this subsection has NOT been deep-planned yet.

**Crates to create:**
- `prb-correlation` — implements CorrelationStrategy trait for each protocol; groups events into Flow objects
- `prb-replay` — reads a session MCAP file and re-emits events to stdout with timing metadata

**Known scope from workspace structure (Issue S1-3):**
- `prb-correlation`: per-protocol correlation strategy definitions; output = `Vec<Flow<'a>>`
- `prb-replay`: replay target is structured stdout with timing (NOT protocol re-emission); supports scripting

**Action required:** Run `/deep-plan` on Subsection 5 before executing. Reference this master plan as context (Entry Point B). Key constraints to carry forward:
- All traits synchronous (Phase 1 offline only)
- CorrelationStrategy trait signature already defined in prb-core: `fn correlate<'a>(&self, events: &'a [DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError>`
- Replay is structured stdout + timing, not live protocol re-emission

**Risk factor:** TBD (unknown until planned)
**Estimated complexity:** TBD

---

## Execution Instructions

**To execute subsections 1-4:** Use `/orchestrate` with the cursor plan path for each subsection (listed in each segment above). The orchestration agent reads each subsection's plan and launches iterative-builder subagents for its segments.

**To plan subsection 5:** Run `/deep-plan` using Entry Point B. Feed this master plan as context for the "Enrich Existing Plan" entry point.

**Full execution sequence:**
1. `/orchestrate` → `.cursor/plans/universal-message-debugger-phase1-2026-03-08/subsection-1-foundation-core-model.md`
2. `/orchestrate` → `.cursor/plans/universal-message-debugger-phase1-2026-03-08/subsection-2-storage-schema-engine.md` (verify segment briefs are finalized first)
3. `/orchestrate` → `.cursor/plans/universal-message-debugger-phase1-2026-03-08/subsection-3-network-capture-pipeline.md`
4. `/orchestrate` → `.cursor/plans/universal-message-debugger-phase1-2026-03-08/subsection-4-protocol-decoders.md`
5. `/deep-plan` on Subsection 5 → approve → `/orchestrate` result
6. Run `/deep-research` or post-build verification after all subsections complete

**Parallelization opportunities within subsections:**
- Subsection 2: Segments S2.3 and S2.4 (schema-backed and wire-format decode) are independent, can run in parallel
- Subsection 4: Segments S4.2 (ZMTP) and S4.3 (DDS/RTPS) are independent after S4.1, can run in parallel

---

## Execution Log

| Segment | Cursor Plan | Risk | Status | Notes |
|---------|------------|------|--------|-------|
| 1: Foundation & Core Model | subsection-1-foundation-core-model.md | 3/10 | pending | -- |
| 2: Storage & Schema Engine | subsection-2-storage-schema-engine.md | 5/10 | pending | Verify segment briefs complete before running |
| 3: Network Capture Pipeline | subsection-3-network-capture-pipeline.md | 8/10 | pending | TLS decryption is high-risk |
| 4: Protocol Decoders | subsection-4-protocol-decoders.md | 6/10 | pending | h2-sans-io adoption risk (pin =0.1.0) |
| 5: Analysis & Replay | (none yet) | TBD | needs-deep-plan | Run /deep-plan first |

**Deep-verify result:** --
**Follow-up plans:** Subsection 5 deep-plan required
