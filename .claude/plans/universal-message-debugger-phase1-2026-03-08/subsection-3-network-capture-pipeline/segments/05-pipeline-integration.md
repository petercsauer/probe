---
segment: 5
title: "Pipeline Integration and CLI Extension"
depends_on: [1, 2, 3, 4]
risk: 4/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(pcap): integrate capture pipeline and extend CLI for PCAP ingest"
---

# Segment 5: Pipeline Integration and CLI Extension

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Wire Segments 1-4 into an end-to-end pipeline implementing `CaptureAdapter` for PCAP files, extend the CLI with `prb ingest <capture> [--tls-keylog <keys.log>]`, and add integration tests with real-world PCAP fixtures.

**Depends on:** Segments 1-4

## Context: Issues Addressed

**S3-5 (Pipeline Architecture and Data Flow):** The four components (file reader, packet normalizer, TCP reassembler, TLS decryptor) must be composed into a coherent pipeline producing reassembled byte streams for protocol decoders. Errors must be handled gracefully without aborting ingest. **Proposed fix:** Layered pipeline: File Reader -> Packet Normalizer -> TCP Reassembler + UDP Extractor -> TLS Decryptor (optional) -> DebugEvent emission -> MCAP storage. Each stage produces typed output. Errors produce warnings (logged via `tracing`) and skip the affected packet/stream. Pipeline implements `CaptureAdapter` trait. **Pre-mortem risks:** Stage boundaries may cause unnecessary copies (use `bytes::Bytes`); large captures need bounded memory; streaming vs batch tradeoff for TCP reassembly.

**S3-6 (CLI Extension for PCAP Ingest):** CLI must support `prb ingest capture.pcapng [--tls-keylog keys.log]` with progress reporting and error summaries. **Proposed fix:** Add `--tls-keylog <path>` flag to `prb ingest`. Auto-detect file format (JSON fixture vs PCAP/pcapng) from magic bytes. Report progress (packet count, stream count, bytes processed). Summarize warnings at end. **Pre-mortem risks:** Progress reporting must not slow ingest; error summary formatting needs care.

## Scope

- `prb-pcap` crate (pipeline module)
- `prb-cli` crate (ingest command extension)

## Key Files and Context

The pipeline composes: `PcapFileReader` -> `PacketNormalizer` -> `TcpReassembler` + UDP extraction -> `TlsDecryptor` (optional) -> `DebugEvent` emission -> MCAP storage. The `CaptureAdapter` trait (from `prb-core`, Subsection 1) defines the interface: `fn ingest(&self, source: &Path, options: IngestOptions) -> Result<Session>`. `IngestOptions` must include `tls_keylog_path: Option<PathBuf>`. The pipeline must: (a) auto-detect file format from magic bytes (JSON fixture vs PCAP/pcapng), (b) report progress (packet count, stream count), (c) emit `DebugEvent` for each complete TCP stream and each UDP datagram, (d) summarize warnings on completion. For CLI: add `--tls-keylog` to `prb ingest`, display progress via simple stderr output. Integration test fixtures: generate synthetic PCAP files using `etherparse::PacketBuilder` for reproducible tests.

## Implementation Approach

Create `PcapCaptureAdapter` implementing `CaptureAdapter`. Wire the components in a push-based pipeline: iterate packets from reader, normalize, feed TCP segments to reassembler (collecting stream events), feed streams to TLS decryptor if key material available, convert output to `DebugEvent` with transport metadata, write to MCAP session. UDP path is simpler: each datagram becomes a `DebugEvent` immediately. Error handling: packet-level errors are logged via `tracing::warn!` and skipped; stream-level errors produce partial events with warning metadata. CLI extension: modify `prb ingest` command to accept file path, detect format, configure adapter with optional `--tls-keylog` argument.

## Alternatives Ruled Out

- Pull-based (iterator) pipeline -- rejected for TCP reassembly which needs to buffer across packets.
- Separate `prb ingest-pcap` subcommand -- rejected, format auto-detection makes a single `prb ingest` command cleaner UX.
- Separate binary for PCAP ingest -- rejected, should be unified CLI.

## Pre-Mortem Risks

- End-to-end integration may reveal interface mismatches between segments built independently.
- Performance bottleneck at any stage (especially TCP reassembly for high-connection-count captures) affects the whole pipeline.
- Large PCAP files (multi-GB) need streaming without OOM -- verify with large generated fixture.
- Progress reporting must not become a performance bottleneck.

## Build and Test Commands

- Build: `cargo build --workspace`
- Test (targeted): `cargo test -p prb-pcap -- pipeline && cargo test -p prb-cli -- ingest_pcap`
- Test (regression): `cargo test -p prb-pcap && cargo test -p prb-core && cargo test -p prb-storage`
- Test (full gate): `cargo test --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_pipeline_tcp_stream`: end-to-end ingest of PCAP with TCP stream produces correct DebugEvents
   - `test_pipeline_udp_datagram`: end-to-end ingest of PCAP with UDP produces correct DebugEvents
   - `test_pipeline_tls_decrypt`: end-to-end ingest of PCAP with TLS + keylog produces decrypted DebugEvents
   - `test_pipeline_mixed`: PCAP with TCP + UDP + TLS streams all produce correct events
   - `test_pipeline_error_tolerance`: corrupt packets are skipped, pipeline continues, warning count reported
   - `test_cli_ingest_pcap`: `prb ingest test.pcap` succeeds (assert_cmd integration test)
   - `test_cli_ingest_pcapng_tls`: `prb ingest test.pcapng --tls-keylog keys.log` succeeds
   - `test_cli_format_autodetect`: `prb ingest` handles both pcap and pcapng without explicit format flag
2. **Regression tests:** all existing `prb-pcap`, `prb-core`, and `prb-storage` tests pass
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo test --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, `CaptureAdapter` trait properly implemented.
6. **Scope verification gate:** Changes in `prb-pcap/src/pipeline.rs`, `prb-cli/src/ingest.rs`, and test fixtures only.
