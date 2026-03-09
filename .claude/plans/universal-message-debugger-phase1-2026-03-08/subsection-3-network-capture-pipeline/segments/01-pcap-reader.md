---
segment: 1
title: "PCAP/pcapng File Reader"
depends_on: []
risk: 3/10
complexity: Low
cycle_budget: 10
status: pending
commit_message: "feat(pcap): add PCAP/pcapng file reader with DSB extraction"
---

# Segment 1: PCAP/pcapng File Reader

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Create a file reader module that transparently reads both pcap and pcapng formats, extracts per-packet data with linktype, and extracts embedded TLS keys from pcapng DSB blocks.

**Depends on:** Subsection 1 (Cargo workspace, core traits, error conventions) and Subsection 2 (MCAP storage) must be complete.

## Context: Issues Addressed

**S3-1 (PCAP/pcapng File Reading and Format Detection):** The pipeline must transparently handle both legacy pcap and modern pcapng formats. pcapng is Wireshark's default since 2012 and can embed TLS key material via Decryption Secrets Blocks (DSB). Users will submit either format without knowing the difference. **Proposed fix:** Use `pcap-parser` v0.17.0 as the sole file reading library. Auto-detect format via magic bytes (pcap: `0xa1b2c3d4`/`0xd4c3b2a1`; pcapng: `0x0a0d0d0a`). Use `PcapNGReader` for streaming large files with constant memory. Extract DSB blocks for embedded TLS keys. Track per-interface linktype. **Pre-mortem risks:** pcapng section boundary resets interface numbering; unusual option blocks may cause parse warnings; compressed pcapng sections not supported (document as known limitation).

## Scope

- New crate `prb-pcap`, module `reader`

## Key Files and Context

The `CaptureAdapter` trait from `prb-core` (defined in Subsection 1) must be implemented. Error types use `thiserror` per project convention (parent plan Issue 11). `pcap-parser` v0.17.0 provides `PcapNGReader` for streaming reads, `PcapCapture` for in-memory reads, and `DecryptionSecretsBlock` for DSB extraction. pcap magic: `0xa1b2c3d4`/`0xd4c3b2a1`; pcapng magic: `0x0a0d0d0a`. pcapng tracks per-interface linktype via Interface Description Blocks; each Enhanced Packet Block references an interface ID.

## Implementation Approach

Create `PcapFileReader` struct wrapping `pcap-parser`'s streaming reader (works for both formats when using auto-detection from magic bytes). Yield `(linktype, timestamp, raw_bytes)` tuples per packet. Collect DSB blocks into a `TlsKeyStore` (simple `HashMap` keyed by client_random bytes). Expose `fn embedded_tls_keys(&self) -> &TlsKeyStore`. Handle both-endian pcap files (pcap-parser handles this internally). Test with crafted minimal pcap and pcapng fixtures generated programmatically using `etherparse::PacketBuilder`.

## Alternatives Ruled Out

- Using `pcap-file` (older, redundant for read-only use).
- Using libpcap FFI (adds C dependency, designed for live capture).
- Reading entire file into memory (breaks on multi-GB captures).

## Pre-Mortem Risks

- pcapng section boundary resets interface numbering -- must test with multi-section fixtures.
- Large files must stream without OOM -- verify constant memory usage with >1MB generated fixture.
- Compressed pcapng sections are not supported by `pcap-parser` -- document as known limitation.

## Build and Test Commands

- Build: `cargo build -p prb-pcap`
- Test (targeted): `cargo test -p prb-pcap -- reader`
- Test (regression): `cargo test -p prb-core -p prb-storage`
- Test (full gate): `cargo test --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_read_pcap_legacy`: reads a legacy pcap file, yields correct packet count and linktype
   - `test_read_pcapng`: reads a pcapng file with multiple interfaces, yields correct per-packet linktype
   - `test_read_pcapng_dsb`: reads pcapng with embedded DSB, extracts TLS key material correctly
   - `test_format_autodetect`: auto-detects pcap vs pcapng from magic bytes
   - `test_streaming_large_file`: reads a >1MB generated pcap without exceeding bounded memory
2. **Regression tests:** all `prb-core` and `prb-storage` tests pass
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo test --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changes limited to `prb-pcap` crate and workspace `Cargo.toml`.
