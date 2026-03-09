---
segment: 3
title: "ZMQ Correlation Strategy"
depends_on: [1]
risk: 5/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(correlation): add ZMQ correlation with per-socket-type strategies"
---

# Segment 3: ZMQ Correlation Strategy

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Implement ZMQ correlation with per-socket-type sub-strategies: REQ/REP lockstep, PUB/SUB topic grouping, ROUTER/DEALER envelope identity.

**Depends on:** Segment 1

## Context: Issues Addressed

**S5-3: ZMQ Correlation -- Per-Socket-Type Strategy Required**

- **Core Problem:** The parent plan conflated REQ/REP and ROUTER/DEALER correlation. ZMTP has fundamentally different semantics per socket type: REQ/REP uses strict lockstep alternation on TCP connection (identity frames NOT involved); PUB/SUB uses topic prefix matching on first frame bytes; ROUTER/DEALER is the ONLY pattern where socket identity matters (multi-frame envelopes).
- **Proposed Fix:** Implement three sub-strategies: (1) REQ/REP: correlate by TCP connection, pair by send order (message[0]=request, message[1]=reply, etc.); (2) PUB/SUB: group by topic prefix only, no request-response; (3) ROUTER/DEALER: correlate by identity bytes when present, fall back to connection-level grouping when absent. Socket type from ZMTP greeting frame; if unknown (mid-stream capture), delegate to generic fallback.
- **Pre-Mortem risks:** Socket type detection requires ZMTP greeting capture; ROUTER/DEALER through proxies produce multi-hop envelopes; ZMQ multipart messages (MORE flag) must be reassembled by Subsection 4 before correlation.

## Scope

- `crates/prb-correlation/src/zmq.rs` -- `ZmqCorrelationStrategy` with sub-strategies

## Key Files and Context

- `crates/prb-correlation/src/zmq.rs` -- new file
- `crates/prb-correlation/src/engine.rs` -- register ZMQ strategy
- `crates/prb-core/src/event.rs` -- DebugEvent must carry from Subsection 4's ZMQ decoder:
  - `socket_type: Option<ZmqSocketType>` -- from ZMTP greeting frame (REQ, REP, PUB, SUB, ROUTER, DEALER, PUSH, PULL, PAIR)
  - `topic: Option<Vec<u8>>` -- first frame prefix for PUB/SUB messages
  - `zmq_identity: Option<Vec<u8>>` -- envelope identity for ROUTER/DEALER
  - `connection_id` -- TCP 4-tuple hash (shared with gRPC)
  - `zmq_message_index: Option<u64>` -- sequential message index within connection for REQ/REP pairing

ZMTP socket type correlation semantics (from ZMTP RFCs 28, 29):
- **REQ/REP:** Strict lockstep. Within a TCP connection, messages alternate: request at even indices (0, 2, 4...), reply at odd indices (1, 3, 5...). No identity frames involved.
- **PUB/SUB:** No request-response. Publisher sends messages with topic prefix as first frame bytes. Subscription commands (0x01/0x00 + topic) are metadata events, not data flows.
- **ROUTER/DEALER:** Multi-frame envelopes. Identity frames appear before an empty delimiter frame, then message body. Correlate by identity bytes.
- **PUSH/PULL:** Pipeline pattern. No correlation; each message is independent. Group by connection only.
- **PAIR:** 1:1 exclusive connection. Group all messages on the connection as one flow.

If `socket_type` is None (mid-stream capture lost the ZMTP greeting), delegate to the generic fallback strategy from Segment 1.

## Implementation Approach

1. `ZmqCorrelationStrategy` matches events where `transport == TransportKind::Zmq`.
2. Internal dispatch based on `socket_type` field:
   - `ReqRep`: key = `CorrelationKey::ZmqReqRep { connection_id, pair_index }` where `pair_index = zmq_message_index / 2`
   - `PubSub`: key = `CorrelationKey::ZmqPubSub { topic_prefix }` -- grouping only, no pairing
   - `RouterDealer`: key = `CorrelationKey::ZmqRouter { identity }` if identity present, else `{ connection_id }`
   - `PushPull` / `Pair`: key = `CorrelationKey::ZmqConnection { connection_id }`
   - `None`: return `None` to let engine fall through to generic strategy
3. For PUB/SUB, subscription command events (0x01/0x00 frames) are attached as metadata to the topic flow, not separate flows.

## Alternatives Ruled Out

- Single "(topic, socket identity)" for all socket types (incorrect per ZMTP RFCs; breaks PUB/SUB and REQ/REP)
- Requiring users to specify socket type manually (auto-detectable from ZMTP greeting)
- Using the `zmtp` crate for correlation-side parsing (unmaintained; Subsection 4 handles parsing)

## Pre-Mortem Risks

- Missing socket type from mid-stream captures. Fall back is clean but users may be confused why ZMQ flows show as "generic."
- ROUTER/DEALER through proxies produce multi-hop envelopes. First implementation handles single-hop; document limitation.
- ZMQ multipart messages (MORE flag set) must be fully reassembled by Subsection 4 before correlation. If Subsection 4 emits per-frame events instead of per-message events, pairing breaks.

## Build and Test Commands

- Build: `cargo build -p prb-correlation`
- Test (targeted): `cargo nextest run -p prb-correlation -- zmq`
- Test (regression): `cargo nextest run -p prb-correlation -p prb-cli`
- Test (full gate): `cargo nextest run --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_zmq_req_rep_lockstep`: Alternating messages on same connection paired into request-reply flows. Message 0+1 = flow A, message 2+3 = flow B.
   - `test_zmq_pub_sub_topic_grouping`: Messages with same topic prefix grouped into one flow; different prefixes produce separate flows.
   - `test_zmq_pub_sub_subscription_metadata`: 0x01 subscription events attached as metadata to the topic flow, not separate flows.
   - `test_zmq_router_dealer_identity`: Messages with same identity bytes produce one flow; different identities produce separate flows.
   - `test_zmq_router_dealer_no_identity`: Missing identity falls back to connection-level grouping.
   - `test_zmq_unknown_socket_type_returns_none`: Missing socket_type returns None, causing engine to use generic fallback.
   - `test_zmq_push_pull_connection_grouping`: PUSH/PULL messages grouped by connection.
2. **Regression tests:** All Segment 1 tests and existing workspace tests pass.
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo nextest run --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changes only in `crates/prb-correlation/src/zmq.rs` and strategy registration in `engine.rs`.
