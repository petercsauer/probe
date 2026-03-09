---
segment: 2
title: "gRPC Correlation Strategy"
depends_on: [1]
risk: 3/10
complexity: Low
cycle_budget: 10
status: pending
commit_message: "feat(correlation): add gRPC correlation strategy by connection and stream ID"
---

# Segment 2: gRPC Correlation Strategy

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Implement gRPC-specific correlation by (connection_id, HTTP/2 stream_id) with method name extraction.

**Depends on:** Segment 1

## Context: Issues Addressed

**S5-2: gRPC Correlation -- Stream ID Semantics Correction**

- **Core Problem:** The parent plan incorrectly stated that gRPC stream IDs are reused after RST_STREAM. Per RFC 7540 Section 5.1.1, stream IDs are monotonically increasing and never reused within an HTTP/2 connection. The actual risks are: multiple TCP connections produce overlapping stream ID sequences, and GOAWAY frames affect in-flight streams.
- **Proposed Fix:** Implement gRPC correlation with key `(connection_id, stream_id)` where `connection_id` is derived from the TCP 4-tuple. Extract method name from `:path` pseudo-header in HEADERS frame. Group request HEADERS + DATA and response HEADERS + DATA + TRAILERS by stream ID within a connection. GOAWAY: streams with ID > last_stream_id flagged as interrupted. RST_STREAM: mark flow as errored with HTTP/2 error code. Streaming RPCs: one stream ID carries multiple DATA frames = one flow.
- **Pre-Mortem risks:** Missing `connection_id` in DebugEvent from Subsection 4; HPACK failure loses method name (graceful degradation: flow without method label); very long-lived connections with thousands of streams produce large flow sets.

## Scope

- `crates/prb-correlation/src/grpc.rs` -- `GrpcCorrelationStrategy`

## Key Files and Context

- `crates/prb-correlation/src/grpc.rs` -- new file
- `crates/prb-correlation/src/engine.rs` -- register gRPC strategy in default engine constructor
- `crates/prb-core/src/event.rs` -- DebugEvent must carry: `connection_id` (TCP 4-tuple hash from Subsection 3), `stream_id` (u32 from Subsection 4's gRPC decoder), `method` (Option<String> from `:path` pseudo-header in HEADERS frame), `grpc_status` (Option from trailers)
- HTTP/2 stream IDs are odd (client-initiated) and monotonically increasing per RFC 7540 Section 5.1.1. They are NEVER reused within a connection. Do not implement stream ID reuse handling -- it is unnecessary.
- GOAWAY frame contains `last_stream_id`. Streams with ID > that value were not processed.
- RST_STREAM terminates a single stream with an error code.
- Streaming RPCs: one stream ID carries multiple DATA frames. This is one flow, not multiple.
- A connection can carry thousands of concurrent streams. Each is an independent flow.

## Implementation Approach

1. `GrpcCorrelationStrategy` matches events where `transport == TransportKind::Grpc`.
2. Correlation key: `CorrelationKey::Grpc { connection_id, stream_id }`.
3. Flow metadata: extract `method` from first HEADERS event in the flow, `status` from TRAILERS event.
4. Streaming RPCs: multiple DATA frames on same stream = one flow (already handled by key design).
5. RST_STREAM: mark flow as `errored` with error code in metadata.
6. GOAWAY: flows with stream_id > goaway.last_stream_id get `interrupted` status in metadata.

## Alternatives Ruled Out

- Correlating by method name only (concurrent calls to same method indistinguishable)
- Worrying about stream ID reuse (does not happen per RFC 7540)
- Implementing HPACK decompression in correlation (belongs in Subsection 4)

## Pre-Mortem Risks

- Missing `connection_id` in DebugEvent from Subsection 4. Verify field exists during implementation.
- HPACK failure loses method name. Graceful degradation: flow without method label, log warning.
- Very long-lived connections with thousands of streams produce large flow sets. Test with high stream count.

## Build and Test Commands

- Build: `cargo build -p prb-correlation`
- Test (targeted): `cargo nextest run -p prb-correlation -- grpc`
- Test (regression): `cargo nextest run -p prb-correlation -p prb-cli`
- Test (full gate): `cargo nextest run --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_grpc_correlates_by_connection_and_stream`: Events from same (conn, stream) produce one flow; different streams produce separate flows.
   - `test_grpc_extracts_method_name`: Flow metadata contains method from HEADERS `:path`.
   - `test_grpc_handles_rst_stream`: RST_STREAM event marks flow as errored with code in metadata.
   - `test_grpc_streaming_rpc`: Multiple DATA frames on same stream = one flow with correct event count.
   - `test_grpc_multiple_connections`: Same stream_id on different connections = separate flows.
   - `test_grpc_goaway_marks_interrupted`: Streams above GOAWAY's last_stream_id flagged as interrupted.
2. **Regression tests:** All Segment 1 tests and existing workspace tests pass.
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo nextest run --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changes only in `crates/prb-correlation/src/grpc.rs` and strategy registration in `engine.rs`.
