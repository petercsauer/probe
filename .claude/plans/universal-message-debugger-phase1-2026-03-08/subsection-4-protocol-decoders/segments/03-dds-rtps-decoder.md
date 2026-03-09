---
segment: 3
title: "DDS/RTPS Decoder"
depends_on: [1]
risk: 5/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(protocol-dds): add DDS/RTPS decoder with SEDP discovery tracking"
---

# Segment 3: DDS/RTPS Decoder

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Implement DDS/RTPS protocol decoding from UDP datagrams, including RTPS message parsing, DATA submessage payload extraction, SEDP discovery tracking for topic name resolution, and GUID-based correlation metadata.

**Depends on:** Segment 1 (protocol dispatch infrastructure must exist)

## Context: Issues Addressed

**D6 (DDS topic name extraction requires discovery):** RTPS entity IDs are numeric. Topic names are in SEDP discovery at connection establishment. Implement `RtpsDiscoveryTracker` that processes SEDP DATA submessages (well-known entity IDs like ENTITYID_SEDP_BUILTIN_PUBLICATIONS_WRITER = {0x00,0x00,0x03,0xC2}) and builds lookup (GuidPrefix, EntityId) -> TopicName. For user DATA submessages, look up writer entity; if discovery not observed, display entity ID in hex. Include topic_name: Option<String> in DebugEvent. Pre-mortem: SEDP uses CDR encoding (not protobuf); multiple domains have overlapping entity IDs (scope by GUID prefix); SEDP may be fragmented across DATA_FRAG.

**Parent Issue 9 (per-protocol correlation metadata):** DDS correlation is two-phase: (A) build discovery cache from SEDP (GUID to topic_name), (B) correlate data by lookup. Key: (domain_id, topic_name). Graceful degradation to GUID-only when discovery traffic is absent. Protocol adapters must emit correlation-relevant metadata (guid_prefix, entity_ids, topic_name, domain_id).

## Scope

- RTPS message parsing from UDP datagrams (using rtps-parser crate backed by dust_dds)
- DATA submessage extraction with serialized payload
- SEDP discovery tracking (topic name resolution from built-in endpoints)
- GUID prefix and entity ID correlation metadata
- Domain ID extraction from UDP port number using RTPS port mapping formula
- CLI integration: `prb inspect` showing DDS message details
- Graceful handling when discovery data is not present in capture

## Key Files and Context

RTPS messages arrive as complete UDP datagrams (no reassembly needed, unlike TCP protocols). Each datagram contains one RTPS message.

RTPS message structure:
- Header (20 bytes): `"RTPS"` magic (4 bytes) + protocol version (2 bytes, e.g. 2.3) + vendor ID (2 bytes) + GUID prefix (12 bytes).
- One or more submessages, each with: header (4 bytes: submessageId(1) + flags(1) + octetsToNextHeader(2)) + body.

Key submessage types:
- `DATA` (0x15): carries user data. Contains writerEntityId, readerEntityId, writerSN, serializedPayload. This is the primary submessage for extracting application messages.
- `DATA_FRAG` (0x16): carries fragmented data. Must reassemble fragments keyed by (writer GUID, sequence number).
- `INFO_TS` (0x09): source timestamp for subsequent submessages. Must be tracked and applied to the next DATA.
- `HEARTBEAT` (0x07): indicates available sequence numbers (reliability protocol).
- `ACKNACK` (0x06): acknowledges received data (reliability protocol).

SEDP discovery:
- Well-known entity IDs publish discovery data: `ENTITYID_SEDP_BUILTIN_PUBLICATIONS_WRITER = {0x00, 0x00, 0x03, 0xC2}` publishes `DiscoveredWriterData` containing topic name, type name, and QoS.
- Discovery data uses CDR (Common Data Representation) encoding with parameter lists, not protobuf.
- To map a user DATA submessage to a topic name: observe SEDP DATA for the same GUID prefix, extract topic name from the serialized DiscoveredWriterData, store in lookup table.

Domain ID calculation from UDP port: the RTPS spec defines default port mapping as `PB + DG * domainId + offset` where PB=7400, DG=250 for the default port mapping. Domain 0 uses ports 7400-7401, domain 1 uses 7650-7651, etc.

`rtps-parser` API (v0.1.1):
- `RtpsMessageRead::new(Arc<[u8]>)` -- parse RTPS message from raw bytes.
- `.header()` -- access GUID prefix, protocol version, vendor ID.
- `.submessages()` -- iterator over parsed submessage types (`RtpsSubmessageReadKind`).
- The crate depends on `dust_dds` (v0.14.0, 31K downloads, actively maintained) for the underlying types.
- License: Apache-2.0 (compatible).

Serialized data in DATA submessages uses CDR encoding. Phase 1 extracts raw CDR bytes and displays hex dump. Full CDR decode is deferred to a future phase or schema engine extension.

Error handling: library crates use `thiserror`. The `no-ignore-failure` rule requires that RTPS parse errors are surfaced, not silently dropped.

## Implementation Approach

1. Add `rtps-parser` (and transitive `dust_dds`) to dependencies. If compile time impact is unacceptable, consider extracting just the message parsing types into a local module.
2. Create DDS/RTPS decoder implementing `ProtocolDecoder`:
   - For each UDP datagram, attempt to parse as RTPS: check for "RTPS" magic bytes at offset 0-3. If no match, reject (not RTPS).
   - Parse with `RtpsMessageRead::new()`. Iterate submessages.
   - Track `INFO_TS` timestamps (apply to subsequent DATA submessages in the same message).
   - For `DATA` submessages: extract writer entity ID, reader entity ID, sequence number, and serialized payload.
   - For SEDP entity IDs (check writer entity ID against well-known IDs): parse `DiscoveredWriterData` / `DiscoveredReaderData` from serialized payload to extract topic name. Store in `RtpsDiscoveryTracker` keyed by `(GuidPrefix, EntityId)`.
   - For user DATA submessages: look up writer entity in discovery tracker for topic name. Fall back to hex entity ID display.
3. Domain ID extraction: calculate from destination UDP port using RTPS port mapping formula.
4. Emit `DebugEvent` with: guid_prefix, entity_ids, sequence_number, topic_name (if discovered), domain_id, timestamp, raw_payload.
5. Protocol dispatch integration: register for UDP traffic. Detect RTPS by "RTPS" magic at bytes 0-3 of UDP payload. Default port range hint: 7400-7500 (covers domains 0-3 with default port mapping).

## Alternatives Ruled Out

- Using `rustdds` (full DDS implementation) for just parsing. Too heavy, brings in networking, async stack, and QoS machinery.
- Building a custom RTPS parser from scratch. The wire format has many submessage types and endianness-dependent field parsing. rtps-parser handles these correctly despite low adoption. Building from scratch risks correctness bugs on edge cases.
- Full CDR decode in Phase 1. CDR is complex (type-dependent encoding, alignment rules). Defer to Phase 2 or schema engine extension.

## Pre-Mortem Risks

- `rtps-parser` depends on `dust_dds` which may significantly increase compile time. If unacceptable, consider vendoring just the parsing types.
- SEDP discovery data uses CDR-encoded parameter lists. Parsing these to extract topic names requires understanding the DiscoveredWriterData serialization format, which is non-trivial. Start with the most common parameter IDs (PID_TOPIC_NAME = 0x0005) and skip unknown ones.
- DATA_FRAG submessages require fragment reassembly (separate from IP fragmentation). Need a fragment buffer keyed by (writer GUID, sequence number).
- Multiple DDS domains in the same capture have overlapping port ranges. Must scope discovery tracker by GUID prefix, not domain ID alone.

## Build and Test Commands

- Build: `cargo build -p prb-protocol-dds`
- Test (targeted): `cargo test -p prb-protocol-dds`
- Test (regression): `cargo test -p prb-core -p prb-protocol-grpc -p prb-protocol-zmtp`
- Test (full gate): `cargo test --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_rtps_message_parse`: Feed a valid RTPS message with "RTPS" magic, version 2.3, and one DATA submessage. Verify header fields and submessage extraction.
   - `test_rtps_data_payload`: Feed a DATA submessage with serialized payload. Verify entity IDs, sequence number, and raw payload bytes are extracted.
   - `test_rtps_info_ts_timestamp`: Feed INFO_TS + DATA submessages. Verify the timestamp from INFO_TS is applied to the subsequent DATA event.
   - `test_rtps_discovery_topic_name`: Feed SEDP DATA submessages containing a DiscoveredWriterData with topic name "sensor/imu". Then feed a user DATA submessage from the same writer entity. Verify the topic name "sensor/imu" is resolved.
   - `test_rtps_no_discovery_fallback`: Feed user DATA submessages without prior SEDP data. Verify entity IDs are displayed as hex and topic_name is None.
   - `test_rtps_domain_id_from_port`: Feed a datagram with destination port 7400. Verify domain_id=0. Feed port 7650, verify domain_id=1.
   - `test_rtps_magic_detection`: Feed a non-RTPS UDP datagram. Verify it is rejected by the protocol detector.
2. **Regression tests:** All tests from Segments 1-2 and Subsections 1-3 continue passing.
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo test --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changed files are in the DDS decoder crate, discovery tracker module, and CLI integration. Out-of-scope changes documented.
