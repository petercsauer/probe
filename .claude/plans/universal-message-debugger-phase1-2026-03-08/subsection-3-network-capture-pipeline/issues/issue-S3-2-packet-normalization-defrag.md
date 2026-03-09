---
id: "S3-2"
title: "Network Packet Normalization and IP Defragmentation"
risk: 4/10
addressed_by_segments: [2]
---

# Issue S3-2: Network Packet Normalization and IP Defragmentation

**Core Problem:**
Raw packets from PCAP files arrive with different link-layer encapsulations (Ethernet, SLL, SLL2, Raw IP, Loopback) and may be IP-fragmented. The pipeline must normalize all packets to a common representation before TCP/UDP processing. The parent plan claims IP fragment reassembly must be built from scratch (~150 lines). This is now incorrect -- `etherparse` v0.19.0 provides `defrag::IpDefragPool`. The parent plan also implies etherparse handles SLL2, but it does not.

**Root Cause:**
Real-world captures come from diverse capture points (physical NIC, `any` device, loopback, tunnels) producing different link-layer headers. The parent plan was written against an older etherparse version without the `defrag` module and without verifying SLL2 support.

**Proposed Fix:**
Use `etherparse` v0.19.0 for Ethernet/SLL/VLAN/IP/TCP/UDP parsing. Use its `defrag` module (`IpDefragPool`) for IP fragment reassembly. For SLL2 (linktype 276), implement a thin custom parser (~40 lines) since etherparse does not support SLL2; use `pcap-parser`'s `get_packetdata_linux_sll2()` to extract the protocol type and payload, then feed into etherparse's `from_ip()` for network-layer-and-above parsing. Linktype dispatch: Ethernet (1) and Loopback/Null (0) through `from_ethernet()`; SLL (113) through `from_linux_sll()`; Raw IP (101) through `from_ip()`; SLL2 (276) through custom parser + `from_ip()`.

**Existing Solutions Evaluated:**
- `etherparse` v0.19.0 (5.6M+ downloads, actively maintained, MIT/Apache-2.0) -- includes `defrag` module for IP fragment reassembly. Supports Ethernet II, 802.1Q VLAN (single and double), SLL v1, IPv4, IPv6, TCP, UDP. Does NOT support SLL2. Adopted.
- `pnet` crate -- requires libpcap, live capture focus. Rejected.
- `pkts` crate -- less mature than etherparse. Rejected.
- Building IP defrag from scratch -- rejected now that etherparse provides `IpDefragPool`.

**Alternatives Considered:**
- Building IP fragment reassembly from scratch (~150 lines) -- rejected, etherparse now provides this natively via `defrag::IpDefragPool`.
- Using `pnet` for comprehensive packet handling -- rejected, requires libpcap C dependency and targets live capture.

**Pre-Mortem -- What Could Go Wrong:**
- `IpDefragPool` may grow unbounded with many incomplete fragment trains; need a cleanup/timeout strategy with configurable max buffer count.
- Loopback/Null linktype (0) has platform-dependent header format (4-byte AF value: AF_INET6=30 on macOS, AF_INET6=10 on Linux); must handle both.
- Double VLAN tags (QinQ) need testing.
- VXLAN/GRE tunneled traffic adds another encapsulation layer not handled here (document as known limitation).
- IPv6 Jumbograms are not supported by etherparse (document as known limitation).

**Risk Factor:** 4/10

**Evidence for Optimality:**
- Existing solutions: etherparse docs confirm `defrag` module with `IpDefragPool` for both IPv4 and IPv6 fragment reassembly. `LinuxSllHeader` support confirmed. GitHub issue #97 confirms SLL2 is not yet supported.
- External evidence: Wireshark's packet dissection pipeline follows the same architecture: linktype dispatch, link layer, network layer (with defrag), transport layer.

**Blast Radius:**
- Direct: packet normalization module in `prb-pcap`
- Ripple: all downstream processing (TCP reassembly, UDP extraction) receives normalized packets
