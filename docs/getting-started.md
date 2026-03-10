# Getting Started

This guide walks you through installing PRB, decoding your first packet capture, and exploring events in the interactive TUI.

## Prerequisites

- **Rust toolchain** -- Rust 2024 edition (1.85+). Install via [rustup](https://rustup.rs/).
- **libpcap** -- Required for live capture and PCAP reading.
  - macOS: included with Xcode Command Line Tools (`xcode-select --install`)
  - Debian/Ubuntu: `sudo apt install libpcap-dev`
  - Fedora/RHEL: `sudo dnf install libpcap-devel`
  - Arch: `sudo pacman -S libpcap`
- **protoc** (optional) -- Required only if loading `.proto` schema files. Install via your package manager or from [github.com/protocolbuffers/protobuf](https://github.com/protocolbuffers/protobuf/releases).

## Build from Source

```bash
git clone https://github.com/yourusername/prb.git
cd prb
cargo build --release
```

The binary is at `target/release/prb`. Add it to your PATH or use `cargo install --path crates/prb-cli`.

Verify the installation:

```bash
prb --version
prb --help
```

## Your First Decode

### From a PCAP file

If you have a PCAP or pcapng file containing gRPC, ZMQ, or DDS traffic:

```bash
prb ingest capture.pcap
```

This outputs NDJSON (one JSON object per line) to stdout. Each line is a `DebugEvent` containing the decoded protocol message, metadata, timestamps, and network addresses.

To save the output:

```bash
prb ingest capture.pcap > events.ndjson
```

### From a JSON fixture

PRB includes a fixture format for testing and demos:

```bash
prb ingest fixtures/grpc_sample.json
```

### With TLS decryption

If your PCAP contains TLS-encrypted traffic and you have the session keys:

```bash
prb ingest capture.pcap --tls-keylog /path/to/sslkeylog.txt
```

See [TLS Decryption](tls-decryption.md) for how to generate keylog files for your language/framework.

### Parallel decoding

For large PCAP files, PRB automatically uses parallel decoding. Control the worker count:

```bash
prb ingest large-capture.pcap --jobs 8
```

Use `--jobs 1` for sequential processing or `--jobs 0` (default) for automatic detection.

## Interactive TUI

The TUI provides a Wireshark-like experience in your terminal:

```bash
prb tui capture.pcap
```

The TUI has four panes:

- **Event List** -- scrollable list of all decoded events with timestamps, protocols, and summaries
- **Decode Tree** -- hierarchical view of the selected event's decoded fields
- **Hex Dump** -- raw bytes of the selected event's payload
- **Timeline** -- visual timeline of events and conversations

### TUI with a pre-applied filter

```bash
prb tui capture.pcap --where 'transport == "gRPC"'
```

## Inspecting Events

For quick command-line inspection without the TUI:

```bash
# Table format (default)
prb inspect events.ndjson

# Filter by transport
prb inspect events.ndjson --where 'transport == "gRPC"'

# Filter by metadata
prb inspect events.ndjson --where 'grpc.method contains "Users"'

# Filter by trace ID
prb inspect events.ndjson --trace-id abc123def456

# Group by trace
prb inspect events.ndjson --group-by-trace
```

## Exporting Events

Export decoded events to formats your existing tools understand:

```bash
# CSV for spreadsheets and data analysis
prb export capture.pcap --format csv --output events.csv

# HAR for browser dev-tools and HTTP analysis
prb export capture.pcap --format har --output events.har

# OTLP JSON for OpenTelemetry backends (Jaeger, Tempo, etc.)
prb export capture.pcap --format otlp --output traces.json

# HTML for shareable reports
prb export capture.pcap --format html --output report.html
```

## Live Capture

Capture and decode traffic in real time:

```bash
# List available interfaces
prb capture --list-interfaces

# Capture on an interface with BPF filter
sudo prb capture -i eth0 -f "port 50051"

# Capture with live TUI
sudo prb capture -i eth0 --tui

# Save raw packets and decoded events
sudo prb capture -i eth0 -w raw.pcap -o events.ndjson
```

## Next Steps

- [User Guide](user-guide.md) -- full command reference and workflow recipes
- [Query Language](query-language.md) -- filter expression syntax
- [Protocols](protocols.md) -- protocol-specific decoding details
- [Architecture](architecture.md) -- how PRB works internally
