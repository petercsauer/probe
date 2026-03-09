---
segment: 1
title: "gRPC/HTTP2 Decoder"
depends_on: []
risk: 6/10
complexity: High
cycle_budget: 20
status: pending
commit_message: "feat(protocol-grpc): add gRPC/HTTP2 decoder with HPACK and compression"
---

# Segment 1: gRPC/HTTP2 Decoder

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Implement gRPC protocol decoding from reassembled TCP streams, including HTTP/2 frame parsing, HPACK header decompression, gRPC message extraction with compression support, trailer/status parsing, and the protocol dispatch infrastructure shared by all decoders.

**Depends on:** Subsection 3 complete (reassembled TCP streams available), Subsection 2 complete (protobuf decode engine available), Subsection 1 complete (ProtocolDecoder trait defined)

## Context: Issues Addressed

**D2 (h2-sans-io adoption risk):** h2-sans-io was published 2026-02-15, has only 107 downloads, one version. Mitigations: pin exact version `h2-sans-io = "=0.1.0"`, write extensive integration tests, document fallback path (fluke-h2-parse + fluke-hpack), vendor if needed. Pre-mortem: CONTINUATION assembly bugs for large header sets; crate abandonment; API changes.

**D3 (gRPC compression):** gRPC uses 5-byte LPM header (compressed_flag: u8, message_length: u32 big-endian, payload). When compress_flag=1, decompress using grpc-encoding header (gzip/deflate/identity) via flate2. Pre-mortem: messages span multiple DATA frames; snappy/zstd not handled; large message memory spikes.

**D4 (gRPC trailers/status):** Trailers in HEADERS with end_stream=true carry grpc-status and grpc-message. Store in DebugEvent. Trailers-Only responses have no DATA frames. Pre-mortem: Trailers-Only misidentified as request headers; grpc-status-details-bin deferred.

**Parent Issue 2 (HPACK statefulness):** HTTP/2 HPACK uses stateful dynamic table. Mid-stream captures lack context. Implement graceful degradation: on HPACK failure, log warning via tracing::warn!, set hpack_degraded: true, continue payload-only analysis. Do NOT silently ignore. Ensure degradation only triggers for missing context (dynamic table reference failure), not malformed data.

**Parent Issue 3 (h2 library):** hyperium/h2 is async client/server and cannot parse offline captures. Use h2-sans-io (synchronous, sans-I/O) which accepts raw bytes and returns parsed events.

## Scope

- Protocol dispatch infrastructure (port/magic-byte detection with user override via `--protocol` flag)
- gRPC protocol decoder crate
- HTTP/2 frame parsing with h2-sans-io
- HPACK header decompression with graceful degradation for mid-stream captures
- gRPC Length-Prefixed-Message parsing (5-byte header: compress_flag + message_length)
- gRPC message decompression (gzip/deflate via flate2 when compress_flag=1)
- gRPC trailer parsing (grpc-status, grpc-message from trailing HEADERS)
- Per-stream state tracking (request headers, response headers, data frames, trailers)
- Correlation metadata population: connection_id, stream_id, method_name, authority, grpc_status
- CLI integration: `prb inspect` showing decoded gRPC call details

## Key Files and Context

The `ProtocolDecoder` trait is defined in the core crate (established by Subsection 1). It accepts byte streams and produces `DebugEvent` instances. The pipeline from Subsection 3 provides reassembled TCP byte streams; each stream represents one TCP connection (both directions). The protobuf decode engine from Subsection 2 provides `SchemaResolver` for decoding protobuf payloads when schemas are available. `DebugEvent` is the canonical event type defined in Subsection 1.

h2-sans-io API (v0.1.0):
- `H2Codec::new()` creates a new codec instance.
- `codec.process(&bytes) -> Result<Vec<H2Event>>` feeds raw bytes and returns parsed events.
- `H2Event::Headers { stream_id, header_block, end_stream }` -- decoded headers on a stream.
- `H2Event::Data { stream_id, data, end_stream }` -- data payload on a stream.
- `H2Event::Settings { ack, settings }` -- SETTINGS frame.
- Additional events for RST_STREAM, GOAWAY, PING, WINDOW_UPDATE.
- HPACK decompression is integrated via fluke-hpack (v0.3.1, 70K downloads).
- CONTINUATION frames are automatically assembled before emitting Headers events.

gRPC wire format over HTTP/2:
- gRPC uses HTTP/2 with POST method. Request path is `/{service}/{method}`.
- Client sends: HEADERS (method, authority, content-type=application/grpc, grpc-encoding) + DATA frames.
- Server sends: HEADERS (status=200, content-type) + DATA frames + trailing HEADERS (grpc-status, grpc-message).
- Each DATA frame carries gRPC Length-Prefixed-Messages: `{compressed_flag: u8, message_length: u32 (big-endian), payload: [u8]}`.
- Messages may span multiple DATA frames; must accumulate until `message_length` bytes received.
- Stream IDs are odd (client-initiated). Stream 0 is the connection control stream.
- Stream IDs can be reused after RST_STREAM; correlation must scope to stream lifetime.
- Trailers-Only responses have no DATA frames, just trailers with grpc-status.

Error handling convention (from Subsection 1): library crates use `thiserror` with typed error enums. The `no-ignore-failure` rule requires loud failures, not silent swallowing.

## Implementation Approach

1. Create a protocol dispatch module in the appropriate crate. Register decoders by (transport, port_hint) and (transport, magic_bytes). When a new stream arrives from the Subsection 3 pipeline, attempt identification and route to the appropriate decoder. Support `--protocol grpc --port 8080` user override.
2. Create the gRPC decoder implementing `ProtocolDecoder`:
   - Maintain an `H2Codec` instance per TCP connection.
   - Feed reassembled bytes to `codec.process()`.
   - For each `H2Event::Headers`, use the integrated HPACK to decompress header blocks. Extract `:path` for method name, `:authority`, `grpc-encoding`, `content-type`.
   - For HPACK failures (mid-stream captures): log warning via `tracing::warn!`, set `hpack_degraded: true` on the connection, continue with payload-only analysis. Do NOT silently ignore the error.
   - For each `H2Event::Data`, accumulate bytes per stream. Parse gRPC LPM: read 1 byte compress flag + 4 bytes big-endian length + payload. If compressed, decompress with flate2 using the algorithm from `grpc-encoding` header.
   - For trailing `H2Event::Headers` with `end_stream: true` on a stream that has already seen initial headers, extract `grpc-status` and `grpc-message`.
   - Emit `DebugEvent` for each complete gRPC message (request body, response body) and for call completion (status).
3. Handle gRPC messages spanning multiple DATA frames (accumulate until LPM message_length bytes received).
4. Wire into CLI `prb inspect` to display method name, status, request/response payload summaries.

## Alternatives Ruled Out

- Using hyperium/h2 for frame parsing. Rejected: async client/server, cannot parse offline captures.
- Ignoring gRPC compression. Rejected: common in production, would produce corrupt protobuf.
- Building a custom HTTP/2 parser from scratch. Rejected: too many edge cases (CONTINUATION, padding, priority).
- Using fluke-h2-parse + fluke-hpack from the start instead of h2-sans-io. Evaluated but deferred: h2-sans-io provides a cleaner integrated API. This combination is the documented fallback if h2-sans-io proves buggy.

## Pre-Mortem Risks

- h2-sans-io may have bugs in CONTINUATION assembly for large header sets. Write tests with multi-frame headers.
- gRPC messages larger than a single DATA frame require careful buffer management. Write tests with multi-frame messages.
- HPACK degradation may hide real parsing bugs. Ensure degradation only triggers when the specific HPACK error indicates missing context (dynamic table reference failure), not malformed data.
- Stream ID reuse after RST_STREAM means correlation must scope to stream lifetime, not just stream ID.

## Build and Test Commands

- Build: `cargo build -p prb-protocol-grpc` (exact crate name depends on workspace layout from Subsection 1; adjust if different)
- Test (targeted): `cargo test -p prb-protocol-grpc`
- Test (regression): `cargo test -p prb-core -p prb-decode -p prb-pcap`
- Test (full gate): `cargo test --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_grpc_simple_unary_call`: Parse a gRPC unary call (HEADERS + DATA request, HEADERS + DATA + trailers response) from raw HTTP/2 bytes. Verify method name, request payload, response payload, and grpc-status are correctly extracted.
   - `test_grpc_compressed_message`: Parse a gRPC message with compress_flag=1 and gzip-compressed payload. Verify decompression produces correct protobuf bytes.
   - `test_grpc_streaming`: Parse a server-streaming gRPC call with multiple response messages on the same stream. Verify all messages are extracted with correct ordering.
   - `test_grpc_trailers_only`: Parse a Trailers-Only response (no DATA frames, just trailers with error status). Verify grpc-status and grpc-message are extracted.
   - `test_hpack_degradation`: Feed bytes starting mid-connection (no SETTINGS, no initial HEADERS). Verify warning is logged and payload-only analysis produces valid DebugEvents.
   - `test_grpc_multi_frame_message`: Parse a gRPC message whose LPM payload spans 3 HTTP/2 DATA frames. Verify correct reassembly.
   - `test_protocol_dispatch`: Register gRPC decoder, feed a TCP stream on port 50051, verify it routes to the gRPC decoder.
2. **Regression tests:** All tests from Subsections 1-3 continue passing (`cargo test -p prb-core -p prb-storage -p prb-schema -p prb-decode -p prb-pcap` or equivalent).
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo test --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changed files are in the gRPC decoder crate, protocol dispatch module, and CLI integration. Out-of-scope supporting changes are documented in the builder's final report.
