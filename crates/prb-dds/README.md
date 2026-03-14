# prb-dds

DDS/RTPS protocol decoder for offline PCAP analysis. This crate parses RTPS wire-format messages from UDP datagrams, extracting submessage structures (DATA, HEARTBEAT, ACKNACK, INFO_TS, DATA_FRAG, etc.), resolving topic names through SEDP discovery tracking, and producing `DebugEvent`s with GUID-based correlation metadata. It implements the `ProtocolDecoder` trait from `prb-core`.

## Key types

| Type | Description |
|------|-------------|
| `DdsDecoder` | Main decoder — implements `ProtocolDecoder::decode_stream` for RTPS datagrams |
| `DdsCorrelationStrategy` | Generates `CorrelationKey` values from RTPS GUID prefixes |
| `DdsError` | Error type covering magic-byte mismatches, truncated headers, and parse failures |
| `RtpsHeader` | Parsed 20-byte RTPS message header (protocol version, vendor ID, GUID prefix) |
| `SubmessageHeader` | Parsed 4-byte submessage header (ID, flags, length) |

## Usage

```rust
use prb_dds::DdsDecoder;
use prb_core::{DecodeContext, ProtocolDecoder};

let mut decoder = DdsDecoder::new();
let rtps_datagram: &[u8] = &[ /* raw UDP payload */ ];
let ctx = DecodeContext::default();

let events = decoder.decode_stream(rtps_datagram, &ctx)?;
for event in &events {
    println!("{:?} -> {:?}", event.transport, event.metadata);
}
```

## Relationship to other crates

- **prb-core** — provides `ProtocolDecoder`, `DebugEvent`, `CorrelationKey`, and other shared types
- **prb-detect** — the `RtpsDetector` in prb-detect identifies RTPS traffic by its `b"RTPS"` magic bytes and delegates decoding to this crate
- **prb-tui** — renders decoded DDS events in the interactive terminal UI

See the [PRB documentation](../../docs/) for the full user guide.

<!-- cargo-rdme start -->

DDS/RTPS protocol decoder for offline PCAP analysis.

This crate implements DDS/RTPS protocol decoding from UDP datagrams,
including RTPS message parsing, DATA submessage payload extraction,
SEDP discovery tracking for topic name resolution, and GUID-based
correlation metadata.

<!-- cargo-rdme end -->
