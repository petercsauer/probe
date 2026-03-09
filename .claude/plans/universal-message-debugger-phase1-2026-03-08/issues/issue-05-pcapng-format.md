---
id: "5"
title: "pcapng Format Not Addressed"
risk: 4/10
addressed_by_subsections: [3]
---

# Issue 5: pcapng Format Not Addressed

**Core Problem:**
The plan says "PCAP" throughout all phases but modern tools (Wireshark, tshark, dumpcap) default to pcapng format since Wireshark 1.8 (2012). Users will submit pcapng files. Additionally, pcapng can embed TLS key material and interface metadata, which the tool should exploit.

**Root Cause:**
The plan was written using the generic term "PCAP" without distinguishing between legacy pcap and the modern pcapng container format.

**Proposed Fix:**
Support both formats transparently. Auto-detect format from magic bytes (pcap: `0xa1b2c3d4` or `0xd4c3b2a1`; pcapng: `0x0a0d0d0a`). Use `pcap-parser` (Rusticata) as the primary parsing library, which handles both formats including pcapng's multiple-interface and multiple-section features. Extract embedded TLS keys from pcapng Decryption Secrets Blocks (DSB) when present.

**Existing Solutions Evaluated:**
- `pcap-parser` (crates.io, Rusticata project, ~20K downloads/month, last release Aug 2024) -- supports both pcap and pcapng with zero-copy parsing. Handles multiple sections, interfaces, endianness. Adopted.
- `pcap-file` (crates.io, 6.3M total downloads, last release 3+ years ago) -- older, less actively maintained. Still functional but `pcap-parser` is preferred for pcapng edge cases.

**Recommendation:** Use `pcap-parser` as the primary library. Drop `pcap-file` from the dependency list unless write support is needed (pcap-parser is read-only).

**Alternatives Considered:**
- Support only legacy pcap and ask users to convert. Rejected: poor UX; conversion loses pcapng-specific metadata.

**Pre-Mortem -- What Could Go Wrong:**
- pcapng files with multiple interfaces assign different link types per interface. Each packet must be decoded according to its interface's link type, not a global default.
- pcapng section headers can reset interface numbering. State management across sections is tricky.

**Risk Factor:** 4/10

**Evidence for Optimality:**
- External evidence: pcapng is the IETF-specified format (draft-ietf-opsawg-pcapng) and Wireshark's default since 2012.
- Existing solutions: `pcap-parser` from the Rusticata project is purpose-built for this, handles both formats, and is actively maintained.

**Blast Radius:**
- Direct: PCAP ingest module
- Ripple: TLS decryption module (can receive embedded keys from pcapng DSB)
