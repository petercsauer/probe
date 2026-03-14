# prb-pcap

The main ingestion engine for PRB, responsible for reading PCAP and pcapng capture files, reassembling TCP streams, decrypting TLS traffic, and feeding reassembled data through protocol decoders. It supports memory-mapped I/O for large files and a parallel pipeline for multi-core throughput.

## Key Types

| Type | Description |
|------|-------------|
| `PcapFileReader` | Reads legacy PCAP and pcapng formats, extracts packets and embedded TLS keys (DSB) |
| `MmapPcapReader` | Memory-mapped reader for efficient processing of large capture files |
| `TcpReassembler` | Reassembles TCP segments into ordered bidirectional byte streams |
| `TlsStreamProcessor` | Decrypts TLS-encrypted streams using SSLKEYLOGFILE material |
| `TlsKeyStore` | Loads and indexes TLS session keys from keylog files and pcapng DSBs |
| `ParallelPipeline` | Multi-stage parallel processing pipeline with configurable worker counts |
| `PipelineConfig` | Configuration for batch sizes, parallelism, and decoder selection |
| `PcapCaptureAdapter` | Implements `CaptureAdapter` — full pcap-to-events pipeline |
| `PipelineStats` | Packet counts, byte totals, stream counts, and timing metrics |
| `FlowKey` / `FlowProtocol` | 5-tuple flow identification for TCP/UDP stream tracking |
| `NormalizedPacket` | Layer-3/4 parsed packet ready for stream reassembly |
| `PipelineCore` | Shared core logic used by both sequential and parallel pipelines |

## Usage

```rust
use prb_pcap::PcapCaptureAdapter;
use prb_core::CaptureAdapter;

let mut adapter = PcapCaptureAdapter::new("capture.pcapng".into(), None);

for event in adapter.ingest() {
    let event = event?;
    println!("[{}] {}", event.transport, event.source.origin);
}
```

## Relationship to Other Crates

`prb-pcap` depends on `prb-core` for types/traits and `prb-detect` for automatic protocol detection. It optionally uses `prb-grpc`, `prb-zmq`, and `prb-dds` as builtin protocol decoders (behind the `builtin-decoders` feature flag). The `prb-cli` crate drives `prb-pcap` for all pcap/pcapng ingestion workflows. `prb-capture` reuses `prb-pcap`'s TCP reassembly and pipeline logic for live captures.

See the [PRB documentation](../../docs/) for the full user guide.

<!-- cargo-rdme start -->

PCAP/pcapng file reading and TLS key extraction for PRB.

This crate provides transparent reading of both legacy PCAP and modern pcapng
capture formats, with support for extracting embedded TLS keys from pcapng
Decryption Secrets Blocks (DSB).

<!-- cargo-rdme end -->
