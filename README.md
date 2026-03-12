# PRB

**Universal message debugger for gRPC, ZMTP, and DDS-RTPS.**

[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)

PRB decodes, inspects, and correlates protocol messages from packet captures, live network traffic, or fixture files. It supports TLS decryption, OpenTelemetry trace correlation, an interactive TUI, and exports to CSV, HAR, OTLP, Parquet, and HTML.

```
┌─────────────┐     ┌──────────────────────────────────────────────────┐     ┌────────────┐
│  PCAP/pcapng │     │                   PRB Pipeline                   │     │   Output   │
│  Live capture├────►│  Normalize ► TCP Reassembly ► TLS Decrypt ►     │     ├────────────┤
│  JSON fixture│     │  Protocol Detect ► Decode ► Correlate           ├────►│  NDJSON    │
└─────────────┘     └──────────────────────────────────────────────────┘     │  MCAP      │
                                                                             │  TUI       │
                                                                             │  CSV / HAR │
                                                                             │  OTLP JSON │
                                                                             │  Parquet   │
                                                                             │  HTML      │
                                                                             └────────────┘
```

## Features

- **Multi-protocol decoding** -- gRPC/HTTP2, ZeroMQ (ZMTP), and DDS-RTPS from a single tool
- **Packet capture ingestion** -- Read PCAP/pcapng files or capture live traffic with BPF filters
- **TLS decryption** -- Decrypt TLS traffic using SSLKEYLOGFILE or Wireshark DSB (Decryption Secrets Block)
- **Parallel pipeline** -- Shard-based parallel decoding with configurable worker count
- **Conversation reconstruction** -- Automatically groups related events into request/response conversations
- **OpenTelemetry correlation** -- Extract and filter by W3C traceparent, B3, and Jaeger trace contexts
- **Interactive TUI** -- Terminal UI with event list, decode tree, hex dump, and timeline panes
- **Query language** -- Filter events with expressions like `transport == "gRPC" && grpc.method contains "Users"`
- **Multiple export formats** -- CSV, HAR, OTLP JSON, HTML, and Parquet
- **Plugin system** -- Extend protocol support with native (.so/.dylib) or WebAssembly plugins
- **Protobuf schema support** -- Load .proto or .desc files for rich message decoding

## Quick Start

```bash
# Build from source
cargo build --release

# Decode a PCAP file to NDJSON
prb ingest capture.pcap

# Open the interactive TUI
prb tui capture.pcap

# Decode with TLS decryption
prb ingest capture.pcap --tls-keylog keys.log

# Export to CSV
prb export capture.pcap --format csv --output events.csv

# Capture live traffic
sudo prb capture -i eth0 --tui
```

## CLI Commands

| Command    | Description                                                          |
|------------|----------------------------------------------------------------------|
| `ingest`   | Decode a PCAP, pcapng, or JSON fixture file into NDJSON/MCAP events  |
| `inspect`  | Inspect and filter NDJSON debug events                               |
| `tui`      | Open the interactive TUI for exploring captured events               |
| `export`   | Export events to CSV, HAR, OTLP JSON, HTML, or Parquet               |
| `merge`    | Merge OTLP trace spans with captured packet events                   |
| `capture`  | Capture live network traffic with real-time protocol decoding        |
| `schemas`  | Load, list, and export protobuf schemas                              |
| `plugins`  | List, install, and remove protocol decoder plugins                   |

## Supported Protocols

| Protocol   | Transport | Decoded Metadata                                          |
|------------|-----------|-----------------------------------------------------------|
| gRPC       | HTTP/2    | Method, status, headers, stream ID, protobuf fields       |
| ZMTP       | TCP       | Socket type, topic, identity, message frames              |
| DDS-RTPS   | UDP       | Domain ID, topic name, participant GUID, QoS              |

All protocols support automatic detection via port mapping, magic-byte inspection, and heuristic analysis. Detection can also be overridden with `--protocol grpc|zmtp|rtps`.

## Architecture

PRB is organized as a Cargo workspace with 19 crates, each owning a single responsibility:

```
prb-cli ─────────────────────────────────────────── CLI entry point
  ├── prb-core ──────────────────────────────────── Core types, traits, conversation engine
  │     ├── prb-decode ──────────────────────────── Protobuf wire-format decoding
  │     └── prb-schema ──────────────────────────── Protobuf schema registry
  ├── prb-pcap ──────────────────────────────────── PCAP reading, TCP reassembly, TLS decrypt
  │     ├── prb-detect ──────────────────────────── Protocol detection engine
  │     ├── prb-grpc ────────────────────────────── gRPC/HTTP2 decoder
  │     ├── prb-zmq ─────────────────────────────── ZeroMQ/ZMTP decoder
  │     └── prb-dds ─────────────────────────────── DDS/RTPS decoder
  ├── prb-fixture ───────────────────────────────── JSON fixture adapter
  ├── prb-capture ───────────────────────────────── Live packet capture (libpcap)
  ├── prb-storage ───────────────────────────────── MCAP session storage
  ├── prb-tui ───────────────────────────────────── Interactive terminal UI
  ├── prb-export ────────────────────────────────── CSV, HAR, OTLP, HTML, Parquet export
  ├── prb-query ─────────────────────────────────── Event filter query language
  ├── prb-ai ────────────────────────────────────── LLM-powered event explanation
  ├── prb-plugin-api ────────────────────────────── Plugin contract (DTOs, FFI)
  ├── prb-plugin-native ─────────────────────────── Native shared library plugins
  └── prb-plugin-wasm ───────────────────────────── WebAssembly plugins
```

The central abstraction is `DebugEvent` -- every decoder, adapter, and exporter speaks this type. Protocol decoders implement `ProtocolDecoder`, input sources implement `CaptureAdapter`, and event grouping uses per-protocol `CorrelationStrategy` implementations.

See [docs/architecture.md](docs/architecture.md) for the full design document.

## Development

This project uses [`just`](https://github.com/casey/just) for task automation.

### Setup

```bash
# Install just
cargo install just

# Install development dependencies
just setup

# Install pre-commit hooks (optional but recommended)
just install-hooks
```

### Common Commands

```bash
# See all available commands
just

# Run all checks before committing
just check

# Run full CI locally
just ci

# Generate coverage report
just coverage

# Run tests
just test

# Build release binary
just build
```

See `justfile` for all available commands.

## Orchestration

This project uses [orchestrate](https://github.com/psauer/orchestrate) for managing large-scale refactoring and development plans.

### Setup

```bash
# Install orchestrate
cd ~/orchestrate
pip install -e ".[dev]"

# Verify installation
orchestrate --help
```

### Running Plans

```bash
# Execute a plan
cd /Users/psauer/probe
orchestrate run .claude/plans/{plan-name}

# Dry-run to see computed waves
orchestrate dry-run .claude/plans/{plan-name}

# Check plan status
orchestrate status .claude/plans/{plan-name}
```

The orchestration dashboard is available at http://localhost:8080 when running plans.

Configuration is in `.claude/orchestrate.toml`.

## Documentation

| Document                                              | Description                                  |
|-------------------------------------------------------|----------------------------------------------|
| [Getting Started](docs/getting-started.md)            | Installation, build, and first-use tutorial  |
| [User Guide](docs/user-guide.md)                      | Command reference and common workflows       |
| [Architecture](docs/architecture.md)                   | System design, crate map, data flow          |
| [Protocols](docs/protocols.md)                         | gRPC, ZMTP, DDS-RTPS decoding details        |
| [TLS Decryption](docs/tls-decryption.md)               | SSLKEYLOGFILE setup and TLS troubleshooting  |
| [Query Language](docs/query-language.md)               | Filter syntax, operators, and examples       |
| [Export Formats](docs/export-formats.md)               | CSV, HAR, OTLP, Parquet, HTML schemas        |
| [Configuration](docs/configuration.md)                 | Environment variables and CLI flags          |
| [Plugin Development](docs/plugin-development.md)       | Native and WASM plugin authoring guide       |
| [Troubleshooting](docs/troubleshooting.md)             | Common issues and performance tuning         |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, build commands, testing requirements, and the pull request process.

## License

This project is licensed under the [GNU Affero General Public License v3.0](LICENSE).

If you run a modified version of this software as a network service, the AGPL requires you to make the complete source code available to users of that service. See the [LICENSE](LICENSE) file for the full terms.

For commercial licensing options that do not require source disclosure, contact the maintainers.
