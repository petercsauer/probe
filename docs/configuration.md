# Configuration

PRB is configured through CLI flags, environment variables, and file-system conventions. There are no configuration files -- all settings are per-invocation.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PRB_JOBS` | `0` (auto) | Override the `--jobs` flag for parallel ingest. `0` = auto-detect CPU count, `1` = sequential. |
| `PRB_PLUGIN_DIR` | `~/.prb/plugins/` | Directory to load plugins from. Overridden by `--plugin-dir`. |
| `PRB_AI_API_KEY` | (none) | API key for the AI explanation provider (Ollama, OpenAI). |
| `SSLKEYLOGFILE` | (none) | Path for applications to write TLS session keys. Not read by PRB directly; use `--tls-keylog` to point PRB to the file. |
| `RUST_LOG` | (none) | Controls log output via `tracing-subscriber`. Examples: `RUST_LOG=debug`, `RUST_LOG=prb_pcap=trace`. |

## CLI Global Flags

These flags are available on all commands:

| Flag | Description |
|------|-------------|
| `--plugin-dir <PATH>` | Override the plugin directory |
| `--no-plugins` | Disable automatic plugin loading |
| `--version` | Print version information |
| `--help` | Print help for the command |

## Plugin Directory Layout

```
~/.prb/
└── plugins/
    ├── my-decoder.so        # Native Linux plugin
    ├── my-decoder.dylib     # Native macOS plugin
    └── wasm-decoder.wasm    # WASM plugin
```

Plugins are discovered by scanning the directory for files with `.so`, `.dylib`, `.dll`, or `.wasm` extensions. Each file is loaded and its `info()` function is called to register the decoder.

## Parallel Processing

The `--jobs` flag (or `PRB_JOBS` environment variable) controls parallelism during PCAP ingestion:

| Value | Behavior |
|-------|----------|
| `0` | Auto-detect: uses the number of logical CPUs (default) |
| `1` | Sequential processing: no parallelism, useful for debugging |
| `N` | Use exactly N worker threads for shard-based parallel decoding |

The parallel pipeline partitions packets by network flow (5-tuple hash), so each worker handles complete TCP streams independently.

## MCAP Output

When the output path has an `.mcap` extension, PRB writes to MCAP format instead of NDJSON:

```bash
prb ingest capture.pcap -o session.mcap
```

MCAP files can be re-opened with `prb tui session.mcap` and support embedded protobuf schemas.

## Capture Configuration

Live capture has additional configuration via CLI flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--snaplen` | 65535 | Maximum bytes to capture per packet |
| `--buffer-size` | 16777216 (16 MB) | Kernel capture buffer size in bytes |
| `--no-promisc` | (off) | Disable promiscuous mode on the interface |

For high-throughput captures, increase `--buffer-size` to avoid dropped packets:

```bash
sudo prb capture -i eth0 --buffer-size 67108864  # 64 MB buffer
```

## Logging

PRB uses the `tracing` framework. Control log output with `RUST_LOG`:

```bash
# Debug output for all PRB crates
RUST_LOG=debug prb ingest capture.pcap

# Trace-level output for a specific crate
RUST_LOG=prb_pcap=trace prb ingest capture.pcap

# Multiple crate filters
RUST_LOG=prb_detect=debug,prb_grpc=trace prb ingest capture.pcap

# Suppress all logs except errors
RUST_LOG=error prb ingest capture.pcap
```
