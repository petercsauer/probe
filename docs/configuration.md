# Configuration

PRB is configured through CLI flags, environment variables, configuration files, and file-system conventions.

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

**TUI Logging:** In TUI mode, logs are written to `/tmp/prb-tui.log` to avoid corrupting the display. Default filter: `prb_tui::panes::event_list=trace,warn`.

## Configuration Files

### TUI Configuration: `~/.config/prb/config.toml`

Optional TOML file for TUI customization:

```toml
[tui.keybindings]
quit = "q"
help = "?"
filter = "/"
zoom = "z"
theme_cycle = "T"

[tui.theme]
# Override specific colors
event_list_selected_bg = "#3b4252"
filter_bar_fg = "#88c0d0"

[ai]
provider = "ollama"  # or "openai" or "custom"
model = "llama3.1"
api_key = "sk-..."  # or use PRB_AI_API_KEY env var
base_url = "http://localhost:11434/v1"  # for custom provider
```

**AI Provider Configuration:**
- `ollama` (default) - Local inference, no API key required
- `openai` - Requires `api_key`, uses gpt-4o-mini by default
- `custom` - Any OpenAI-compatible endpoint, requires `base_url` and `api_key`

### Filter Persistence: `~/.config/prb/filters.toml`

Automatically managed by the TUI. Stores filter history and favorites:

```toml
history = [
    "transport == \"gRPC\"",
    "grpc.status != 0",
    "tcp.port in {80, 443}"
]

[[favorites]]
name = "DNS Traffic"
filter = "transport == \"UDP\" && dst contains \":53\""
description = "All DNS queries"
created_at = 1710000000

[[favorites]]
name = "gRPC Errors"
filter = "transport == \"gRPC\" && grpc.status != 0"
```

**Limits:**
- History: 50 entries (FIFO)
- Favorites: 100 entries (FIFO)

Access via TUI:
- `↑` / `↓` in filter mode to browse history
- `Ctrl+F` in filter mode to favorite current filter
- `F3` to open filter templates (includes favorites)

### Session Files

Session files preserve TUI state for later restoration. Format: JSON v1.0

**Saved State:**
- Input file path
- Active filter expression
- Scroll position and selected event
- Pane focus (EventList, DecodeTree, HexDump, Timeline)
- Split percentages (vertical: event list height, horizontal: decode/hex split)
- View toggles (conversations, waterfall, trace, AI panel)
- TLS keylog path

**Save:** Via command palette (`:`) → "Save session"
**Load:** `prb tui --session <file.json>`

**Example session.json:**
```json
{
  "version": "1.0",
  "input_file": "/path/to/capture.pcap",
  "filter": "transport == \"gRPC\" && grpc.status != 0",
  "scroll_offset": 42,
  "selected_event": 42,
  "pane_focus": "DecodeTree",
  "tls_keylog": "/path/to/keylog.txt",
  "vertical_split": 60,
  "horizontal_split": 50,
  "ai_panel_visible": true,
  "showing_conversations": false,
  "showing_waterfall": false
}
```
