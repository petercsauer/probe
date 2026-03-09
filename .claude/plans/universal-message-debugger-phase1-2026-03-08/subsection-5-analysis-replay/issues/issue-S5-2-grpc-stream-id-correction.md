---
id: "S5-2"
title: "gRPC Correlation -- Stream ID Semantics Correction"
risk: 3/10
addressed_by_segments: [2]
---
# Issue S5-2: gRPC Correlation -- Stream ID Semantics Correction

**Core Problem:**
The parent plan's Issue 9 pre-mortem states "gRPC stream IDs are reused after RST_STREAM; correlation must scope to a connection lifetime." This is factually incorrect per RFC 7540 Section 5.1.1. Stream IDs are monotonically increasing and never reused within an HTTP/2 connection. The actual risks are: (a) multiple TCP connections to the same server produce overlapping stream ID sequences, (b) GOAWAY frames signal connection teardown and affect in-flight streams.

**Root Cause:**
Confusion between stream ID reuse (which does not happen per RFC 7540) and connection multiplexing (which does).

**Proposed Fix:**
Implement gRPC correlation with key `(connection_id, stream_id)` where `connection_id` is derived from the TCP 4-tuple (src_ip, src_port, dst_ip, dst_port) assigned during Subsection 3's TCP reassembly. Extract method name from the `:path` pseudo-header in the HEADERS frame (populated by Subsection 4's gRPC decoder). Group request HEADERS + DATA and response HEADERS + DATA + TRAILERS by stream ID within a connection.

Edge cases:
- GOAWAY: streams with ID > last_stream_id are flagged as interrupted
- RST_STREAM: mark the flow as errored with the HTTP/2 error code
- Streaming RPCs (server-streaming, client-streaming, bidi): one stream ID carries multiple DATA frames -- this is one flow, not multiple

**Existing Solutions Evaluated:**
- N/A -- correlation logic is specific to our event model. Wireshark's gRPC dissector (C, GPL) is the reference implementation; architecture is transferable, not code.

**Alternatives Considered:**
- Correlate by method name only without stream ID. Rejected: multiple concurrent calls to the same method are indistinguishable without stream ID.

**Pre-Mortem -- What Could Go Wrong:**
- Connection ID relies on TCP 4-tuple from Subsection 3. If TCP reassembly doesn't propagate this into DebugEvent, correlation fails. Must verify DebugEvent carries connection context.
- HPACK decompression failure (mid-stream capture, see parent plan Issue 2) loses method name. Graceful degradation: correlate by stream ID without method label.
- Long-lived gRPC connections with thousands of streams produce large flow sets.

**Risk Factor:** 3/10

**Evidence for Optimality:**
- External evidence: RFC 7540 Section 5.1.1 states stream IDs "MUST be numerically greater than all streams that the initiating endpoint has opened or reserved."
- External evidence: Wireshark's gRPC dissector correlates by HTTP/2 stream ID within connection (wiki.wireshark.org/gRPC).

**Blast Radius:**
- Direct: gRPC correlation strategy in `prb-correlation`
- Ripple: requires DebugEvent to carry `connection_id` and `stream_id` from Subsection 4
