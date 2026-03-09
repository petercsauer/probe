---
id: "10"
title: "Missing Network Layer Handling"
risk: 5/10
addressed_by_subsections: [3]
---

# Issue 10: Missing Network Layer Handling

**Core Problem:**
The plan's PCAP pipeline jumps from "packet parsing" to "transport decoding" without addressing several network-layer concerns: (a) IP fragmentation reassembly (critical for large UDP/DDS messages), (b) pcap linktype detection (captures from different interfaces produce different link-layer frames), (c) VLAN tag and encapsulation stripping (common in enterprise captures).

**Root Cause:**
The plan models the network stack as Ethernet → IP → TCP/UDP, ignoring the real-world variations between the capture point and the transport layer.

**Proposed Fix:**
Add a network normalization layer between raw packet parsing and protocol decoding:
1. **Linktype dispatch:** Read pcap/pcapng link-layer header type. Support at minimum: Ethernet (1), Raw IP (101), Linux cooked capture SLL (113), SLL2 (276), Loopback/Null (0).
2. **VLAN stripping:** `etherparse` already handles 802.1Q tags. Expose VLAN ID as event metadata.
3. **IP fragment reassembly:** Implement a fragment reassembly buffer keyed by (src IP, dst IP, IP ID, protocol). Timeout incomplete fragments after a configurable window.

**Existing Solutions Evaluated:**
- `etherparse` (crates.io, actively maintained) -- handles Ethernet, VLAN, IPv4/IPv6, TCP, UDP. Supports "lax" parsing for truncated packets. Does not handle IP fragmentation (noted in docs as requiring allocation). Does handle 802.1Q.
- `pcap-parser` -- provides linktype from pcap/pcapng file headers but does not parse packets. Complementary to etherparse.
- No Rust crate specifically handles IP fragment reassembly for offline analysis. Must be built or adapted.

**Recommendation:** Use `etherparse` for link-through-transport parsing. Build a small IP fragment reassembly buffer (~150 lines) using a `HashMap<FragmentKey, FragmentBuffer>` with configurable timeout. Use `pcap-parser` linktype to determine the entry point for etherparse (skip Ethernet header for raw IP captures, etc.).

**Alternatives Considered:**
- Ignore IP fragmentation. Rejected: DDS/RTPS messages commonly exceed MTU and fragment; ignoring this produces corrupt protocol data.
- Use `pnet` crate for comprehensive packet handling. Rejected: `pnet` requires libpcap and is designed for live capture, not offline parsing.

**Pre-Mortem -- What Could Go Wrong:**
- Fragment reassembly buffer grows unbounded if captures contain many incomplete fragment trains.
- Unusual linktypes (e.g., USB capture, Bluetooth HCI) will cause opaque parse failures.
- VXLAN/GRE tunneling adds another encapsulation layer not handled by this fix.

**Risk Factor:** 5/10

**Evidence for Optimality:**
- External evidence: `etherparse` docs explicitly list supported protocols and note the IP fragmentation limitation, confirming it must be handled separately.
- External evidence: Wireshark's packet dissection pipeline follows the exact same architecture: linktype dispatch → link layer → network layer (with defrag) → transport layer.

**Blast Radius:**
- Direct: PCAP ingest pipeline (new normalization layer)
- Ripple: all protocol decoders receive normalized TCP/UDP payloads instead of raw packets
