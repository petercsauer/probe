---
id: "S5-3"
title: "ZMQ Correlation -- Per-Socket-Type Strategy Required"
risk: 5/10
addressed_by_segments: [3]
---
# Issue S5-3: ZMQ Correlation -- Per-Socket-Type Strategy Required

**Core Problem:**
The parent plan says "correlate by (topic, socket identity) for REQ/REP patterns. PUB/SUB has no correlation; group by topic only." Research shows this is substantially wrong. ZMTP has fundamentally different correlation semantics per socket type:
- REQ/REP: strict lockstep alternation on TCP connection (recv, send, recv, send). Identity frames are NOT involved.
- PUB/SUB: messages prefixed with topic bytes (first frame). Subscription commands (0x01 + topic) visible in captured ZMTP traffic.
- ROUTER/DEALER: multi-frame envelopes with identity frames and empty delimiter. This is the ONLY pattern where socket identity matters.

**Root Cause:**
The parent plan conflates REQ/REP correlation (connection-level lockstep) with ROUTER/DEALER correlation (identity envelopes).

**Proposed Fix:**
Implement three ZMQ sub-strategies:

1. **REQ/REP:** Correlate by TCP connection. Within each connection, messages alternate request/reply. Pair by send order: message[0]=request, message[1]=reply, message[2]=request, etc.
2. **PUB/SUB:** No request-response correlation. Group by topic prefix (extracted from first frame bytes). Subscription commands (0x01/0x00 + topic) are metadata events, not data flows.
3. **ROUTER/DEALER:** Parse multi-frame message structure. Identity in envelope frames before empty delimiter. Correlate by identity bytes when present, fall back to connection-level grouping when absent.

Socket type is known from the ZMTP greeting frame (parsed by Subsection 4's ZMQ decoder). Strategy selected based on socket type pair. If socket type unknown (mid-stream capture), delegate to generic fallback.

**Existing Solutions Evaluated:**
- `zmtp` crate (v0.6.0, crates.io) -- provides frame/greeting parsing with `Traffic`, `TrafficReader` types. However: only 54 downloads/90 days, depends on old `byteorder 0.5.3`. Effectively unmaintained. Subsection 4 handles ZMTP parsing; correlation here consumes parsed metadata only.
- `zmtpdump` (GitHub: zeromq/zmtpdump) -- ZeroMQ transport protocol packet analyzer. Reference for ZMTP analysis architecture.
- Wireshark ZMTP dissector -- provides frame-level parsing with identity extraction.

**Alternatives Considered:**
- Single "(topic, socket identity)" strategy for all socket types. Rejected: incorrectly pairs PUB messages as "requests" to other PUB messages.
- Require users to specify socket types manually. Rejected: ZMTP greeting frame contains socket type; auto-detection is possible and preferred.

**Pre-Mortem -- What Could Go Wrong:**
- Socket type detection requires ZMTP greeting capture (connection start). Mid-stream captures lose socket type; fall back to generic.
- ROUTER/DEALER through proxies produce multi-hop envelopes with multiple identity frames. Deep parsing needed.
- ZMQ multipart messages (MORE flag) must be reassembled by Subsection 4 before correlation sees them.

**Risk Factor:** 5/10

**Evidence for Optimality:**
- External evidence: ZMTP RFC 28 (REQ/REP spec) defines strict alternating send/recv that forms lockstep correlation basis.
- External evidence: ZMTP RFC 29 (PUB/SUB spec) defines topic prefix matching on first frame bytes.

**Blast Radius:**
- Direct: ZMQ correlation strategy in `prb-correlation`
- Ripple: requires DebugEvent to carry `socket_type`, `topic`, optionally `identity` from Subsection 4
