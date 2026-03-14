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

<!-- cargo-rdme start -->

Live packet capture engine for the PRB universal message debugger.

This crate provides live packet capture from network interfaces using libpcap.
It includes:

- Interface enumeration and selection
- BPF filter compilation and application
- Dedicated OS thread capture loop with bounded channel delivery
- Real-time statistics tracking
- Privilege checking (Linux `CAP_NET_RAW`)

### Architecture

The capture engine uses a dedicated OS thread (not a tokio task) for the packet
capture loop. This is critical for production capture systems because:

1. `cap.next_packet()` must be called continuously without interruption
2. Tokio tasks can be preempted by the scheduler on a loaded runtime
3. Preemption causes packet drops in the kernel ring buffer
4. A real OS thread guarantees the capture loop is never preempted

All production capture systems (Hubble, Suricata, Zeek, Wireshark) use this model.

### Example

```rust
use prb_capture::{CaptureEngine, CaptureConfig};

let config = CaptureConfig::new("eth0")
    .with_filter("tcp port 443");

let mut engine = CaptureEngine::new(config);
engine.start().expect("failed to start capture");

// Receive packets
if let Some(rx) = engine.receiver() {
    for packet in rx.iter() {
        println!("Captured {} bytes", packet.data.len());
    }
}

// Stop and get statistics
let stats = engine.stop().expect("failed to stop capture");
println!("{}", stats);
```

<!-- cargo-rdme end -->
