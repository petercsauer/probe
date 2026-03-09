---
id: "S3-1"
title: "PCAP/pcapng File Reading and Format Detection"
risk: 3/10
addressed_by_segments: [1]
---

# Issue S3-1: PCAP/pcapng File Reading and Format Detection

**Core Problem:**
The pipeline must transparently handle both legacy pcap and modern pcapng formats. pcapng is Wireshark's default since 2012 and can embed TLS key material via Decryption Secrets Blocks (DSB). Users will submit either format without knowing the difference.

**Root Cause:**
Two container formats exist for packet captures with different capabilities. The tool must support both seamlessly.

**Proposed Fix:**
Use `pcap-parser` v0.17.0 as the sole file reading library. Auto-detect format via magic bytes (pcap: `0xa1b2c3d4`/`0xd4c3b2a1`; pcapng: `0x0a0d0d0a`). Use `PcapNGReader` for streaming large files with constant memory. Extract DSB blocks for embedded TLS keys. Track per-interface linktype (pcapng allows different linktypes per interface).

**Existing Solutions Evaluated:**
- `pcap-parser` (Rusticata, v0.17.0, 834K total downloads, MIT/Apache-2.0, actively maintained) -- handles both formats, multiple sections, multiple interfaces, DSB blocks. Adopted.
- `pcap-file` (crates.io, 6.3M total downloads, last release 3+ years ago) -- older, less actively maintained. Rejected: `pcap-parser` handles both formats alone.
- `libpcap` FFI bindings via `pcap` crate -- adds C dependency, designed for live capture. Rejected.

**Alternatives Considered:**
- Using `pcap-file` alongside `pcap-parser` -- rejected, redundant.
- Using `libpcap` FFI bindings -- rejected, adds C dependency and is designed for live capture.

**Pre-Mortem -- What Could Go Wrong:**
- pcapng files with multiple sections reset interface numbering; state management across section boundaries must be tested.
- Unusual pcapng option blocks may cause parse warnings that should not abort ingestion.
- Compressed pcapng sections are not supported by `pcap-parser` (document as known limitation).

**Risk Factor:** 3/10

**Evidence for Optimality:**
- Existing solutions: `pcap-parser` docs confirm DSB support (`DecryptionSecretsBlock` struct) and both-format handling.
- External evidence: pcapng is IETF-specified (draft-ietf-opsawg-pcapng) and Wireshark's default since 2012, making dual-format support mandatory.

**Blast Radius:**
- Direct: new `prb-pcap` crate file reader module
- Ripple: TLS decryption module receives embedded keys from DSB
