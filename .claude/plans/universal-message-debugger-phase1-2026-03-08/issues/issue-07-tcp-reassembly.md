---
id: "7"
title: "TCP Reassembly Underscoped"
risk: 6/10
addressed_by_subsections: [3]
---

# Issue 7: TCP Reassembly Underscoped

**Core Problem:**
Phase 9 describes TCP stream reassembly as one phase with three bullet points. Production-quality TCP reassembly is one of the hardest problems in network analysis: FIN/RST handling, simultaneous close, zero-window probing, segment overlap resolution, keep-alive detection, and more.

**Root Cause:**
The plan treats TCP reassembly as a simple ordered-merge problem rather than a full state machine.

**Proposed Fix:**
Use an existing TCP reassembly library rather than building from scratch. Primary candidate: `pcap_tcp_assembler` (GitHub: rus0000/pcap_tcp_assembler) -- designed specifically for PCAP log analysis, tolerates packet loss from capture tools, includes message boundary detection. Secondary candidate: extract and adapt the assembler from `smoltcp` (widely used embedded TCP stack).

**Existing Solutions Evaluated:**
- `pcap_tcp_assembler` (GitHub, MIT license) -- purpose-built for PCAP analysis. Tolerates capture-tool packet loss. Uses modified smoltcp assembler. Best fit.
- `protolens` (crates.io, v0.2.3) -- high-performance TCP reassembly (2-5 GiB/s). More capable than needed but well-maintained. Includes application-layer protocol parsers we don't need.
- `blatta-stream` (GitHub, MIT) -- thin wrapper around smoltcp assembler. Minimal but potentially too minimal.

**Recommendation:** Start with `pcap_tcp_assembler` for its PCAP-specific design. If it proves insufficient, evaluate `protolens` as a heavier but more battle-tested alternative.

**Alternatives Considered:**
- Build from scratch using the RFC 793 state machine. Rejected: months of work to reach the reliability of existing libraries; premature for Phase 1.
- Use `smoltcp` directly. Rejected: smoltcp is designed for embedded networking stacks, not passive analysis. Its assembler is useful but the full crate brings unwanted baggage.

**Pre-Mortem -- What Could Go Wrong:**
- `pcap_tcp_assembler` may not handle all edge cases (e.g., TCP timestamp options, SACK).
- Integrating a third-party assembler with our event model may require significant adapter code.
- Performance may not meet the 100k events/sec target for large captures.

**Risk Factor:** 6/10

**Evidence for Optimality:**
- Existing solutions: `pcap_tcp_assembler` is explicitly designed for the exact use case (offline PCAP TCP reassembly with tolerance for capture artifacts).
- External evidence: Production network analysis tools (Wireshark, Zeek, Suricata) all use dedicated TCP reassembly engines that took years to mature. Building from scratch is not justified for Phase 1.

**Blast Radius:**
- Direct: TCP reassembly module
- Ripple: all TCP-based protocol adapters depend on reassembled streams
