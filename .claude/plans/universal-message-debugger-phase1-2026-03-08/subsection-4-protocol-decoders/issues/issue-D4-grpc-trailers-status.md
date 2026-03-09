---
id: "D4"
title: "gRPC Trailers/Status Not Addressed"
risk: 2/10
addressed_by_segments: [1]
---
# Issue D4: gRPC Trailers/Status Not Addressed

**Core Problem:**
gRPC responses end with trailers carried in an HTTP/2 HEADERS frame with END_STREAM. Trailers contain `grpc-status` (0=OK, 1=CANCELLED, 2=UNKNOWN, etc.) and optionally `grpc-message` (error description). Without parsing trailers, the decoder cannot report whether a gRPC call succeeded or failed -- the single most important debugging signal.

**Root Cause:**
The plan focused on request/response data payloads without considering gRPC's status reporting mechanism.

**Proposed Fix:**
When h2-sans-io emits an `H2Event::Headers` with `end_stream: true` on a response stream, parse the headers for `grpc-status` and `grpc-message`. Store these in the `DebugEvent` as `grpc_status: Option<u32>` and `grpc_message: Option<String>`. For Trailers-Only responses (no DATA frames, just trailers), this is the entire response.

**Existing Solutions Evaluated:**
- N/A -- internal implementation. The gRPC status codes are defined in the gRPC spec.

**Alternatives Considered:**
- Skip status extraction; let users grep for it manually. Rejected: status is the single most important debugging signal in gRPC.

**Pre-Mortem -- What Could Go Wrong:**
- Trailers-Only responses (error before any data is sent) might be misidentified as request headers.
- `grpc-status-details-bin` contains a serialized `google.rpc.Status` proto; parsing it requires the Status proto descriptor. Initial implementation should extract the raw bytes and defer detailed parsing.

**Risk Factor:** 2/10

**Evidence for Optimality:**
- External evidence: gRPC spec (PROTOCOL-HTTP2.md) requires trailers for every response.
- External evidence: Wireshark's gRPC dissector prominently displays grpc-status in its protocol tree.

**Blast Radius:**
- Direct: gRPC decoder (trailer parsing)
- Ripple: DebugEvent type (new optional fields), CLI output formatting
