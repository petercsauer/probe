---
segment: 9
title: "Protocol Decode Tree Pane"
depends_on: [6]
risk: 3
complexity: Low
cycle_budget: 2
status: pending
commit_message: "feat(prb-tui): add protocol decode tree pane with tui-tree-widget"
---

# Subsection 4: Protocol Decode Tree Pane

## Purpose

Hierarchical tree view of the selected event's protocol layers and metadata,
inspired by Wireshark's packet detail pane. Uses `tui-tree-widget` for
expand/collapse navigation.

## Tree Structure

For a gRPC event:
```
▶ Event #42
  ├── Timestamp: 2026-03-10 14:00:01.234
  ├── Direction: Outbound (→)
  ▶ Source
  │   ├── Adapter: pcap
  │   ├── Origin: capture.pcap
  │   ├── Src: 10.0.0.1:42837
  │   └── Dst: 10.0.0.2:50051
  ▶ Transport: gRPC
  │   ├── grpc.method: /api.v1.Users/GetUser
  │   ├── h2.stream_id: 1
  │   ├── grpc.encoding: identity
  │   └── grpc.status: 0 (OK)
  ▶ Payload (142 bytes)
  │   ├── Type: Decoded
  │   ├── Schema: api.v1.GetUserRequest
  │   └── Fields: { "user_id": "abc-123" }
  └── Correlation
      ├── StreamId: 1
      └── ConnectionId: 10.0.0.1:42837→10.0.0.2:50051
```

Each tree node carries optional byte range `(offset, len)` for cross-highlighting
with the hex dump pane.

---

## Segment S4.1: Tree Node Model + Conversion

**TreeNode model**:
```rust
pub struct DecodeNode {
    pub label: String,
    pub children: Vec<DecodeNode>,
    pub byte_range: Option<(usize, usize)>,
}
```

**Conversion**: `fn event_to_tree(event: &DebugEvent) -> Vec<DecodeNode>`
- Root: Event ID + timestamp
- Source section (adapter, origin, network addresses)
- Transport section (kind + all metadata keys sorted)
- Payload section (type, size, decoded fields if available)
- Correlation section (all correlation keys)
- Warnings section (if any)

## Segment S4.2: Protocol-Specific Formatters

Protocol-aware formatting for metadata values:
- gRPC: method path highlighting, status code → name mapping (0=OK, 1=CANCELLED, etc.)
- ZMQ: socket type display, topic formatting
- DDS: domain ID, topic name, GUID formatting
- Generic: key=value for unknown metadata
