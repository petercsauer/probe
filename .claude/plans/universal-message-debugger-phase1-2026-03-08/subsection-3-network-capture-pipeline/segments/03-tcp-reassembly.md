---
segment: 3
title: "TCP Stream Reassembly"
depends_on: [2]
risk: 5/10
complexity: Medium
cycle_budget: 20
status: pending
commit_message: "feat(pcap): add TCP stream reassembly using smoltcp assembler"
---

# Segment 3: TCP Stream Reassembly

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Reassemble individual TCP segments into ordered byte streams per connection, tolerating out-of-order delivery, retransmissions, and capture-tool packet loss.

**Depends on:** Segment 2

## Context: Issues Addressed

**S3-3 (TCP Stream Reassembly Library Selection):** The parent plan recommended `pcap_tcp_assembler` which has 0 stars and is not on crates.io. **Proposed fix:** Use `smoltcp`'s `storage::Assembler` as the core segment reassembly engine, wrapped in a custom connection tracker. `smoltcp` (12K+ stars, MIT/Apache-2.0) is battle-tested. The `Assembler` handles out-of-order segments, overlaps, and gap tracking. Build ~300 lines of connection tracking around it (keyed by 4-tuple, handling SYN/FIN/RST state transitions, configurable timeout). **Pre-mortem risks:** smoltcp's `Assembler` uses fixed-size buffers; connection tracking must tolerate captures starting mid-connection (no SYN); high connection counts (10K+) may cause memory pressure; TCP timestamp option may affect sequence number wrapping for long-lived connections.

## Scope

- `prb-pcap` crate, module `tcp`

## Key Files and Context

`smoltcp` (crates.io, v0.12+, MIT/Apache-2.0, 12K+ GitHub stars) provides `storage::Assembler` for TCP segment reassembly. The `Assembler` tracks contiguous ranges of received data and reports gaps. We wrap it with: (1) A `ConnectionTracker` keyed by 4-tuple `(src_ip, src_port, dst_ip, dst_port)` mapping packets to connections. (2) A `TcpStreamState` per connection tracking: SYN/FIN/RST flags seen, initial sequence number, current assembler state, timeout timer. (3) A `ReassembledStream` output type: `{ connection_key, direction: Client|Server, data: Vec<u8>, is_complete: bool, missing_ranges: Vec<Range<u64>> }`. The module must handle captures starting mid-connection (no SYN): infer initial sequence number from first seen segment. Packet loss tolerance: when a gap persists beyond a configurable threshold (packet count or timeout), skip the gap and continue reassembly, logging a warning. `pcap_tcp_assembler` (GitHub, MIT) demonstrates this pattern by wrapping smoltcp's assembler with capture-specific gap tolerance -- use as design reference.

## Implementation Approach

Add `smoltcp` as a dependency (feature-gate to pull in `storage` module). For each TCP segment from the normalizer: (1) look up or create connection state by 4-tuple, (2) adjust sequence number relative to ISN, (3) feed payload into smoltcp's `Assembler`, (4) if assembler reports contiguous data, yield it to callback. Track bidirectional streams (client-to-server, server-to-client) independently. Connection cleanup: on FIN/RST, flush remaining data and mark stream complete. Timeout: connections with no new data for N seconds (configurable, default 30s) are flushed and closed. Expose a `TcpReassembler` struct with `fn process_segment(&mut self, segment: TcpSegment) -> Vec<StreamEvent>` API.

## Alternatives Ruled Out

- Using `pcap_tcp_assembler` directly (not on crates.io, 0 community adoption, single maintainer risk).
- Using `protolens` (TCP reassembly not separable from bundled protocol parsing).
- Building from scratch without smoltcp (reinventing a complex, error-prone wheel).

## Pre-Mortem Risks

- smoltcp's `Assembler` uses fixed-size contiguous range tracking; may need custom buffer growth strategy for large streams.
- High connection counts (10K+) may cause memory pressure -- need per-connection buffer limits and eviction strategy.
- Captures with heavy packet loss produce many fragmented stream segments rather than clean byte streams.
- TCP timestamp option handling may affect sequence number wrapping detection for connections exceeding 4GB.

## Build and Test Commands

- Build: `cargo build -p prb-pcap`
- Test (targeted): `cargo test -p prb-pcap -- tcp`
- Test (regression): `cargo test -p prb-pcap -- reader normalize`
- Test (full gate): `cargo test --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_simple_stream`: reassembles a 3-segment in-order TCP stream
   - `test_out_of_order`: reassembles segments arriving out of order
   - `test_retransmission`: handles duplicate segments without producing duplicate data
   - `test_packet_loss_tolerance`: skips gap after threshold and continues reassembly with warning
   - `test_bidirectional`: tracks client-to-server and server-to-client independently
   - `test_fin_rst_cleanup`: connection state is flushed on FIN and RST
   - `test_mid_connection_start`: handles capture starting mid-connection (no SYN)
   - `test_connection_timeout`: idle connections are flushed after timeout
   - `test_multiple_connections`: tracks 100+ concurrent connections without error
2. **Regression tests:** `cargo test -p prb-pcap -- reader normalize`
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo test --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changes in `prb-pcap/src/tcp.rs` and test fixtures only.
