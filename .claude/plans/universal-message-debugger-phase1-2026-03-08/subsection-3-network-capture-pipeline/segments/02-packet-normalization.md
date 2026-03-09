---
segment: 2
title: "Packet Parsing and Network Normalization"
depends_on: [1]
risk: 4/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(pcap): add packet normalization with linktype dispatch and IP defrag"
---

# Segment 2: Packet Parsing and Network Normalization

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Parse raw packets from the file reader into normalized (IP header + transport header + payload) tuples, handling multiple linktypes, VLAN stripping, and IP fragment reassembly.

**Depends on:** Segment 1

## Context: Issues Addressed

**S3-2 (Network Packet Normalization and IP Defragmentation):** Raw packets arrive with different link-layer encapsulations (Ethernet, SLL, SLL2, Raw IP, Loopback) and may be IP-fragmented. The pipeline must normalize all packets to a common representation. `etherparse` v0.19.0 provides `defrag::IpDefragPool` for IP fragment reassembly. etherparse does NOT support SLL2 (linktype 276). **Proposed fix:** Use `etherparse` v0.19.0 for Ethernet/SLL/VLAN/IP/TCP/UDP parsing. Use `IpDefragPool` for fragment reassembly. For SLL2, implement a thin custom parser (~40 lines) using `pcap-parser`'s `get_packetdata_linux_sll2()` to extract protocol type and payload, then feed into etherparse's `from_ip()`. Linktype dispatch: Ethernet (1) and Loopback/Null (0) through `from_ethernet()`; SLL (113) through `from_linux_sll()`; Raw IP (101) through `from_ip()`; SLL2 (276) through custom parser + `from_ip()`. **Pre-mortem risks:** `IpDefragPool` unbounded growth with incomplete fragment trains; Loopback AF values differ (AF_INET6=30 on macOS, 10 on Linux); double VLAN (QinQ) needs testing; IPv6 Jumbograms not supported (document as known limitation).

## Scope

- `prb-pcap` crate, module `normalize`

## Key Files and Context

`etherparse` v0.19.0 provides: `SlicedPacket::from_ethernet()` for Ethernet/VLAN, `SlicedPacket::from_linux_sll()` for SLL v1, `SlicedPacket::from_ip()` for Raw IP. `etherparse::defrag::IpDefragPool` handles IPv4/IPv6 fragment reassembly with configurable buffer limits. For SLL2 (linktype 276): etherparse does NOT support it; use `pcap-parser::data::get_packetdata_linux_sll2()` to extract protocol type + payload, then feed into etherparse's `from_ip()` or `from_ether_type()`. Loopback/Null (linktype 0): 4-byte AF family header; AF_INET=2 for IPv4, AF_INET6=30 (macOS) or 10 (Linux) for IPv6; strip header and use `from_ip()`. Output type: `NormalizedPacket { timestamp, src_ip, dst_ip, transport: TcpSegment | UdpDatagram | Other, vlan_id: Option<u16>, raw_payload: &[u8] }`.

## Implementation Approach

Create a `PacketNormalizer` struct that: (1) dispatches on linktype to the appropriate etherparse entry point, (2) extracts VLAN IDs from 802.1Q headers, (3) feeds IP-fragmented packets into `IpDefragPool` and yields reassembled datagrams, (4) separates TCP segments and UDP datagrams. For SLL2, write a minimal `LinuxSll2Header::from_slice()` parser (~40 lines: 20 bytes header, extract protocol_type at offset 0 and interface_index at offset 4). UDP datagrams are immediately available as complete payloads. TCP segments are yielded individually for the reassembler (Segment 3). Configure `IpDefragPool` with a max buffer count and per-packet timeout to prevent unbounded memory growth.

## Alternatives Ruled Out

- Using `pnet` (requires libpcap, live capture focus).
- Building custom IP defrag (etherparse now provides it).
- Ignoring SLL2 (too common in Linux `any` device captures to skip).

## Pre-Mortem Risks

- `IpDefragPool` unbounded growth with many incomplete fragment trains -- must enforce max buffer count and timeout.
- Loopback AF values differ between macOS (AF_INET6=30) and Linux (AF_INET6=10) -- must handle both.
- Double VLAN tags (QinQ) need testing via etherparse's `DoubleVlanHeader` support.
- IPv6 Jumbograms are not supported by etherparse -- document as known limitation.

## Build and Test Commands

- Build: `cargo build -p prb-pcap`
- Test (targeted): `cargo test -p prb-pcap -- normalize`
- Test (regression): `cargo test -p prb-pcap -- reader`
- Test (full gate): `cargo test --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_ethernet_ipv4_tcp`: parses standard Ethernet+IPv4+TCP packet
   - `test_ethernet_ipv4_udp`: parses standard Ethernet+IPv4+UDP packet
   - `test_vlan_single`: strips single VLAN tag, exposes VLAN ID in metadata
   - `test_vlan_double`: strips double VLAN (QinQ) tags correctly
   - `test_sll_v1`: parses SLL v1 encapsulated packet (linktype 113)
   - `test_sll_v2`: parses SLL v2 encapsulated packet (linktype 276) via custom parser
   - `test_raw_ip`: parses Raw IP (linktype 101) packet
   - `test_loopback_null`: parses Loopback/Null (linktype 0) on both macOS and Linux AF values
   - `test_ip_fragment_reassembly`: reassembles a 3-fragment IPv4 packet via `IpDefragPool`
   - `test_ip_fragment_timeout`: incomplete fragment train is cleaned up after configured limit
   - `test_ipv6_fragment`: reassembles IPv6 fragmented packet
2. **Regression tests:** `cargo test -p prb-pcap -- reader`
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo test --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changes in `prb-pcap/src/normalize.rs`, custom SLL2 parser, and test fixtures only.
