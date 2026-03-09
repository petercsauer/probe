---
id: "S3-3"
title: "TCP Stream Reassembly Library Selection"
risk: 5/10
addressed_by_segments: [3]
---

# Issue S3-3: TCP Stream Reassembly Library Selection

**Core Problem:**
The parent plan recommends `pcap_tcp_assembler` as the primary TCP reassembly library. This crate has 0 stars, 0 forks, is not published on crates.io, and was created by a single developer with last activity in November 2024. Using it as a foundational dependency is risky.

**Root Cause:**
Library selection in the parent plan was based on use-case fit without evaluating community adoption and maintenance indicators.

**Proposed Fix:**
Use `smoltcp`'s `storage::Assembler` as the core segment reassembly engine, wrapped in a custom connection tracker. `smoltcp` (12K+ stars, actively maintained, MIT/Apache-2.0) is the most battle-tested Rust TCP implementation. The `Assembler` handles out-of-order segments, overlaps, and gap tracking. We build ~300 lines of connection tracking around it (keyed by 4-tuple, handling SYN/FIN/RST state transitions, configurable timeout). This approach gives us a proven reassembly core with our own PCAP-specific tolerance for missing segments.

**Existing Solutions Evaluated:**
- `smoltcp` v0.12+ (crates.io, MIT/Apache-2.0, 12K+ GitHub stars, 1.4K+ forks) -- mature embedded TCP stack. `storage::Assembler` is the exact component needed. Adopted.
- `pcap_tcp_assembler` (GitHub, MIT) -- purpose-built for PCAP but 0 stars, not on crates.io, single maintainer. Rejected as primary due to adoption risk. Useful as design reference (it wraps smoltcp's assembler, validating our approach).
- `protolens` v0.2.3 (crates.io, MIT) -- high performance (2-5 GiB/s) but bundles application-layer protocol parsers. TCP reassembly API may not be separable from protocol parsing. Rejected for composability concerns.
- `pcapsql-core` v0.3.1 (crates.io, MIT) -- has TCP reassembly but brings large dependency graph including `anyhow` in library code. Rejected for library-level use due to convention conflict.
- Building from scratch using RFC 793 state machine -- rejected, months of work to reach reliability of existing libraries.

**Alternatives Considered:**
- Adopting `pcap_tcp_assembler` as a git dependency -- rejected, single maintainer risk and no crates.io presence means no versioning guarantees.
- Using `protolens` just for TCP reassembly -- rejected, tight coupling with its protocol parsers makes selective use impractical.

**Pre-Mortem -- What Could Go Wrong:**
- `smoltcp`'s `Assembler` is designed for embedded contexts with fixed-size buffers; our PCAP use case needs dynamic allocation. May need to adapt buffer management.
- Connection tracking for PCAP must tolerate captures that start mid-connection (no SYN seen); must infer initial sequence number from first seen segment.
- Performance with high connection counts (10K+) needs benchmarking; HashMap lookups per packet must not become a bottleneck.
- TCP timestamp option handling may affect sequence number wrapping detection for long-lived connections.

**Risk Factor:** 5/10

**Evidence for Optimality:**
- Existing solutions: smoltcp GitHub (12K+ stars, 1.4K+ forks) confirms active maintenance and battle-tested status. `pcap_tcp_assembler` README confirms it wraps smoltcp's assembler, validating the approach.
- External evidence: Production network analysis tools (Wireshark, Zeek, Suricata) all use dedicated TCP reassembly engines that took years to mature, confirming build-from-scratch is not viable for Phase 1.

**Blast Radius:**
- Direct: TCP reassembly module in `prb-pcap`
- Ripple: all TCP-based protocol decoders (gRPC, ZMQ) depend on reassembled streams
