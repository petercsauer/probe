# prb-cli

The command-line entry point for the PRB universal message debugger. This crate compiles to the `prb` binary and orchestrates all user-facing functionality — ingesting captures, inspecting events, launching the TUI, exporting to developer formats, live packet capture, and managing schemas and plugins.

## Subcommands

| Command | Description |
|---------|-------------|
| `prb ingest <file>` | Ingest a JSON fixture, PCAP, or pcapng file and emit debug events (NDJSON or MCAP) |
| `prb inspect [file]` | Display debug events as a table or JSON, with filtering and trace grouping |
| `prb tui <file>` | Open the interactive terminal UI for exploring captured events |
| `prb export <file>` | Export events to CSV, HAR, OTLP, HTML, or Parquet |
| `prb merge <packets> <traces>` | Merge OTLP trace spans with captured packet events |
| `prb capture` | Capture live network traffic with real-time protocol decoding |
| `prb schemas <load\|list\|export>` | Load, list, or export protobuf schemas |
| `prb plugins <list\|info\|install\|remove>` | Manage protocol decoder plugins |

## Key Types

| Type | Description |
|------|-------------|
| `Cli` | Top-level clap `Parser` with global options (`--plugin-dir`, `--no-plugins`) |
| `Commands` | Enum of all subcommands dispatched from `main()` |
| `IngestArgs` | Input path, output path, TLS keylog, protocol override, parallelism |
| `CaptureArgs` | Interface, BPF filter, output, snaplen, duration, TUI mode |
| `ExportFormat` | Enum: `Csv`, `Har`, `Otlp`, `Html`, `Parquet` |

## Usage

```bash
# Ingest a pcap and write MCAP session
prb ingest capture.pcapng -o session.mcap

# Inspect with a query filter
prb inspect session.mcap --where 'transport == "gRPC"'

# Launch interactive TUI
prb tui session.mcap

# Export to CSV
prb export session.mcap --format csv -o events.csv

# Live capture on eth0
prb capture -i eth0 --tui
```

## Relationship to Other Crates

`prb-cli` is the **top-level integration crate** — it depends on nearly every other crate in the workspace. It uses `prb-core` for types, `prb-pcap` and `prb-fixture` for ingestion, `prb-storage` for MCAP I/O, `prb-schema` and `prb-decode` for protobuf handling, `prb-grpc`/`prb-zmq` for protocol decoding, `prb-capture` for live capture, `prb-tui` for the terminal UI, `prb-export` for output formats, `prb-query` for filtering, and `prb-plugin-api`/`prb-plugin-native`/`prb-plugin-wasm` for extensibility.

See the [PRB documentation](../../docs/) for the full user guide.

<!-- cargo-rdme start -->

PRB CLI library - exposes command handlers and CLI definitions for testing.

<!-- cargo-rdme end -->
