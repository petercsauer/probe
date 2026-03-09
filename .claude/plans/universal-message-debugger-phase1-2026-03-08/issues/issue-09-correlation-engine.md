---
id: "9"
title: "Correlation Engine Underspecified"
risk: 6/10
addressed_by_subsections: [5]
---

# Issue 9: Correlation Engine Underspecified

**Core Problem:**
Phase 11 says "connect related messages" using "protocol identifiers, timestamps, correlation keys." Each protocol has completely different correlation semantics: gRPC uses HTTP/2 stream IDs, ZMQ pub/sub has no inherent request-response, DDS uses GUID prefix + entity ID matching. A single generic strategy cannot work.

**Root Cause:**
The plan treats correlation as a single algorithm when it's actually a per-protocol strategy that the core engine orchestrates.

**Proposed Fix:**
Define a `CorrelationStrategy` trait with per-protocol implementations:
- **gRPC:** correlate by (connection_id, HTTP/2 stream ID). Request and response share a stream. Map stream ID to method name from `:path` pseudo-header in HEADERS frame. Stream IDs are monotonically increasing and never reused per RFC 7540 Section 5.1.1.
- **ZMQ:** per-socket-type strategies. REQ/REP: strict lockstep correlation by TCP connection alternation (not socket identity). PUB/SUB: group by topic prefix (first frame bytes); no request-response pairing. ROUTER/DEALER: correlate by envelope identity frames. Socket type auto-detected from ZMTP greeting.
- **DDS/RTPS:** two-phase correlation. Phase A: build discovery cache from SEDP submessages (GUID to topic_name mapping). Phase B: correlate data messages by looking up writer GUID in cache. Key: (domain_id, topic_name). Graceful degradation to GUID-only when discovery traffic is absent.
- **Generic fallback:** correlate by (source IP:port, dest IP:port, timestamp proximity bucket).

The core engine dispatches to the appropriate strategy based on transport type detected during decode.

**Existing Solutions Evaluated:**
- N/A -- correlation logic is domain-specific to our event model. No generic library solves multi-protocol message correlation. Wireshark's per-protocol dissectors (C, GPL) serve as architecture reference.

**Alternatives Considered:**
- Single timestamp-based correlation for all protocols. Rejected: too imprecise; concurrent messages on the same connection would be incorrectly grouped.
- User-defined correlation rules (regex on payload, header matching). Rejected for Phase 1: useful but adds significant complexity. Better as a Phase 2 feature.

**Pre-Mortem -- What Could Go Wrong:**
- gRPC: multiple TCP connections to the same server produce overlapping stream ID sequences; correlation key must include connection_id. GOAWAY frames affect in-flight streams (streams with ID > last_stream_id were not processed).
- ZMQ: mid-stream captures lose the ZMTP greeting frame, making socket type unknown. Correlation must fall back to generic strategy. ROUTER/DEALER through proxies produce multi-hop envelopes requiring deep envelope parsing.
- DDS: topic names are NOT in RTPS DATA submessages -- only in SEDP discovery messages. If discovery traffic was captured separately or is missing, correlation degrades to opaque GUID-only grouping. RTPS vendor-specific extensions may cause parsing failures for non-standard DDS implementations.

**Risk Factor:** 6/10

**Evidence for Optimality:**
- External evidence: Wireshark's gRPC dissector correlates by HTTP/2 stream ID (documented in Wireshark gRPC wiki page).
- External evidence: The DDS specification (OMG DDS-RTPS v2.5) defines entity correlation through GUID prefixes, which is the canonical approach.

**Blast Radius:**
- Direct: correlation engine module
- Ripple: CLI output (flow display depends on correlation quality), protocol adapters (must emit correlation-relevant metadata)
