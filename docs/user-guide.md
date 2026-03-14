# User Guide

Complete reference for all PRB commands, flags, and common workflows.

## Command Reference

### prb ingest

Decode a PCAP, pcapng, or JSON fixture file into debug events.

```bash
prb ingest <INPUT> [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `<INPUT>` | Path to input file (PCAP, pcapng, or JSON fixture) |
| `-o, --output <FILE>` | Output file (defaults to stdout for NDJSON; use `.mcap` extension for MCAP) |
| `--tls-keylog <FILE>` | Path to SSLKEYLOGFILE for TLS decryption |
| `--protocol <PROTO>` | Force protocol: `grpc`, `zmtp`, or `rtps` (bypasses auto-detection) |
| `--trace-id <ID>` | Filter events by OpenTelemetry trace ID |
| `--span-id <ID>` | Filter events by OpenTelemetry span ID |
| `-j, --jobs <N>` | Parallel workers: `0` = auto, `1` = sequential (default: `0`) |

**Examples:**

```bash
# Basic PCAP decode
prb ingest capture.pcap

# Save to file
prb ingest capture.pcap -o events.ndjson

# Save as MCAP session
prb ingest capture.pcap -o session.mcap

# With TLS decryption
prb ingest capture.pcap --tls-keylog keys.log

# Force gRPC detection
prb ingest capture.pcap --protocol grpc

# Filter by trace ID during ingest
prb ingest capture.pcap --trace-id 4bf92f3577b34da6a3ce929d0e0e4736

# 8-worker parallel decode
prb ingest large.pcap -j 8
```

### prb inspect

Inspect and filter NDJSON debug events.

```bash
prb inspect [INPUT] [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `[INPUT]` | Path to NDJSON file (reads from stdin if omitted) |
| `-f, --format <FMT>` | Output format: `table` (default) or `json` |
| `--filter <EXPR>` | Filter by transport kind |
| `--where <EXPR>` | Filter with a query expression |
| `--trace-id <ID>` | Filter by trace ID |
| `--span-id <ID>` | Filter by span ID |
| `--group-by-trace` | Group events into trace trees |
| `--wire-format` | Decode protobuf payloads as wire format (best-effort, no schema) |

**Examples:**

```bash
# Table view
prb inspect events.ndjson

# Pipe from ingest
prb ingest capture.pcap | prb inspect --where 'transport == "gRPC"'

# JSON output for further processing
prb inspect events.ndjson -f json --where 'grpc.method contains "Users"'

# Trace tree view
prb inspect events.ndjson --group-by-trace --trace-id abc123
```

### prb tui

Open the interactive terminal UI.

```bash
prb tui <INPUT> [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `<INPUT>` | Path to input file (JSON, PCAP, pcapng, or MCAP) |
| `--where <EXPR>` | Pre-apply a filter expression on open |

The TUI supports keyboard navigation and mouse interaction. See [TUI Reference](tui-reference.md) for complete documentation.

**Quick Reference:**

| Key | Action |
|-----|--------|
| `j` / `k` or Arrow keys | Navigate events |
| `Tab` / `Shift+Tab` | Cycle between panes |
| `/` | Open filter input |
| `Enter` | Accept autocomplete or expand tree nodes |
| `a` | AI explain selected event |
| `e` | Export dialog |
| `:` | Command palette |
| `?` | Help overlay |
| `q` | Quit |

### prb export

Export events to developer ecosystem formats.

```bash
prb export <INPUT> --format <FMT> [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `<INPUT>` | Path to input file |
| `-f, --format <FMT>` | Export format: `csv`, `har`, `otlp`, `html`, `parquet` |
| `-o, --output <FILE>` | Output file (required for binary formats) |
| `--where <EXPR>` | Filter events before export |

See [Export Formats](export-formats.md) for per-format details.

### prb merge

Merge OTLP trace spans with captured packet events, enriching packet-level events with trace metadata.

```bash
prb merge <PACKETS> <TRACES> [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `<PACKETS>` | Packet events file (NDJSON or MCAP) |
| `<TRACES>` | OTLP JSON trace file |
| `-o, --output <FILE>` | Output file (defaults to stdout NDJSON) |

**Example:**

```bash
prb merge events.ndjson traces.json -o merged.ndjson
```

### prb capture

Capture live network traffic with real-time protocol decoding.

```bash
prb capture [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `-i, --interface <IF>` | Network interface (e.g., `eth0`, `lo`, `en0`) |
| `-f, --filter <BPF>` | BPF filter expression (same syntax as tcpdump) |
| `-o, --output <FILE>` | Write decoded events to file (NDJSON or MCAP) |
| `-w, --write <FILE>` | Write raw packets to pcap savefile |
| `--tls-keylog <FILE>` | TLS keylog file for decrypting live traffic |
| `--snaplen <N>` | Max bytes per packet (default: 65535) |
| `--no-promisc` | Disable promiscuous mode |
| `-c, --count <N>` | Stop after N packets |
| `--duration <N>` | Stop after N seconds |
| `--list-interfaces` | List available interfaces and exit |
| `--tui` | Open live TUI |
| `--format <FMT>` | Output format: `summary` (default) or `json` |
| `-q, --quiet` | Suppress per-packet output, only show final stats |
| `--buffer-size <N>` | Kernel capture buffer size in bytes (default: 16MB) |

**Examples:**

```bash
# List interfaces
prb capture --list-interfaces

# Capture gRPC traffic with live TUI
sudo prb capture -i eth0 -f "port 50051" --tui

# Capture to file with 30-second limit
sudo prb capture -i lo -o events.ndjson --duration 30

# Capture raw + decoded simultaneously
sudo prb capture -i eth0 -w raw.pcap -o decoded.ndjson
```

### prb schemas

Manage protobuf schemas for rich message decoding.

```bash
prb schemas <SUBCOMMAND>
```

| Subcommand | Description |
|------------|-------------|
| `load <PATH> [-I <INCLUDE>]` | Load a `.proto` or `.desc` file |
| `list <SESSION>` | List message types in a session's embedded schemas |
| `export <SESSION> -o <FILE>` | Export schemas from a session to a `.desc` file |

### prb plugins

Manage protocol decoder plugins.

```bash
prb plugins <SUBCOMMAND>
```

| Subcommand | Description |
|------------|-------------|
| `list` | List all available decoders and plugins |
| `info <NAME>` | Show detailed info about a decoder |
| `install <PATH> [--name <NAME>]` | Install a plugin from a `.so`/`.dylib`/`.dll` or `.wasm` file |
| `remove <NAME>` | Remove an installed plugin |

## Common Workflows

### Offline PCAP Analysis

```bash
# 1. Decode the PCAP
prb ingest capture.pcap -o session.mcap

# 2. Explore in TUI
prb tui session.mcap

# 3. Export filtered results
prb export session.mcap --format csv --where 'transport == "gRPC"' -o grpc-events.csv
```

### Live Debugging Session

```bash
# Start capturing with TUI
sudo prb capture -i eth0 -f "port 50051" --tui -o recording.ndjson

# After stopping, review the recording
prb tui recording.ndjson
```

### Trace Correlation

```bash
# 1. Decode packets
prb ingest capture.pcap -o packets.ndjson

# 2. Export traces from your observability backend (Jaeger, Tempo, etc.)
# ... save as traces.json in OTLP format

# 3. Merge packet events with trace spans
prb merge packets.ndjson traces.json -o merged.ndjson

# 4. Inspect by trace
prb inspect merged.ndjson --group-by-trace
```

### Pipeline Composition

PRB commands compose via stdin/stdout piping:

```bash
# Decode, filter, and export in one pipeline
prb ingest capture.pcap | prb inspect --where 'transport == "gRPC"' -f json | jq '.metadata'

# Multi-stage filtering
prb ingest capture.pcap | prb inspect --where 'grpc.method contains "Users"' -f json > users.ndjson
```

## Global Flags

These flags are available on all commands:

| Flag | Description |
|------|-------------|
| `--plugin-dir <PATH>` | Plugin directory (default: `~/.prb/plugins/`) |
| `--no-plugins` | Disable automatic plugin loading |
| `--version` | Print version |
| `--help` | Print help |
