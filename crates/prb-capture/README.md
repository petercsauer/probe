# prb-capture

Live packet capture engine for PRB, providing real-time network traffic acquisition from system interfaces via libpcap. The capture loop runs on a dedicated OS thread (not a tokio task) to guarantee continuous packet reads without scheduler preemption — the same architecture used by production capture systems like Wireshark, Suricata, and Zeek.

## Key Types

| Type | Description |
|------|-------------|
| `CaptureEngine` | Manages the capture lifecycle: start, receive packets, stop, report statistics |
| `CaptureConfig` | Interface name, BPF filter, snaplen, promiscuous mode, buffer size |
| `LiveCaptureAdapter` | Implements `CaptureAdapter` — bridges live capture into the PRB event pipeline |
| `OwnedPacket` | A captured packet with timestamp and owned byte buffer |
| `CaptureStats` | Packets received, dropped (kernel + interface), bytes captured |
| `InterfaceEnumerator` | Discovers available network interfaces |
| `InterfaceInfo` | Interface name, description, addresses, and flags |
| `PrivilegeCheck` | Checks for required privileges (Linux `CAP_NET_RAW`) |
| `CaptureError` | Error type for permission, device, and I/O failures |

## Usage

```rust
use prb_capture::{CaptureEngine, CaptureConfig};

let config = CaptureConfig::new("eth0")
    .with_filter("tcp port 443")
    .with_snaplen(65535);

let mut engine = CaptureEngine::new(config);
engine.start().expect("failed to start capture");

if let Some(rx) = engine.receiver() {
    for packet in rx.iter() {
        println!("Captured {} bytes", packet.data.len());
    }
}

let stats = engine.stop().expect("failed to stop capture");
println!("{}", stats);
```

## Relationship to Other Crates

`prb-capture` depends on `prb-core` for the `CaptureAdapter` trait and event types, and on `prb-pcap` for TCP reassembly and pipeline processing of captured packets. It is used by `prb-cli`'s `capture` subcommand to provide live network capture with real-time protocol decoding and optional TUI display.

See the [PRB documentation](../../docs/) for the full user guide.
